use std::collections::{BTreeMap, HashMap};

use chrono::{NaiveDateTime, Timelike};
use extrittio_device_contract::{AggregateKind, DeviceBlueprint, FieldValueType};

use super::require_permission;
use crate::Permission;
use crate::{ApplicationError, PersistenceError, TenantContext};

use crate::analytics::AnalyticsRepository;
use crate::analytics::{
    AnalyticsBlueprintRevision, AnalyticsBucket, AnalyticsCoveragePoint, AnalyticsDataSource,
    AnalyticsDeviceStats, AnalyticsMetric, AnalyticsMetricSelector, AnalyticsPoint, AnalyticsQuery,
    AnalyticsQueryData, AnalyticsRequest, AnalyticsResult, AnalyticsSeries, AnalyticsSeriesKind,
    AnalyticsSeriesMode, AnalyticsStats, AnalyticsWeighting,
};

const MAX_DEVICES: usize = 50;
const DEFAULT_MAX_POINTS_PER_SERIES: usize = 1_200;
const MAX_POINTS_PER_SERIES: usize = 2_000;
const MAX_TOTAL_POINTS: usize = 100_000;
const MAX_SCOPE_VALUES: usize = 100;
const MAX_RANGE_SECONDS: i64 = 366 * 24 * 60 * 60;
const ALLOWED_BUCKET_SECONDS: [i64; 6] = [60, 300, 900, 3_600, 21_600, 86_400];

