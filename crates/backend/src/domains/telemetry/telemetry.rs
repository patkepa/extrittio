use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::state::AppState;
use crate::util;
use extrittio_backend_core::events::{
    DeviceMetricQuery as PortDeviceMetricQuery, DeviceMetricRecord, MetricValue,
};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

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
    pub contract_id: String,
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
            contract_id: record.contract_id,
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
    Router::new().route("/api/v1/devices/{id}/metrics", get(get_device_metrics))
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
