use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::domains::logs::types::{LogQuery, LogRecord};
use crate::error::AppError;
use crate::state::AppState;
use crate::util;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct LogResponse {
    pub id: i64,
    pub device_id: String,
    pub level: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct LogsQuery {
    /// Maximum number of records to return (default 100, max 1000).
    pub limit: Option<i64>,
    /// Filter by log level (DEBUG, INFO, WARN, ERROR).
    pub level: Option<String>,
    /// Only return records after this timestamp (RFC 3339 or YYYY-MM-DDTHH:MM:SS).
    pub since: Option<String>,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl From<LogRecord> for LogResponse {
    fn from(r: LogRecord) -> Self {
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
    Router::new().route("/api/v1/devices/{id}/logs", get(get_device_logs))
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Get logs for a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/logs",
    tag = "logs",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Device ID"),
        LogsQuery,
    ),
    responses(
        (status = 200, description = "Device logs", body = Vec<LogResponse>),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn get_device_logs(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(params): Query<LogsQuery>,
) -> Result<Json<Vec<LogResponse>>, AppError> {
    // Parse timestamp filter before entering the blocking closure
    let since = util::parse_timestamp(params.since.as_deref())?;

    let results = state
        .application()
        .logs()
        .list(
            &ctx.tenant_context(),
            &id,
            LogQuery {
                limit: params.limit.unwrap_or(100),
                level: params.level,
                since,
            },
        )
        .await?;
    let response = results.into_iter().map(LogResponse::from).collect();

    Ok(Json(response))
}
