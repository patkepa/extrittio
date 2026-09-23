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
    /// Filter by the stream key recorded under each event's originating contract.
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
    /// Decimal text preserves values outside JavaScript's safe integer range.
    Int64(String),
    String(String),
    Boolean(bool),
    #[schema(value_type = Object)]
    Json(serde_json::Value),
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceMetricResponse {
    pub contract_id: String,
    pub blueprint_id: String,
    pub blueprint_name: String,
    pub blueprint_revision_id: String,
    pub blueprint_revision: i32,
    pub event_id: String,
    pub device_id: String,
    pub stream_key: String,
    pub field_path: String,
    pub field_label: String,
    pub field_unit: Option<String>,
    pub field_semantic: Option<String>,
    pub field_presentation: Option<MetricFieldPresentationResponse>,
    pub value_type: String,
    pub value: MetricValueResponse,
    pub occurred_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MetricFieldPresentationResponse {
    pub color: Option<String>,
    pub chart: Option<MetricChartKindResponse>,
    pub precision: Option<u8>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricChartKindResponse {
    Line,
    Step,
    Bar,
    None,
}

impl From<extrittio_device_contract::FieldPresentation> for MetricFieldPresentationResponse {
    fn from(value: extrittio_device_contract::FieldPresentation) -> Self {
        use extrittio_device_contract::ChartKind;
        Self {
            color: value.color,
            chart: value.chart.map(|chart| match chart {
                ChartKind::Line => MetricChartKindResponse::Line,
                ChartKind::Step => MetricChartKindResponse::Step,
                ChartKind::Bar => MetricChartKindResponse::Bar,
                ChartKind::None => MetricChartKindResponse::None,
            }),
            precision: value.precision,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl From<DeviceMetricRecord> for DeviceMetricResponse {
    fn from(record: DeviceMetricRecord) -> Self {
        let value_type = record.value.value_type().to_owned();
        let value = match record.value {
            MetricValue::Float64(value) => MetricValueResponse::Float64(value),
            MetricValue::Int64(value) => MetricValueResponse::Int64(value.to_string()),
            MetricValue::String(value) => MetricValueResponse::String(value),
            MetricValue::Boolean(value) => MetricValueResponse::Boolean(value),
            MetricValue::Json(value) => MetricValueResponse::Json(value),
        };
        Self {
            event_id: record.event_id,
            contract_id: record.contract_id,
            blueprint_id: record.blueprint_id,
            blueprint_name: record.blueprint_name,
            blueprint_revision_id: record.blueprint_revision_id,
            blueprint_revision: record.blueprint_revision,
            device_id: record.device_id,
            stream_key: record.stream_key,
            field_path: record.field_path,
            field_label: record.field.label,
            field_unit: record.field.unit,
            field_semantic: record.field.semantic,
            field_presentation: record.field.presentation.map(Into::into),
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

/// Get typed metric samples with each event's originating contract and field metadata.
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

#[cfg(test)]
mod tests {
    use super::*;
    use extrittio_backend_core::events::DeviceMetricFieldMetadata;

    #[test]
    fn history_response_preserves_large_integer_and_origin_metadata() {
        let response = DeviceMetricResponse::from(DeviceMetricRecord {
            contract_id: "contract".into(),
            blueprint_id: "blueprint".into(),
            blueprint_name: "Counter".into(),
            blueprint_revision_id: "revision".into(),
            blueprint_revision: 2,
            event_id: "event".into(),
            device_id: "device".into(),
            stream_key: "readings".into(),
            field_path: "/count".into(),
            field: DeviceMetricFieldMetadata {
                label: "Count".into(),
                unit: Some("ticks".into()),
                semantic: None,
                presentation: None,
            },
            value: MetricValue::Int64(9_007_199_254_740_993),
            occurred_at: chrono::DateTime::from_timestamp(1, 0).unwrap(),
        });
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["value"], "9007199254740993");
        assert_eq!(json["blueprint_revision_id"], "revision");
        assert_eq!(json["field_unit"], "ticks");
    }
}
