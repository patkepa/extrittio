use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::TelemetryRecord;
use crate::db::schema::{devices, telemetry};
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
) -> Result<Json<Vec<TelemetryResponse>>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify device exists (404 if not)
    let _device: crate::db::models::Device = devices::table
        .find(&id)
        .select(crate::db::models::Device::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    // Determine limit (default 50, max 1000)
    let limit = params.limit.unwrap_or(50).clamp(1, 1000);

    // Build telemetry query
    let mut query = telemetry::table
        .filter(telemetry::device_id.eq(&id))
        .into_boxed();

    // Apply `since` filter if provided
    if let Some(ref since) = params.since {
        let since_dt = since
            .parse::<NaiveDateTime>()
            .or_else(|_| {
                chrono::DateTime::parse_from_rfc3339(since).map(|dt| dt.naive_utc())
            })
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        query = query.filter(telemetry::received_at.gt(since_dt));
    }

    let results: Vec<TelemetryRecord> = query
        .order(telemetry::received_at.desc())
        .limit(limit)
        .select(TelemetryRecord::as_select())
        .load(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<TelemetryResponse> =
        results.into_iter().map(TelemetryResponse::from).collect();

    Ok(Json(response))
}