#[derive(Clone)]
pub struct AnalyticsApplication {
    repository: std::sync::Arc<dyn AnalyticsRepository>,
}
impl AnalyticsApplication {
    pub fn new(repository: std::sync::Arc<dyn AnalyticsRepository>) -> Self {
        Self { repository }
    }
    pub async fn catalog(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<AnalyticsMetric>, ApplicationError> {
        require_permission(ctx, Permission::ReadTelemetry)?;
        let revisions = self.repository.blueprint_catalog(ctx.tenant_id()).await?;
        metrics_from_blueprints(revisions)
    }

    pub async fn query(
        &self,
        ctx: &TenantContext,
        request: AnalyticsRequest,
    ) -> Result<AnalyticsResult, ApplicationError> {
        require_permission(ctx, Permission::ReadTelemetry)?;
        validate_scope(&request)?;

        let (metric, compatible_revision_ids) = resolve_metric(
            request.metric.clone(),
            self.repository.blueprint_catalog(ctx.tenant_id()).await?,
        )?;

        let range_seconds = (request.end - request.start).num_seconds();
        if range_seconds <= 0 {
            return Err(ApplicationError::InvalidInput(
                "Analytics time range must have an end after its start".to_string(),
            ));
        }
        if range_seconds > MAX_RANGE_SECONDS {
            return Err(ApplicationError::InvalidOperation(
                "Analytics time range cannot exceed 366 days".to_string(),
            ));
        }

        let max_points = request
            .max_points_per_series
            .unwrap_or(DEFAULT_MAX_POINTS_PER_SERIES)
            .clamp(10, MAX_POINTS_PER_SERIES);
        let bucket_seconds = match request.bucket_seconds {
            Some(bucket) => validate_explicit_bucket(bucket, range_seconds, max_points)?,
            None => automatic_bucket(range_seconds, max_points),
        };
        let data = self
            .repository
            .query(
                ctx.tenant_id(),
                AnalyticsQuery {
                    scope: request.scope.clone(),
                    metric: metric.clone(),
                    compatible_revision_ids,
                    start: storage_bound(request.start)?,
                    end: storage_bound(request.end)?,
                    bucket_seconds,
                    max_devices: MAX_DEVICES,
                    max_rows: MAX_TOTAL_POINTS,
                },
            )
            .await
            .map_err(|error| match error {
                PersistenceError::HistoryExpired => ApplicationError::InvalidOperation(
                    "Analytics range is outside retained metric history; use a newer range or coarser complete-hour buckets"
                        .into(),
                ),
                other => other.into(),
            })?;

        if data.compatible_devices > MAX_DEVICES {
            return Err(ApplicationError::InvalidOperation(format!(
                "Analytics metric is compatible with {} devices in this scope; narrow it to {MAX_DEVICES} or fewer",
                data.compatible_devices
            )));
        }
        if data.buckets.len() > MAX_TOTAL_POINTS {
            return Err(ApplicationError::InvalidOperation(
                "Analytics query exceeds the point budget; use a larger bucket or narrower scope"
                    .to_string(),
            ));
        }

        Ok(build_result(
            request,
            metric,
            bucket_seconds,
            data.source,
            data,
        ))
    }
}

// For a microsecond store, ceil both ends to preserve the original [start, end)
// membership. The response and point-budget policy retain the requested bounds.
fn storage_bound(value: NaiveDateTime) -> Result<NaiveDateTime, ApplicationError> {
    let remainder = value.nanosecond() % 1_000;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add_signed(chrono::TimeDelta::nanoseconds(i64::from(1_000 - remainder)))
        .ok_or_else(|| {
            ApplicationError::InvalidInput("Analytics timestamp exceeds the supported range".into())
        })
}

fn resolve_metric(
    selector: AnalyticsMetricSelector,
    revisions: Vec<AnalyticsBlueprintRevision>,
) -> Result<(AnalyticsMetric, Vec<String>), ApplicationError> {
    let metric = metrics_from_blueprints(revisions.clone())?
        .into_iter()
        .find(|metric| metric.selector == selector)
        .ok_or_else(|| {
            ApplicationError::InvalidOperation(
                "Analytics metric is not a numeric field in the selected published blueprint"
                    .to_string(),
            )
        })?;
    let mut compatible_revision_ids = Vec::new();
    for revision in revisions {
        let revision_id = revision.revision_id.clone();
        if metrics_from_revision(revision)?
            .into_iter()
            .any(|candidate| {
                candidate.selector == selector
                    && candidate.value_type == metric.value_type
                    && candidate.unit == metric.unit
                    && {
                        let mut declared = candidate.aggregates;
                        let mut selected = metric.aggregates.clone();
                        declared.sort();
                        selected.sort();
                        declared == selected
                    }
            })
        {
            compatible_revision_ids.push(revision_id);
        }
    }
    compatible_revision_ids.sort();
    Ok((metric, compatible_revision_ids))
}

fn metrics_from_blueprints(
    revisions: Vec<AnalyticsBlueprintRevision>,
) -> Result<Vec<AnalyticsMetric>, ApplicationError> {
    let mut latest = HashMap::<String, AnalyticsBlueprintRevision>::new();
    for revision in revisions {
        let key = revision.blueprint_id.clone();
        if latest
            .get(&key)
            .is_none_or(|current| revision.revision > current.revision)
        {
            latest.insert(key, revision);
        }
    }
    let mut metrics = Vec::new();
    for revision in latest.into_values() {
        metrics.extend(metrics_from_revision(revision)?);
    }
    metrics.sort_by(|left, right| {
        left.blueprint_name
            .cmp(&right.blueprint_name)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.key().cmp(&right.key()))
    });
    Ok(metrics)
}

fn metrics_from_revision(
    revision: AnalyticsBlueprintRevision,
) -> Result<Vec<AnalyticsMetric>, ApplicationError> {
    let blueprint: DeviceBlueprint =
        serde_json::from_value(revision.document).map_err(|error| {
            ApplicationError::Internal(format!(
                "stored blueprint revision '{}' is invalid: {error}",
                revision.revision_id
            ))
        })?;
    let mut metrics = Vec::new();
    for stream in blueprint.spec.streams {
        for field in stream.fields {
            if !field.value_type.is_numeric() {
                continue;
            }
            metrics.push(AnalyticsMetric {
                selector: AnalyticsMetricSelector {
                    blueprint_id: revision.blueprint_id.clone(),
                    stream_key: stream.key.clone(),
                    field_path: field.path,
                },
                blueprint_key: revision.blueprint_key.clone(),
                blueprint_name: revision.blueprint_name.clone(),
                label: field.label,
                unit: field.unit,
                value_type: metric_value_type(field.value_type).to_string(),
                aggregates: field
                    .aggregates
                    .into_iter()
                    .map(|aggregate| aggregate_name(aggregate).to_string())
                    .collect(),
                precision: field
                    .presentation
                    .and_then(|presentation| presentation.precision),
            });
        }
    }
    Ok(metrics)
}

