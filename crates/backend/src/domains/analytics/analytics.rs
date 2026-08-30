use std::sync::Arc;

use axum::{Extension, Json, Router, extract::State, routing::get};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::state::AppState;
use crate::util;

use crate::domains::analytics::analytics_service;
use crate::domains::analytics::types::{
    AnalyticsDataSource, AnalyticsMetric, AnalyticsMetricSelector, AnalyticsRequest,
    AnalyticsResult, AnalyticsScope, AnalyticsSeriesKind, AnalyticsSeriesMode, AnalyticsWeighting,
};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AnalyticsMetricRequest {
    pub blueprint_id: String,
    pub stream_key: String,
    pub field_path: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsSeriesModeName {
    PerDevice,
    FleetMean,
    MeanAndRange,
    LatestRanking,
}

impl From<AnalyticsSeriesModeName> for AnalyticsSeriesMode {
    fn from(value: AnalyticsSeriesModeName) -> Self {
        match value {
            AnalyticsSeriesModeName::PerDevice => Self::PerDevice,
            AnalyticsSeriesModeName::FleetMean => Self::FleetMean,
            AnalyticsSeriesModeName::MeanAndRange => Self::MeanAndRange,
            AnalyticsSeriesModeName::LatestRanking => Self::LatestRanking,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsWeightingName {
    EqualDevice,
    Sample,
}

impl From<AnalyticsWeightingName> for AnalyticsWeighting {
    fn from(value: AnalyticsWeightingName) -> Self {
        match value {
            AnalyticsWeightingName::EqualDevice => Self::EqualDevice,
            AnalyticsWeightingName::Sample => Self::Sample,
        }
    }
}

fn default_series_mode() -> AnalyticsSeriesModeName {
    AnalyticsSeriesModeName::MeanAndRange
}

fn default_weighting() -> AnalyticsWeightingName {
    AnalyticsWeightingName::EqualDevice
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct AnalyticsScopeRequest {
    #[serde(default)]
    pub device_type_ids: Vec<i32>,
    #[serde(default)]
    pub fleet_ids: Vec<i32>,
    #[serde(default)]
    pub device_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AnalyticsQueryRequest {
    #[serde(default)]
    pub scope: AnalyticsScopeRequest,
    pub metric: AnalyticsMetricRequest,
    /// Inclusive RFC 3339 range start.
    pub from: String,
    /// Exclusive RFC 3339 range end.
    pub to: String,
    /// Null selects an automatic bounded bucket.
    pub bucket_seconds: Option<i64>,
    #[serde(default = "default_series_mode")]
    pub mode: AnalyticsSeriesModeName,
    #[serde(default = "default_weighting")]
    pub weighting: AnalyticsWeightingName,
    pub max_points_per_series: Option<usize>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsMetricCatalogEntry {
    pub key: String,
    pub blueprint_id: String,
    pub blueprint_key: String,
    pub blueprint_name: String,
    pub stream_key: String,
    pub field_path: String,
    pub label: String,
    pub unit: Option<String>,
    pub value_type: String,
    pub aggregates: Vec<String>,
    pub series_modes: Vec<String>,
    pub precision: Option<u8>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsCatalogResponse {
    pub metrics: Vec<AnalyticsMetricCatalogEntry>,
    pub bucket_seconds: Vec<i64>,
    pub max_devices_per_query: usize,
    pub max_points_per_series: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsMetricResponse {
    pub key: String,
    pub blueprint_id: String,
    pub blueprint_key: String,
    pub blueprint_name: String,
    pub stream_key: String,
    pub field_path: String,
    pub label: String,
    pub unit: Option<String>,
    pub value_type: String,
    pub precision: Option<u8>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsEffectiveResponse {
    pub from: String,
    pub to: String,
    pub bucket_seconds: i64,
    pub source: String,
    pub weighting: String,
    pub fill: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsScopeResponse {
    pub selected_devices: usize,
    pub compatible_devices: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsStatsResponse {
    pub minimum: f64,
    pub maximum: f64,
    pub average: f64,
    pub latest: f64,
    pub sample_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsPointResponse {
    pub timestamp_ms: i64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsSeriesResponse {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub device_id: Option<String>,
    pub points: Vec<AnalyticsPointResponse>,
    pub stats: AnalyticsStatsResponse,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsDeviceStatsResponse {
    pub device_id: String,
    pub device_name: String,
    pub stats: Option<AnalyticsStatsResponse>,
    pub coverage_percent: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsCoverageResponse {
    pub timestamp_ms: i64,
    pub reporting_devices: usize,
    pub selected_devices: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AnalyticsQueryResponse {
    pub metric: AnalyticsMetricResponse,
    pub effective: AnalyticsEffectiveResponse,
    pub scope: AnalyticsScopeResponse,
    pub series: Vec<AnalyticsSeriesResponse>,
    pub devices: Vec<AnalyticsDeviceStatsResponse>,
    pub coverage: Vec<AnalyticsCoverageResponse>,
    pub stats: Option<AnalyticsStatsResponse>,
    pub warnings: Vec<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/analytics/catalog", get(get_catalog))
        .route("/api/v1/analytics/query", axum::routing::post(run_query))
}

/// Get numeric metrics declared by the tenant's latest published device blueprints.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/catalog",
    tag = "analytics",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "Analytics metric catalog", body = AnalyticsCatalogResponse)),
)]
pub(crate) async fn get_catalog(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<AnalyticsCatalogResponse>, AppError> {
    let metrics = analytics_service::catalog(&ctx, state.persistence.analytics.as_ref())
        .await?
        .into_iter()
        .map(metric_catalog_entry)
        .collect();
    Ok(Json(AnalyticsCatalogResponse {
        metrics,
        bucket_seconds: vec![60, 300, 900, 3_600, 21_600, 86_400],
        max_devices_per_query: 50,
        max_points_per_series: 2_000,
    }))
}

/// Run a bounded tenant-scoped telemetry analytics query.
#[utoipa::path(
    post,
    path = "/api/v1/analytics/query",
    tag = "analytics",
    security(("bearer_auth" = [])),
    request_body = AnalyticsQueryRequest,
    responses(
        (status = 200, description = "Chart-ready analytics result", body = AnalyticsQueryResponse),
        (status = 400, description = "Invalid query"),
        (status = 422, description = "Query exceeds a cost or compatibility limit"),
    ),
)]
pub(crate) async fn run_query(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<AnalyticsQueryRequest>,
) -> Result<Json<AnalyticsQueryResponse>, AppError> {
    let from = util::parse_timestamp(Some(&body.from))?
        .ok_or_else(|| AppError::BadRequest("Analytics from timestamp is required".into()))?;
    let to = util::parse_timestamp(Some(&body.to))?
        .ok_or_else(|| AppError::BadRequest("Analytics to timestamp is required".into()))?;
    let result = analytics_service::query(
        &ctx,
        state.persistence.analytics.as_ref(),
        AnalyticsRequest {
            scope: AnalyticsScope {
                device_type_ids: body.scope.device_type_ids,
                fleet_ids: body.scope.fleet_ids,
                device_ids: body.scope.device_ids,
            },
            metric: AnalyticsMetricSelector {
                blueprint_id: body.metric.blueprint_id,
                stream_key: body.metric.stream_key,
                field_path: body.metric.field_path,
            },
            start: from,
            end: to,
            bucket_seconds: body.bucket_seconds,
            mode: body.mode.into(),
            weighting: body.weighting.into(),
            max_points_per_series: body.max_points_per_series,
        },
    )
    .await?;
    Ok(Json(result.into()))
}

impl From<AnalyticsResult> for AnalyticsQueryResponse {
    fn from(result: AnalyticsResult) -> Self {
        Self {
            metric: metric_response(result.metric),
            effective: AnalyticsEffectiveResponse {
                from: result.start.and_utc().to_rfc3339(),
                to: result.end.and_utc().to_rfc3339(),
                bucket_seconds: result.bucket_seconds,
                source: match result.source {
                    AnalyticsDataSource::BlueprintMetricSamples => "blueprint_metric_samples",
                }
                .to_string(),
                weighting: match result.weighting {
                    AnalyticsWeighting::EqualDevice => "equal_device",
                    AnalyticsWeighting::Sample => "sample",
                }
                .to_string(),
                fill: "none".to_string(),
            },
            scope: AnalyticsScopeResponse {
                selected_devices: result.selected_devices,
                compatible_devices: result.compatible_devices,
            },
            series: result
                .series
                .into_iter()
                .map(|series| AnalyticsSeriesResponse {
                    id: series.id,
                    label: series.label,
                    kind: series_kind(series.kind).to_string(),
                    device_id: series.device_id,
                    points: series
                        .points
                        .into_iter()
                        .map(|point| AnalyticsPointResponse {
                            timestamp_ms: point.timestamp.and_utc().timestamp_millis(),
                            value: point.value,
                        })
                        .collect(),
                    stats: series.stats.into(),
                })
                .collect(),
            devices: result
                .devices
                .into_iter()
                .map(|device| AnalyticsDeviceStatsResponse {
                    device_id: device.device_id,
                    device_name: device.device_name,
                    stats: device.stats.map(Into::into),
                    coverage_percent: device.coverage_percent,
                })
                .collect(),
            coverage: result
                .coverage
                .into_iter()
                .map(|point| AnalyticsCoverageResponse {
                    timestamp_ms: point.timestamp.and_utc().timestamp_millis(),
                    reporting_devices: point.reporting_devices,
                    selected_devices: point.selected_devices,
                })
                .collect(),
            stats: result.stats.map(Into::into),
            warnings: result.warnings,
        }
    }
}

fn metric_catalog_entry(metric: AnalyticsMetric) -> AnalyticsMetricCatalogEntry {
    AnalyticsMetricCatalogEntry {
        key: metric.key(),
        blueprint_id: metric.selector.blueprint_id,
        blueprint_key: metric.blueprint_key,
        blueprint_name: metric.blueprint_name,
        stream_key: metric.selector.stream_key,
        field_path: metric.selector.field_path,
        label: metric.label,
        unit: metric.unit,
        value_type: metric.value_type,
        aggregates: metric.aggregates,
        series_modes: vec![
            "per_device".into(),
            "fleet_mean".into(),
            "mean_and_range".into(),
            "latest_ranking".into(),
        ],
        precision: metric.precision,
    }
}

fn metric_response(metric: AnalyticsMetric) -> AnalyticsMetricResponse {
    AnalyticsMetricResponse {
        key: metric.key(),
        blueprint_id: metric.selector.blueprint_id,
        blueprint_key: metric.blueprint_key,
        blueprint_name: metric.blueprint_name,
        stream_key: metric.selector.stream_key,
        field_path: metric.selector.field_path,
        label: metric.label,
        unit: metric.unit,
        value_type: metric.value_type,
        precision: metric.precision,
    }
}

impl From<crate::domains::analytics::types::AnalyticsStats> for AnalyticsStatsResponse {
    fn from(stats: crate::domains::analytics::types::AnalyticsStats) -> Self {
        Self {
            minimum: stats.minimum,
            maximum: stats.maximum,
            average: stats.average,
            latest: stats.latest,
            sample_count: stats.sample_count,
        }
    }
}

const fn series_kind(kind: AnalyticsSeriesKind) -> &'static str {
    match kind {
        AnalyticsSeriesKind::Device => "device",
        AnalyticsSeriesKind::Mean => "mean",
        AnalyticsSeriesKind::Minimum => "minimum",
        AnalyticsSeriesKind::Maximum => "maximum",
    }
}
