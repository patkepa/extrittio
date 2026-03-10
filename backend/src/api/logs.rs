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

use crate::db::models::DeviceLog;
use crate::db::schema::{device_logs, devices};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct LogResponse {
    pub id: i32,
    pub device_id: String,
    pub level: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub limit: Option<i64>,
    pub level: Option<String>,
    pub since: Option<String>,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl From<DeviceLog> for LogResponse {
    fn from(r: DeviceLog) -> Self {
        Self {
            id: r.id,
            device_id: r.device_id,
            level: r.level,
            message: r.message,
            created_at: r.created_at.and_utc().to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/devices/{id}/logs", get(get_device_logs))
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

async fn get_device_logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<LogsQuery>,
) -> Result<Json<Vec<LogResponse>>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify device exists
    devices::table
        .find(&id)
        .select(devices::id)
        .first::<String>(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    let limit = params.limit.unwrap_or(100).clamp(1, 1000);

    let mut query = device_logs::table
        .filter(device_logs::device_id.eq(&id))
        .into_boxed();

    // Filter by log level
    if let Some(ref level) = params.level {
        query = query.filter(device_logs::level.eq(level.to_uppercase()));
    }

    // Filter by timestamp
    if let Some(ref since) = params.since {
        let since_dt = since
            .parse::<NaiveDateTime>()
            .or_else(|_| chrono::DateTime::parse_from_rfc3339(since).map(|dt| dt.naive_utc()))
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        query = query.filter(device_logs::created_at.gt(since_dt));
    }

    let results: Vec<DeviceLog> = query
        .order(device_logs::created_at.desc())
        .limit(limit)
        .select(DeviceLog::as_select())
        .load(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<LogResponse> = results.into_iter().map(LogResponse::from).collect();

    Ok(Json(response))
}