const fn metric_value_type(value_type: FieldValueType) -> &'static str {
    match value_type {
        FieldValueType::Float64 => "float64",
        FieldValueType::Int64 => "int64",
        FieldValueType::String => "string",
        FieldValueType::Boolean => "boolean",
        FieldValueType::Json => "json",
    }
}

const fn aggregate_name(aggregate: AggregateKind) -> &'static str {
    match aggregate {
        AggregateKind::Min => "minimum",
        AggregateKind::Max => "maximum",
        AggregateKind::Avg => "average",
        AggregateKind::Sum => "sum",
        AggregateKind::Count => "count",
        AggregateKind::Last => "latest",
    }
}

fn validate_scope(request: &AnalyticsRequest) -> Result<(), ApplicationError> {
    if request.scope.fleet_ids.len() > MAX_SCOPE_VALUES
        || request.scope.device_ids.len() > MAX_SCOPE_VALUES
    {
        return Err(ApplicationError::InvalidInput(format!(
            "Each analytics scope filter supports at most {MAX_SCOPE_VALUES} values"
        )));
    }
    Ok(())
}

fn validate_explicit_bucket(
    bucket_seconds: i64,
    range_seconds: i64,
    max_points: usize,
) -> Result<i64, ApplicationError> {
    if !ALLOWED_BUCKET_SECONDS.contains(&bucket_seconds) {
        return Err(ApplicationError::InvalidInput(format!(
            "Unsupported analytics bucket {bucket_seconds}; use 60, 300, 900, 3600, 21600, or 86400 seconds"
        )));
    }
    let estimated_points = ceil_div(range_seconds, bucket_seconds);
    if estimated_points > i64::try_from(max_points).unwrap_or(i64::MAX) {
        let suggested = automatic_bucket(range_seconds, max_points);
        return Err(ApplicationError::InvalidOperation(format!(
            "Requested bucket exceeds the per-series point budget; use at least {suggested} seconds"
        )));
    }
    Ok(bucket_seconds)
}

fn automatic_bucket(range_seconds: i64, max_points: usize) -> i64 {
    let target = ceil_div(range_seconds, i64::try_from(max_points).unwrap_or(i64::MAX));
    ALLOWED_BUCKET_SECONDS
        .iter()
        .copied()
        .find(|bucket| *bucket >= target)
        .unwrap_or(86_400)
}

fn build_result(
    request: AnalyticsRequest,
    metric: AnalyticsMetric,
    bucket_seconds: i64,
    source: AnalyticsDataSource,
    data: AnalyticsQueryData,
) -> AnalyticsResult {
    let expected_buckets = usize::try_from(ceil_div(
        (request.end - request.start).num_seconds(),
        bucket_seconds,
    ))
    .unwrap_or(usize::MAX)
    .max(1);
    let bucket_groups = group_by_time(&data.buckets);
    let coverage = bucket_groups
        .iter()
        .map(|(timestamp, buckets)| AnalyticsCoveragePoint {
            timestamp: *timestamp,
            reporting_devices: buckets.len(),
            selected_devices: data.compatible_devices,
        })
        .collect();
    let devices = build_device_stats(&data, expected_buckets);
    let series = match request.mode {
        AnalyticsSeriesMode::PerDevice => per_device_series(&data.buckets, false),
        AnalyticsSeriesMode::LatestRanking => per_device_series(&data.buckets, true),
        AnalyticsSeriesMode::FleetMean => aggregate_series(
            "fleet-mean",
            "Fleet mean",
            AnalyticsSeriesKind::Mean,
            &bucket_groups,
            request.weighting,
        )
        .into_iter()
        .collect(),
        AnalyticsSeriesMode::MeanAndRange => [
            aggregate_series(
                "fleet-minimum",
                "Fleet minimum",
                AnalyticsSeriesKind::Minimum,
                &bucket_groups,
                request.weighting,
            ),
            aggregate_series(
                "fleet-maximum",
                "Fleet maximum",
                AnalyticsSeriesKind::Maximum,
                &bucket_groups,
                request.weighting,
            ),
            aggregate_series(
                "fleet-mean",
                "Fleet mean",
                AnalyticsSeriesKind::Mean,
                &bucket_groups,
                request.weighting,
            ),
        ]
        .into_iter()
        .flatten()
        .collect(),
    };
    let stats = fleet_stats(&devices, request.weighting);

    AnalyticsResult {
        metric,
        start: request.start,
        end: request.end,
        bucket_seconds,
        source,
        weighting: request.weighting,
        selected_devices: data.selected_devices,
        compatible_devices: data.compatible_devices,
        series,
        devices,
        coverage,
        stats,
        warnings: Vec::new(),
    }
}

