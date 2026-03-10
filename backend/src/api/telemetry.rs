use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::TelemetryRecord;
use crate::error::AppError;
use crate::repositories::{device_repo, telemetry_repo};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TelemetryResponse {
    pub id: i32,
    pub device_id: String,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    pub custom_json: Option<String>,
    pub received_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TelemetryQuery {
    pub limit: Option<i64>,
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
    Router::new().route("/api/devices/{id}/telemetry", get(get_device_telemetry))
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

async fn get_device_telemetry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<TelemetryQuery>,
) -> Result<Json<Vec<TelemetryResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists (404 if not)
    device_repo::find_device(&mut conn, &id)?;

    // Determine limit (default 50, max 1000)
    let limit = params.limit.unwrap_or(50).clamp(1, 1000);

    // Parse `since` filter if provided
    let since = if let Some(ref since_str) = params.since {
        let since_dt = since_str
            .parse::<NaiveDateTime>()
            .or_else(|_| {
                chrono::DateTime::parse_from_rfc3339(since_str).map(|dt| dt.naive_utc())
            })
            .map_err(|_| AppError::BadRequest("Invalid date format, expected YYYY-MM-DDTHH:MM:SS".into()))?;
        Some(since_dt)
    } else {
        None
    };

    let results = telemetry_repo::list_telemetry(&mut conn, &id, since, limit)?;

    let response: Vec<TelemetryResponse> =
        results.into_iter().map(TelemetryResponse::from).collect();

    Ok(Json(response))
}
