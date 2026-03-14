use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::db::models::TelemetryRecord;
use crate::error::AppError;
use crate::services::telemetry_service;
use crate::state::{AppState, run_db};
use crate::util;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct TelemetryResponse {
    pub id: i32,
    pub device_id: String,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    pub custom_json: Option<String>,
    pub received_at: String,
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
            received_at: r.received_at.and_utc().to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/devices/{id}/telemetry", get(get_device_telemetry))
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
    Path(id): Path<String>,
    Query(params): Query<TelemetryQuery>,
) -> Result<Json<Vec<TelemetryResponse>>, AppError> {
    // Parse timestamp filters before entering the blocking closure
    let since = util::parse_timestamp(params.since.as_deref())?;
    let before = util::parse_timestamp(params.before.as_deref())?;

    let response = run_db(&state.db_pool, move |conn| {
        let limit = params.limit.unwrap_or(50).clamp(1, 1000);

        let results = telemetry_service::list(conn, &id, since, before, limit)?;

        Ok(results.into_iter().map(TelemetryResponse::from).collect())
    })
    .await?;

    Ok(Json(response))
}