const fn ceil_div(value: i64, divisor: i64) -> i64 {
    value.saturating_add(divisor.saturating_sub(1)) / divisor
}

fn group_by_time(buckets: &[AnalyticsBucket]) -> BTreeMap<NaiveDateTime, Vec<&AnalyticsBucket>> {
    let mut groups = BTreeMap::new();
    for bucket in buckets {
        groups
            .entry(bucket.bucket_start)
            .or_insert_with(Vec::new)
            .push(bucket);
    }
    groups
}

fn build_device_stats(
    data: &AnalyticsQueryData,
    expected_buckets: usize,
) -> Vec<AnalyticsDeviceStats> {
    let mut buckets_by_device: HashMap<&str, Vec<&AnalyticsBucket>> = HashMap::new();
    for bucket in &data.buckets {
        buckets_by_device
            .entry(bucket.device_id.as_str())
            .or_default()
            .push(bucket);
    }

    data.devices
        .iter()
        .map(|device| {
            let buckets = buckets_by_device
                .get(device.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default();
            AnalyticsDeviceStats {
                device_id: device.id.clone(),
                device_name: device.name.clone(),
                stats: stats_for_buckets(buckets),
                coverage_percent: (buckets.len() as f64 / expected_buckets as f64 * 100.0)
                    .clamp(0.0, 100.0),
            }
        })
        .collect()
}

fn stats_for_buckets(buckets: &[&AnalyticsBucket]) -> Option<AnalyticsStats> {
    let first = *buckets.first()?;
    let mut minimum = first.minimum;
    let mut maximum = first.maximum;
    let mut weighted_sum = 0.0;
    let mut sample_count = 0_i64;
    let mut latest_bucket = first;
    for bucket in buckets {
        minimum = minimum.min(bucket.minimum);
        maximum = maximum.max(bucket.maximum);
        weighted_sum += bucket.average * bucket.sample_count as f64;
        sample_count += bucket.sample_count;
        if bucket.bucket_start > latest_bucket.bucket_start {
            latest_bucket = bucket;
        }
    }
    Some(AnalyticsStats {
        minimum,
        maximum,
        average: weighted_sum / sample_count.max(1) as f64,
        latest: latest_bucket.latest,
        sample_count,
    })
}

fn per_device_series(buckets: &[AnalyticsBucket], latest_only: bool) -> Vec<AnalyticsSeries> {
    let mut grouped: HashMap<&str, Vec<&AnalyticsBucket>> = HashMap::new();
    for bucket in buckets {
        grouped
            .entry(bucket.device_id.as_str())
            .or_default()
            .push(bucket);
    }
    let mut series: Vec<_> = grouped
        .into_values()
        .filter_map(|mut device_buckets| {
            device_buckets.sort_by_key(|bucket| bucket.bucket_start);
            let stats = stats_for_buckets(&device_buckets)?;
            let first = device_buckets.first()?;
            let points = if latest_only {
                device_buckets
                    .last()
                    .map(|bucket| {
                        vec![AnalyticsPoint {
                            timestamp: bucket.bucket_start,
                            value: bucket.latest,
                        }]
                    })
                    .unwrap_or_default()
            } else {
                device_buckets
                    .iter()
                    .map(|bucket| AnalyticsPoint {
                        timestamp: bucket.bucket_start,
                        value: bucket.average,
                    })
                    .collect()
            };
            Some(AnalyticsSeries {
                id: format!("device:{}", first.device_id),
                label: first.device_name.clone(),
                kind: AnalyticsSeriesKind::Device,
                device_id: Some(first.device_id.clone()),
                points,
                stats,
            })
        })
        .collect();
    series.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then_with(|| left.id.cmp(&right.id))
    });
    series
}

