use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::DeviceLog;
use crate::error::AppError;
use crate::repositories::{device_repo, log_repo};
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
) -> Result<Json<Vec<LogResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    device_repo::device_exists(&mut conn, &id)?;

    let limit = params.limit.unwrap_or(100).clamp(1, 1000);

    // Normalize level to uppercase for the query
    let level = params.level.as_deref().map(str::to_uppercase);

    // Parse timestamp filter
    let since = if let Some(ref since_str) = params.since {
        let since_dt = since_str
            .parse::<NaiveDateTime>()
            .or_else(|_| chrono::DateTime::parse_from_rfc3339(since_str).map(|dt| dt.naive_utc()))
            .map_err(|_| AppError::BadRequest("Invalid date format, expected YYYY-MM-DDTHH:MM:SS".into()))?;
        Some(since_dt)
    } else {
        None
    };

    let results = log_repo::list_logs(
        &mut conn,
        &id,
        level.as_deref(),
        since,
        limit,
    )?;

    let response: Vec<LogResponse> = results.into_iter().map(LogResponse::from).collect();

    Ok(Json(response))
}
