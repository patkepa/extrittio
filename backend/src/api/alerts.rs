use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, put},
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::AppError;
use crate::pagination;
use crate::services::alert_service;
use crate::state::{AppState, run_db};
use crate::util;

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListAlertsQuery {
    pub status: Option<String>,
    pub severity: Option<String>,
    pub device_id: Option<String>,
    pub rule_id: Option<String>,
    pub since: Option<String>,
    pub before: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BulkAlertIds {
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AlertResponse {
    pub id: String,
    pub rule_id: Option<String>,
    pub device_id: String,
    pub severity: String,
    pub status: String,
    pub message: String,
    pub triggered_value: Option<String>,
    pub resolved_at: Option<String>,
    pub acknowledged_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AlertSummary {
    pub active: AlertSeverityCounts,
    pub acknowledged: AlertSeverityCounts,
    pub total_active: i64,
}

#[derive(Debug, Serialize)]
pub struct AlertSeverityCounts {
    pub info: i64,
    pub warning: i64,
    pub critical: i64,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

fn to_alert_response(alert: crate::db::models::Alert) -> AlertResponse {
    AlertResponse {
        id: alert.id,
        rule_id: alert.rule_id,
        device_id: alert.device_id,
        severity: alert.severity,
        status: alert.status,
        message: alert.message,
        triggered_value: alert.triggered_value,
        resolved_at: alert
            .resolved_at
            .map(|dt| DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).to_rfc3339()),
        acknowledged_at: alert
            .acknowledged_at
            .map(|dt| DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).to_rfc3339()),
        created_at: DateTime::<chrono::Utc>::from_naive_utc_and_offset(alert.created_at, chrono::Utc).to_rfc3339(),
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/alerts", get(list_alerts))
        .route("/api/v1/alerts/summary", get(get_summary))
        .route("/api/v1/alerts/{id}", get(get_alert))
        .route("/api/v1/alerts/{id}/acknowledge", put(acknowledge_alert))
        .route("/api/v1/alerts/{id}/resolve", put(resolve_alert_handler))
        .route("/api/v1/alerts/bulk-acknowledge", put(bulk_acknowledge))
        .route("/api/v1/alerts/bulk-resolve", put(bulk_resolve))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub(crate) async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListAlertsQuery>,
) -> Result<Json<pagination::PaginatedResponse<AlertResponse>>, AppError> {
    let since = util::parse_timestamp(params.since.as_deref())?;
    let before = util::parse_timestamp(params.before.as_deref())?;
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        let (alerts, total) = alert_service::list_alerts(
            conn,
            params.status.as_deref(),
            params.severity.as_deref(),
            params.device_id.as_deref(),
            params.rule_id.as_deref(),
            since,
            before,
            limit,
            offset,
        )?;

        let data = alerts.into_iter().map(to_alert_response).collect();
        Ok(pagination::PaginatedResponse::new(data, total, limit, offset))
    })
    .await?;

    Ok(Json(response))
}

pub(crate) async fn get_summary(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AlertSummary>, AppError> {
    let rows = run_db(&state.db_pool, move |conn| alert_service::summary(conn)).await?;

    let mut active = AlertSeverityCounts { info: 0, warning: 0, critical: 0 };
    let mut acknowledged = AlertSeverityCounts { info: 0, warning: 0, critical: 0 };

    for (status, severity, count) in &rows {
        match status.as_str() {
            "active" => match severity.as_str() {
                "info" => active.info += count,
                "warning" => active.warning += count,
                "critical" => active.critical += count,
                _ => {}
            },
            "acknowledged" => match severity.as_str() {
                "info" => acknowledged.info += count,
                "warning" => acknowledged.warning += count,
                "critical" => acknowledged.critical += count,
                _ => {}
            },
            _ => {}
        }
    }

    let total_active = active.info + active.warning + active.critical;

    Ok(Json(AlertSummary { active, acknowledged, total_active }))
}

pub(crate) async fn get_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert =
        run_db(&state.db_pool, move |conn| alert_service::get_alert(conn, &id)).await?;
    Ok(Json(to_alert_response(alert)))
}

pub(crate) async fn acknowledge_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = run_db(&state.db_pool, move |conn| {
        alert_service::acknowledge_alert(conn, &id)
    })
    .await?;

    // Remove from active_alerts cache
    {
        let mut guard = state.rule_cache.write().unwrap();
        if let Some(rule_id) = &alert.rule_id {
            guard.active_alerts.remove(&(rule_id.clone(), alert.device_id.clone()));
        }
    }

    Ok(Json(to_alert_response(alert)))
}

pub(crate) async fn resolve_alert_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = run_db(&state.db_pool, move |conn| {
        alert_service::resolve_alert(conn, &id)
    })
    .await?;

    // Remove from active_alerts cache
    {
        let mut guard = state.rule_cache.write().unwrap();
        if let Some(rule_id) = &alert.rule_id {
            guard.active_alerts.remove(&(rule_id.clone(), alert.device_id.clone()));
        }
    }

    Ok(Json(to_alert_response(alert)))
}

pub(crate) async fn bulk_acknowledge(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAlertIds>,
) -> Result<Json<serde_json::Value>, AppError> {
    let ids = body.ids.clone();
    let mut updated = Vec::new();

    for id in &ids {
        let id_owned = id.clone();
        let id_log = id.clone();
        let alert = run_db(&state.db_pool, move |conn| {
            alert_service::acknowledge_alert(conn, &id_owned)
        })
        .await;

        match alert {
            Ok(a) => {
                // Update cache
                let mut guard = state.rule_cache.write().unwrap();
                if let Some(rule_id) = &a.rule_id {
                    guard.active_alerts.remove(&(rule_id.clone(), a.device_id.clone()));
                }
                updated.push(a.id.clone());
            }
            Err(e) => {
                tracing::warn!("Failed to acknowledge alert {id_log}: {e}");
            }
        }
    }

    Ok(Json(serde_json::json!({ "acknowledged": updated.len() })))
}

pub(crate) async fn bulk_resolve(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAlertIds>,
) -> Result<Json<serde_json::Value>, AppError> {
    let ids = body.ids.clone();
    let mut updated = Vec::new();

    for id in &ids {
        let id_owned = id.clone();
        let id_log = id.clone();
        let alert = run_db(&state.db_pool, move |conn| {
            alert_service::resolve_alert(conn, &id_owned)
        })
        .await;

        match alert {
            Ok(a) => {
                // Update cache
                let mut guard = state.rule_cache.write().unwrap();
                if let Some(rule_id) = &a.rule_id {
                    guard.active_alerts.remove(&(rule_id.clone(), a.device_id.clone()));
                }
                updated.push(a.id.clone());
            }
            Err(e) => {
                tracing::warn!("Failed to resolve alert {id_log}: {e}");
            }
        }
    }

    Ok(Json(serde_json::json!({ "resolved": updated.len() })))
}