fn aggregate_series(
    id: &str,
    label: &str,
    kind: AnalyticsSeriesKind,
    groups: &BTreeMap<NaiveDateTime, Vec<&AnalyticsBucket>>,
    weighting: AnalyticsWeighting,
) -> Option<AnalyticsSeries> {
    let points: Vec<_> = groups
        .iter()
        .map(|(timestamp, buckets)| AnalyticsPoint {
            timestamp: *timestamp,
            value: match kind {
                AnalyticsSeriesKind::Mean => mean_for_buckets(buckets, weighting),
                AnalyticsSeriesKind::Minimum => buckets
                    .iter()
                    .map(|bucket| bucket.minimum)
                    .fold(f64::INFINITY, f64::min),
                AnalyticsSeriesKind::Maximum => buckets
                    .iter()
                    .map(|bucket| bucket.maximum)
                    .fold(f64::NEG_INFINITY, f64::max),
                AnalyticsSeriesKind::Device => unreachable!("device series are separate"),
            },
        })
        .collect();
    let first = *points.first()?;
    let minimum = points
        .iter()
        .map(|point| point.value)
        .fold(f64::INFINITY, f64::min);
    let maximum = points
        .iter()
        .map(|point| point.value)
        .fold(f64::NEG_INFINITY, f64::max);
    let average = points.iter().map(|point| point.value).sum::<f64>() / points.len() as f64;
    let latest = points.last().map_or(first.value, |point| point.value);
    Some(AnalyticsSeries {
        id: id.to_string(),
        label: label.to_string(),
        kind,
        device_id: None,
        points,
        stats: AnalyticsStats {
            minimum,
            maximum,
            average,
            latest,
            sample_count: i64::try_from(groups.len()).unwrap_or(i64::MAX),
        },
    })
}

fn mean_for_buckets(buckets: &[&AnalyticsBucket], weighting: AnalyticsWeighting) -> f64 {
    match weighting {
        AnalyticsWeighting::EqualDevice => {
            buckets.iter().map(|bucket| bucket.average).sum::<f64>() / buckets.len().max(1) as f64
        }
        AnalyticsWeighting::Sample => {
            let samples = buckets
                .iter()
                .map(|bucket| bucket.sample_count)
                .sum::<i64>();
            buckets
                .iter()
                .map(|bucket| bucket.average * bucket.sample_count as f64)
                .sum::<f64>()
                / samples.max(1) as f64
        }
    }
}

