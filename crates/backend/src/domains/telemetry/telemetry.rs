use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::domains::events::types::{
    DeviceMetricQuery as PortDeviceMetricQuery, DeviceMetricRecord, MetricValue,
};
use crate::domains::telemetry::types::{
    TelemetryQuery as PortTelemetryQuery, TelemetryRecord, TelemetryRollup,
};
use crate::error::AppError;
use crate::state::AppState;
use crate::util;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct TelemetryResponse {
    pub id: i64,
    pub device_id: String,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    #[schema(value_type = Option<Object>)]
    pub custom_json: Option<serde_json::Value>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub speed: Option<f32>,
    pub altitude: Option<f32>,
    pub heading: Option<f32>,
    pub received_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HourlyTelemetryResponse {
    pub device_id: String,
    pub bucket_start: String,
    pub sample_count: i64,
    pub avg_temperature: Option<f32>,
    pub min_temperature: Option<f32>,
    pub max_temperature: Option<f32>,
    pub avg_humidity: Option<f32>,
    pub min_humidity: Option<f32>,
    pub max_humidity: Option<f32>,
    pub avg_battery_level: Option<f32>,
    pub min_battery_level: Option<f32>,
    pub max_battery_level: Option<f32>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct TelemetryQuery {
    /// Maximum number of records to return (default 50, max 1000).
    pub limit: Option<i64>,
    /// Only return records after this timestamp (RFC 3339 or YYYY-MM-DDTHH:MM:SS).
    pub since: Option<String>,
    /// Only return records before this timestamp (RFC 3339 or YYYY-MM-DDTHH:MM:SS).
    pub before: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeviceMetricQuery {
    /// Filter by the stream key declared in the assigned device contract.
    pub stream_key: Option<String>,
    /// Filter by the JSON pointer field path declared in the stream.
    pub field_path: Option<String>,
    /// Only return samples at or after this timestamp.
    pub since: Option<String>,
    /// Only return samples before this timestamp.
    pub before: Option<String>,
    /// Maximum number of samples to return (default 1000, max 10000).
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
pub enum MetricValueResponse {
    Float64(f64),
    Int64(i64),
    String(String),
    Boolean(bool),
    #[schema(value_type = Object)]
    Json(serde_json::Value),
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceMetricResponse {
    pub event_id: String,
    pub device_id: String,
    pub stream_key: String,
    pub field_path: String,
    pub value_type: String,
    pub value: MetricValueResponse,
    pub occurred_at: String,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl From<TelemetryRecord> for TelemetryResponse {
    fn from(r: TelemetryRecord) -> Self {
        Self {
            id: r.id,
            device_id: r.device_id,
            temperature: r.temperature,
            humidity: r.humidity,
            battery_level: r.battery_level,
            custom_json: r.custom_json,
            latitude: r.latitude,
            longitude: r.longitude,
            speed: r.speed,
            altitude: r.altitude,
            heading: r.heading,
            received_at: r.received_at.and_utc().to_rfc3339(),
        }
    }
}

impl From<TelemetryRollup> for HourlyTelemetryResponse {
    fn from(rollup: TelemetryRollup) -> Self {
        Self {
            device_id: rollup.device_id,
            bucket_start: rollup.bucket_start.and_utc().to_rfc3339(),
            sample_count: rollup.sample_count,
            avg_temperature: rollup.avg_temperature,
            min_temperature: rollup.min_temperature,
            max_temperature: rollup.max_temperature,
            avg_humidity: rollup.avg_humidity,
            min_humidity: rollup.min_humidity,
            max_humidity: rollup.max_humidity,
            avg_battery_level: rollup.avg_battery_level,
            min_battery_level: rollup.min_battery_level,
            max_battery_level: rollup.max_battery_level,
        }
    }
}

impl From<DeviceMetricRecord> for DeviceMetricResponse {
    fn from(record: DeviceMetricRecord) -> Self {
        let value_type = record.value.value_type().to_owned();
        let value = match record.value {
            MetricValue::Float64(value) => MetricValueResponse::Float64(value),
            MetricValue::Int64(value) => MetricValueResponse::Int64(value),
            MetricValue::String(value) => MetricValueResponse::String(value),
            MetricValue::Boolean(value) => MetricValueResponse::Boolean(value),
            MetricValue::Json(value) => MetricValueResponse::Json(value),
        };
        Self {
            event_id: record.event_id,
            device_id: record.device_id,
            stream_key: record.stream_key,
            field_path: record.field_path,
            value_type,
            value,
            occurred_at: record.occurred_at.to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/devices/{id}/telemetry", get(get_device_telemetry))
        .route("/api/v1/devices/{id}/metrics", get(get_device_metrics))
        .route(
            "/api/v1/devices/{id}/telemetry/latest",
            get(get_latest_device_telemetry),
        )
        .route(
            "/api/v1/devices/{id}/telemetry/hourly",
            get(get_hourly_device_telemetry),
        )
}

/// Get typed metric samples extracted according to the device's assigned contract.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/metrics",
    tag = "telemetry",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Device ID"),
        DeviceMetricQuery,
    ),
    responses(
        (status = 200, description = "Contract-defined metric samples", body = Vec<DeviceMetricResponse>),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn get_device_metrics(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(params): Query<DeviceMetricQuery>,
) -> Result<Json<Vec<DeviceMetricResponse>>, AppError> {
    let since = util::parse_timestamp(params.since.as_deref())?;
    let before = util::parse_timestamp(params.before.as_deref())?;
    let records = state
        .application()
        .events()
        .list_metrics(
            &ctx.tenant_context(),
            &id,
            PortDeviceMetricQuery {
                stream_key: params.stream_key,
                field_path: params.field_path,
                since,
                before,
                limit: params.limit.unwrap_or(1000),
            },
        )
        .await?;
    Ok(Json(
        records
            .into_iter()
            .map(DeviceMetricResponse::from)
            .collect(),
    ))
}

/// Get the most recently received telemetry sample for a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/telemetry/latest",
    tag = "telemetry",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Latest telemetry sample", body = TelemetryResponse),
        (status = 404, description = "Device or telemetry sample not found"),
    ),
)]
pub(crate) async fn get_latest_device_telemetry(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<TelemetryResponse>, AppError> {
    let response = state
        .application()
        .telemetry()
        .latest(&ctx.tenant_context(), &id)
        .await?
        .map(TelemetryResponse::from)
        .ok_or_else(|| AppError::NotFound(format!("No telemetry found for device {id}")))?;

    Ok(Json(response))
}

/// Get hourly telemetry aggregates for a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/telemetry/hourly",
    tag = "telemetry",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Device ID"),
        TelemetryQuery,
    ),
    responses(
        (status = 200, description = "Hourly telemetry aggregates", body = Vec<HourlyTelemetryResponse>),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn get_hourly_device_telemetry(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(params): Query<TelemetryQuery>,
) -> Result<Json<Vec<HourlyTelemetryResponse>>, AppError> {
    let since = util::parse_timestamp(params.since.as_deref())?;
    let before = util::parse_timestamp(params.before.as_deref())?;
    let limit = params.limit.unwrap_or(168);

    let results = state
        .application()
        .telemetry()
        .list_hourly(
            &ctx.tenant_context(),
            &id,
            PortTelemetryQuery {
                since,
                before,
                limit,
            },
        )
        .await?;
    let response = results
        .into_iter()
        .map(HourlyTelemetryResponse::from)
        .collect();

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Get telemetry data for a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/telemetry",
    tag = "telemetry",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Device ID"),
        TelemetryQuery,
    ),
    responses(
        (status = 200, description = "Telemetry records", body = Vec<TelemetryResponse>),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn get_device_telemetry(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(params): Query<TelemetryQuery>,
) -> Result<Json<Vec<TelemetryResponse>>, AppError> {
    // Parse timestamp filters before entering the blocking closure
    let since = util::parse_timestamp(params.since.as_deref())?;
    let before = util::parse_timestamp(params.before.as_deref())?;

    let results = state
        .application()
        .telemetry()
        .list(
            &ctx.tenant_context(),
            &id,
            PortTelemetryQuery {
                since,
                before,
                limit: params.limit.unwrap_or(50),
            },
        )
        .await?;
    let response = results.into_iter().map(TelemetryResponse::from).collect();

    Ok(Json(response))
}
