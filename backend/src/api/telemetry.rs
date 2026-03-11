use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::db::models::TelemetryRecord;
use crate::error::AppError;
use crate::repositories::{device_repo, telemetry_repo};
use crate::state::{AppState, run_db};

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
    // Parse `since` filter before entering the blocking closure
    let since = parse_since(params.since.as_deref())?;

    let response = run_db(&state.db_pool, move |conn| {
        // Verify device exists (404 if not)
        device_repo::find_device(conn, &id)?;

        // Determine limit (default 50, max 1000)
        let limit = params.limit.unwrap_or(50).clamp(1, 1000);

        let results = telemetry_repo::list_telemetry(conn, &id, since, limit)?;

        Ok(results.into_iter().map(TelemetryResponse::from).collect())
    })
    .await?;

    Ok(Json(response))
}

/// Parse an optional `since` timestamp string, accepting both NaiveDateTime
/// and RFC 3339 formats.
fn parse_since(since_str: Option<&str>) -> Result<Option<NaiveDateTime>, AppError> {
    let Some(s) = since_str else {
        return Ok(None);
    };
    let dt = s
        .parse::<NaiveDateTime>()
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(s).map(|dt| dt.naive_utc()))
        .map_err(|_| {
            AppError::BadRequest("Invalid date format, expected YYYY-MM-DDTHH:MM:SS".into())
        })?;
    Ok(Some(dt))
}
