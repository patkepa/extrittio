use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::domains::operations::metrics_types::{AppMetricRecord, SystemMetricRecord};
use crate::error::AppError;
use crate::services::server_metrics;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct CurrentMetricsResponse {
    pub system: Option<SystemMetricsSnapshot>,
    pub app: Option<AppMetricsSnapshot>,
}

#[derive(Serialize, ToSchema)]
pub struct SystemMetricsSnapshot {
    pub cpu_usage_percent: f32,
    pub memory_used_bytes: i64,
    pub memory_total_bytes: i64,
    pub disk_used_bytes: i64,
    pub disk_total_bytes: i64,
    pub network_rx_bytes_delta: i64,
    pub network_tx_bytes_delta: i64,
    pub load_avg_1m: f32,
    pub load_avg_5m: f32,
    pub load_avg_15m: f32,
    pub recorded_at: String,
}

#[derive(Serialize, ToSchema)]
pub struct AppMetricsSnapshot {
    pub request_count: i32,
    pub error_count: i32,
    pub avg_latency_ms: f32,
    pub p95_latency_ms: f32,
    pub db_pool_active: i32,
    pub db_pool_idle: i32,
    pub zenoh_messages_in: i32,
    pub zenoh_messages_out: i32,
    pub recorded_at: String,
}

#[derive(Deserialize, IntoParams)]
pub struct HistoryParams {
    /// Start of the time range (ISO 8601). Defaults to 1 hour ago.
    pub since: Option<String>,
    /// Bucket size in seconds for downsampling. Defaults to 10 (raw data).
    pub resolution: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct MetricsHistoryResponse {
    pub system: Vec<SystemMetricsSnapshot>,
    pub app: Vec<AppMetricsSnapshot>,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl From<SystemMetricRecord> for SystemMetricsSnapshot {
    fn from(m: SystemMetricRecord) -> Self {
        Self {
            cpu_usage_percent: m.cpu_usage_percent,
            memory_used_bytes: m.memory_used_bytes,
            memory_total_bytes: m.memory_total_bytes,
            disk_used_bytes: m.disk_used_bytes,
            disk_total_bytes: m.disk_total_bytes,
            network_rx_bytes_delta: m.network_rx_bytes_delta,
            network_tx_bytes_delta: m.network_tx_bytes_delta,
            load_avg_1m: m.load_avg_1m,
            load_avg_5m: m.load_avg_5m,
            load_avg_15m: m.load_avg_15m,
            recorded_at: m.recorded_at.and_utc().to_rfc3339(),
        }
    }
}

impl From<AppMetricRecord> for AppMetricsSnapshot {
    fn from(m: AppMetricRecord) -> Self {
        Self {
            request_count: m.request_count,
            error_count: m.error_count,
            avg_latency_ms: m.avg_latency_ms,
            p95_latency_ms: m.p95_latency_ms,
            db_pool_active: m.db_pool_active,
            db_pool_idle: m.db_pool_idle,
            zenoh_messages_in: m.zenoh_messages_in,
            zenoh_messages_out: m.zenoh_messages_out,
            recorded_at: m.recorded_at.and_utc().to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/server/metrics/current", get(get_current_metrics))
        .route("/api/v1/server/metrics/history", get(get_metrics_history))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Get the latest server and application metrics snapshot.
#[utoipa::path(
    get,
    path = "/api/v1/server/metrics/current",
    tag = "server-metrics",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current metrics snapshot", body = CurrentMetricsResponse),
    ),
)]
pub(crate) async fn get_current_metrics(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<CurrentMetricsResponse>, AppError> {
    let metrics =
        server_metrics::get_current_metrics(&ctx, state.persistence.metrics.as_ref()).await?;
    let response = CurrentMetricsResponse {
        system: metrics.system.map(SystemMetricsSnapshot::from),
        app: metrics.app.map(AppMetricsSnapshot::from),
    };

    Ok(Json(response))
}

/// Get server and application metrics history over a time range.
#[utoipa::path(
    get,
    path = "/api/v1/server/metrics/history",
    tag = "server-metrics",
    security(("bearer_auth" = [])),
    params(HistoryParams),
    responses(
        (status = 200, description = "Metrics time-series", body = MetricsHistoryResponse),
    ),
)]
pub(crate) async fn get_metrics_history(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<MetricsHistoryResponse>, AppError> {
    let since = parse_since(params.since.as_deref())?;
    let resolution = params.resolution.unwrap_or(10);
    if resolution < 10 {
        return Err(AppError::BadRequest("resolution must be >= 10".into()));
    }

    let history = server_metrics::get_metrics_history(
        &ctx,
        state.persistence.metrics.as_ref(),
        since,
        resolution,
    )
    .await?;
    let response = MetricsHistoryResponse {
        system: history
            .system
            .into_iter()
            .map(SystemMetricsSnapshot::from)
            .collect(),
        app: history
            .app
            .into_iter()
            .map(AppMetricsSnapshot::from)
            .collect(),
    };

    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse an optional `since` timestamp string. Accepts both NaiveDateTime and
/// RFC 3339 formats. Falls back to 1 hour ago when `None`.
fn parse_since(since_str: Option<&str>) -> Result<NaiveDateTime, AppError> {
    match since_str {
        Some(s) => {
            let dt = s
                .parse::<NaiveDateTime>()
                .or_else(|_| chrono::DateTime::parse_from_rfc3339(s).map(|dt| dt.naive_utc()))
                .map_err(|_| {
                    AppError::BadRequest(
                        "Invalid date format, expected ISO 8601 (e.g. 2025-01-01T00:00:00)".into(),
                    )
                })?;
            Ok(dt)
        }
        None => {
            let now = chrono::Utc::now().naive_utc();
            Ok(now - chrono::TimeDelta::hours(1))
        }
    }
}
