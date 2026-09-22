use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsMetricSelector {
    pub blueprint_id: String,
    pub stream_key: String,
    pub field_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsMetric {
    pub selector: AnalyticsMetricSelector,
    pub blueprint_key: String,
    pub blueprint_name: String,
    pub label: String,
    pub unit: Option<String>,
    pub value_type: String,
    pub aggregates: Vec<String>,
    pub precision: Option<u8>,
}

impl AnalyticsMetric {
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}.{}", self.selector.stream_key, self.selector.field_path)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsBlueprintRevision {
    pub blueprint_id: String,
    pub blueprint_key: String,
    pub blueprint_name: String,
    pub revision_id: String,
    pub revision: i32,
    pub document: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnalyticsScope {
    pub fleet_ids: Vec<i32>,
    pub device_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsSeriesMode {
    PerDevice,
    FleetMean,
    MeanAndRange,
    LatestRanking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsWeighting {
    EqualDevice,
    Sample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsDataSource {
    BlueprintMetricSamples,
    BlueprintMetricSamplesAndRollups,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsRequest {
    pub scope: AnalyticsScope,
    pub metric: AnalyticsMetricSelector,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub bucket_seconds: Option<i64>,
    pub mode: AnalyticsSeriesMode,
    pub weighting: AnalyticsWeighting,
    pub max_points_per_series: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsQuery {
    pub scope: AnalyticsScope,
    pub metric: AnalyticsMetric,
    /// Published revisions whose exact field declaration matches the selected
    /// metric's type, unit and aggregate semantics.
    pub compatible_revision_ids: Vec<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub bucket_seconds: i64,
    pub max_devices: usize,
    pub max_rows: usize,
}

/// Full UTC hours entirely inside [start, end) and before the current hour.
/// Partial and current hours must be read from raw samples.
#[must_use]
pub fn full_hour_rollup_window(
    start: NaiveDateTime,
    end: NaiveDateTime,
    now: NaiveDateTime,
) -> Option<(NaiveDateTime, NaiveDateTime)> {
    const HOUR_MICROS: i64 = 3_600_000_000;
    let start_us = start.and_utc().timestamp_micros();
    let end_us = end
        .and_utc()
        .timestamp_micros()
        .min(now.and_utc().timestamp_micros());
    let first = start_us
        .div_euclid(HOUR_MICROS)
        .checked_add(i64::from(start_us.rem_euclid(HOUR_MICROS) != 0))?
        .checked_mul(HOUR_MICROS)?;
    let last = end_us.div_euclid(HOUR_MICROS).checked_mul(HOUR_MICROS)?;
    if first >= last {
        return None;
    }
    Some((
        DateTime::from_timestamp_micros(first)?.naive_utc(),
        DateTime::from_timestamp_micros(last)?.naive_utc(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsBucket {
    pub device_id: String,
    pub device_name: String,
    pub bucket_start: NaiveDateTime,
    pub sample_count: i64,
    pub average: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub latest: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsQueryData {
    pub source: AnalyticsDataSource,
    pub selected_devices: usize,
    pub compatible_devices: usize,
    pub devices: Vec<AnalyticsDevice>,
    pub buckets: Vec<AnalyticsBucket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsSeriesKind {
    Device,
    Mean,
    Minimum,
    Maximum,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalyticsPoint {
    pub timestamp: NaiveDateTime,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsStats {
    pub minimum: f64,
    pub maximum: f64,
    pub average: f64,
    pub latest: f64,
    pub sample_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsSeries {
    pub id: String,
    pub label: String,
    pub kind: AnalyticsSeriesKind,
    pub device_id: Option<String>,
    pub points: Vec<AnalyticsPoint>,
    pub stats: AnalyticsStats,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsDeviceStats {
    pub device_id: String,
    pub device_name: String,
    pub stats: Option<AnalyticsStats>,
    pub coverage_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsCoveragePoint {
    pub timestamp: NaiveDateTime,
    pub reporting_devices: usize,
    pub selected_devices: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsResult {
    pub metric: AnalyticsMetric,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub bucket_seconds: i64,
    pub source: AnalyticsDataSource,
    pub weighting: AnalyticsWeighting,
    pub selected_devices: usize,
    pub compatible_devices: usize,
    pub series: Vec<AnalyticsSeries>,
    pub devices: Vec<AnalyticsDeviceStats>,
    pub coverage: Vec<AnalyticsCoveragePoint>,
    pub stats: Option<AnalyticsStats>,
    pub warnings: Vec<String>,
}

#[async_trait]
pub trait AnalyticsRepository: Send + Sync {
    /// Returns every published revision, including historical revisions needed
    /// to validate the semantics of stored samples and rollups.
    async fn blueprint_catalog(
        &self,
        tenant: &TenantId,
    ) -> Result<Vec<AnalyticsBlueprintRevision>, PersistenceError>;

    /// Read scope counts, compatible devices, and metric buckets in one snapshot.
    /// The preceding catalog lookup is independent and resolves immutable selector
    /// metadata; it does not pin the catalog and sample read to one transaction.
    /// Bounds are microsecond [start, end); bucket labels are UTC epoch floors.
    /// Device/page ties and latest-event ties use binary text ordering.
    async fn query(
        &self,
        tenant: &TenantId,
        query: AnalyticsQuery,
    ) -> Result<AnalyticsQueryData, PersistenceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollup_window_excludes_partial_and_current_hours() {
        let at = |seconds| DateTime::from_timestamp(seconds, 0).unwrap().naive_utc();
        assert_eq!(
            full_hour_rollup_window(at(1), at(4 * 3_600 + 1), at(4 * 3_600 + 30)),
            Some((at(3_600), at(4 * 3_600)))
        );
        assert_eq!(
            full_hour_rollup_window(at(3_600), at(7_200), at(5_000)),
            None
        );
        assert_eq!(
            full_hour_rollup_window(at(-3_600), at(3_600), at(7_200)),
            Some((at(-3_600), at(3_600)))
        );
    }
}