fn fleet_stats(
    devices: &[AnalyticsDeviceStats],
    weighting: AnalyticsWeighting,
) -> Option<AnalyticsStats> {
    let stats: Vec<_> = devices
        .iter()
        .filter_map(|device| device.stats.as_ref())
        .collect();
    let first = *stats.first()?;
    let sample_count = stats.iter().map(|stat| stat.sample_count).sum::<i64>();
    let average = match weighting {
        AnalyticsWeighting::EqualDevice => {
            stats.iter().map(|stat| stat.average).sum::<f64>() / stats.len() as f64
        }
        AnalyticsWeighting::Sample => {
            stats
                .iter()
                .map(|stat| stat.average * stat.sample_count as f64)
                .sum::<f64>()
                / sample_count.max(1) as f64
        }
    };
    Some(AnalyticsStats {
        minimum: stats
            .iter()
            .map(|stat| stat.minimum)
            .fold(first.minimum, f64::min),
        maximum: stats
            .iter()
            .map(|stat| stat.maximum)
            .fold(first.maximum, f64::max),
        average,
        latest: stats.iter().map(|stat| stat.latest).sum::<f64>() / stats.len() as f64,
        sample_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_uses_arbitrary_numeric_blueprint_fields() {
        let metrics = metrics_from_blueprints(vec![AnalyticsBlueprintRevision {
            blueprint_id: "power-meter".into(),
            blueprint_key: "three-phase-meter".into(),
            blueprint_name: "Three-phase meter".into(),
            revision_id: "power-meter-r1".into(),
            revision: 1,
            document: serde_json::json!({
                "apiVersion": "extrittio.io/v1alpha1",
                "kind": "DeviceBlueprint",
                "metadata": {"key": "three-phase-meter", "name": "Three-phase meter"},
                "spec": {
                    "runtime": {
                        "minimumContractApi": 1,
                        "heartbeat": {"interval": "30s", "offlineAfter": "95s"},
                        "limits": {"maxMessageBytes": 8192, "maxMessagesPerMinute": 120}
                    },
                    "streams": [{
                        "key": "electrical",
                        "route": "measurements",
                        "fields": [
                            {
                                "path": "/phase_a/voltage",
                                "type": "float64",
                                "label": "Phase A voltage",
                                "unit": "V",
                                "aggregates": ["min", "max", "avg"],
                                "presentation": {"precision": 2}
                            },
                            {
                                "path": "/status",
                                "type": "string",
                                "label": "Status",
                                "aggregates": ["last"]
                            }
                        ]
                    }]
                }
            }),
        }])
        .unwrap();

        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].key(), "electrical./phase_a/voltage");
        assert_eq!(metrics[0].unit.as_deref(), Some("V"));
        assert_eq!(metrics[0].precision, Some(2));
    }

    #[test]
    fn historical_revision_selection_excludes_changed_units_and_value_types() {
        let revision = |number, value_type: &str, unit: &str| AnalyticsBlueprintRevision {
            blueprint_id: "meter".into(),
            blueprint_key: "meter".into(),
            blueprint_name: "Meter".into(),
            revision_id: format!("revision-{number}"),
            revision: number,
            document: serde_json::json!({
                "apiVersion": "extrittio.io/v1alpha1",
                "kind": "DeviceBlueprint",
                "metadata": {"key": "meter", "name": "Meter"},
                "spec": {
                    "runtime": {
                        "minimumContractApi": 1,
                        "heartbeat": {"interval": "30s", "offlineAfter": "95s"},
                        "limits": {"maxMessageBytes": 8192, "maxMessagesPerMinute": 120}
                    },
                    "streams": [{"key": "readings", "route": "measurements", "fields": [{
                        "path": "/value", "type": value_type, "label": "Value",
                        "unit": unit, "aggregates": ["min", "max", "avg"]
                    }]}]
                }
            }),
        };
        let revisions = vec![
            revision(1, "float64", "V"),
            revision(2, "float64", "A"),
            revision(3, "int64", "V"),
            revision(4, "float64", "V"),
        ];
        assert_eq!(metrics_from_blueprints(revisions.clone()).unwrap().len(), 1);
        let (metric, compatible) = resolve_metric(
            AnalyticsMetricSelector {
                blueprint_id: "meter".into(),
                stream_key: "readings".into(),
                field_path: "/value".into(),
            },
            revisions,
        )
        .unwrap();
        assert_eq!(metric.unit.as_deref(), Some("V"));
        assert_eq!(metric.value_type, "float64");
        assert_eq!(compatible, ["revision-1", "revision-4"]);
    }

    #[test]
    fn equal_device_mean_does_not_favor_faster_publishers() {
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .unwrap()
            .naive_utc();
        let slow = AnalyticsBucket {
            device_id: "slow".into(),
            device_name: "Slow".into(),
            bucket_start: now,
            sample_count: 1,
            average: 10.0,
            minimum: 10.0,
            maximum: 10.0,
            latest: 10.0,
        };
        let fast = AnalyticsBucket {
            device_id: "fast".into(),
            device_name: "Fast".into(),
            bucket_start: now,
            sample_count: 100,
            average: 20.0,
            minimum: 20.0,
            maximum: 20.0,
            latest: 20.0,
        };

        assert_eq!(
            mean_for_buckets(&[&slow, &fast], AnalyticsWeighting::EqualDevice),
            15.0
        );
        assert!(mean_for_buckets(&[&slow, &fast], AnalyticsWeighting::Sample) > 19.8);
    }

    #[test]
    fn automatic_bucket_stays_within_the_point_budget() {
        assert_eq!(automatic_bucket(24 * 60 * 60, 1_200), 300);
        assert_eq!(automatic_bucket(30 * 24 * 60 * 60, 1_200), 3_600);
    }
}
