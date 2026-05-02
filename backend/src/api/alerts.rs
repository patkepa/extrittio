use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::{get, put},
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::context::RequestContext;
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
        resolved_at: alert.resolved_at.map(|dt| {
            DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).to_rfc3339()
        }),
        acknowledged_at: alert.acknowledged_at.map(|dt| {
            DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).to_rfc3339()
        }),
        created_at: DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            alert.created_at,
            chrono::Utc,
        )
        .to_rfc3339(),
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
        .route(
            "/api/v1/alerts/{id}/reactivate",
            put(reactivate_alert_handler),
        )
        .route("/api/v1/alerts/bulk-acknowledge", put(bulk_acknowledge))
        .route("/api/v1/alerts/bulk-resolve", put(bulk_resolve))
        .route("/api/v1/alerts/bulk-reactivate", put(bulk_reactivate))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub(crate) async fn list_alerts(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListAlertsQuery>,
) -> Result<Json<pagination::PaginatedResponse<AlertResponse>>, AppError> {
    let since = util::parse_timestamp(params.since.as_deref())?;
    let before = util::parse_timestamp(params.before.as_deref())?;
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        let (alerts, total) = alert_service::list_alerts(
            &ctx,
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
        Ok(pagination::PaginatedResponse::new(
            data, total, limit, offset,
        ))
    })
    .await?;

    Ok(Json(response))
}

pub(crate) async fn get_summary(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<AlertSummary>, AppError> {
    let rows = run_db(&state.db_pool, move |conn| {
        alert_service::summary(&ctx, conn)
    })
    .await?;

    let mut active = AlertSeverityCounts {
        info: 0,
        warning: 0,
        critical: 0,
    };
    let mut acknowledged = AlertSeverityCounts {
        info: 0,
        warning: 0,
        critical: 0,
    };

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

    Ok(Json(AlertSummary {
        active,
        acknowledged,
        total_active,
    }))
}

pub(crate) async fn get_alert(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = run_db(&state.db_pool, move |conn| {
        alert_service::get_alert(&ctx, conn, &id)
    })
    .await?;
    Ok(Json(to_alert_response(alert)))
}

pub(crate) async fn acknowledge_alert(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = run_db(&state.db_pool, move |conn| {
        alert_service::acknowledge_alert(&ctx, conn, &id)
    })
    .await?;

    // Keep the alert in active_alerts cache so the rule engine continues to
    // track it (UpdateAlertValue) rather than creating a duplicate alert.

    Ok(Json(to_alert_response(alert)))
}

pub(crate) async fn resolve_alert_handler(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = run_db(&state.db_pool, move |conn| {
        alert_service::resolve_alert(&ctx, conn, &id)
    })
    .await?;

    // Remove from active_alerts cache and set cooldown to prevent immediate re-fire
    if let Some(rule_id) = &alert.rule_id {
        let rid = rule_id.clone();
        let did = alert.device_id.clone();
        let now = chrono::Utc::now().naive_utc();
        if let Ok(mut guard) = state.rule_cache.write() {
            guard.active_alerts.remove(&(rid.clone(), did.clone()));
            guard.cooldowns.insert((rid.clone(), did.clone()), now);
        }
        let pool = state.db_pool.clone();
        tokio::spawn(async move {
            let _ = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                alert_service::persist_cooldown(&mut conn, &rid, &did, now)
                    .map_err(|e| e.to_string())
            })
            .await;
        });
    }

    Ok(Json(to_alert_response(alert)))
}

pub(crate) async fn bulk_acknowledge(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAlertIds>,
) -> Result<Json<serde_json::Value>, AppError> {
    let ids = body.ids;
    let results = run_db(&state.db_pool, move |conn| {
        let mut updated = Vec::new();
        for id in &ids {
            match alert_service::acknowledge_alert(&ctx, conn, id) {
                Ok(a) => updated.push(a),
                Err(e) => tracing::warn!("Failed to acknowledge alert {id}: {e}"),
            }
        }
        Ok::<_, crate::error::AppError>(updated)
    })
    .await?;

    // Keep alerts in active_alerts cache so the rule engine continues to
    // track them rather than creating duplicate alerts.

    Ok(Json(serde_json::json!({ "acknowledged": results.len() })))
}

pub(crate) async fn bulk_resolve(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAlertIds>,
) -> Result<Json<serde_json::Value>, AppError> {
    let ids = body.ids;
    let results = run_db(&state.db_pool, move |conn| {
        let mut updated = Vec::new();
        for id in &ids {
            match alert_service::resolve_alert(&ctx, conn, id) {
                Ok(a) => updated.push(a),
                Err(e) => tracing::warn!("Failed to resolve alert {id}: {e}"),
            }
        }
        Ok::<_, crate::error::AppError>(updated)
    })
    .await?;

    // Remove from cache and set cooldowns to prevent immediate re-fire
    let now = chrono::Utc::now().naive_utc();
    let mut cooldown_entries = Vec::new();
    if let Ok(mut guard) = state.rule_cache.write() {
        for a in &results {
            if let Some(rule_id) = &a.rule_id {
                guard
                    .active_alerts
                    .remove(&(rule_id.clone(), a.device_id.clone()));
                guard
                    .cooldowns
                    .insert((rule_id.clone(), a.device_id.clone()), now);
                cooldown_entries.push((rule_id.clone(), a.device_id.clone()));
            }
        }
    }
    if !cooldown_entries.is_empty() {
        let pool = state.db_pool.clone();
        tokio::spawn(async move {
            let _ = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                for (rid, did) in cooldown_entries {
                    if let Err(e) = alert_service::persist_cooldown(&mut conn, &rid, &did, now) {
                        tracing::warn!("Failed to persist cooldown on bulk resolve: {}", e);
                    }
                }
                Ok::<(), String>(())
            })
            .await;
        });
    }

    Ok(Json(serde_json::json!({ "resolved": results.len() })))
}

pub(crate) async fn reactivate_alert_handler(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = run_db(&state.db_pool, move |conn| {
        alert_service::reactivate_alert(&ctx, conn, &id)
    })
    .await?;

    // Re-add to active_alerts cache and clear any stale cooldown so the rule
    // engine can resume tracking this alert immediately.
    if let Ok(mut guard) = state.rule_cache.write() {
        if let Some(rule_id) = &alert.rule_id {
            guard
                .active_alerts
                .insert((rule_id.clone(), alert.device_id.clone()), alert.id.clone());
            guard
                .cooldowns
                .remove(&(rule_id.clone(), alert.device_id.clone()));
        }
    }

    Ok(Json(to_alert_response(alert)))
}

pub(crate) async fn bulk_reactivate(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAlertIds>,
) -> Result<Json<serde_json::Value>, AppError> {
    let ids = body.ids;
    let results = run_db(&state.db_pool, move |conn| {
        let mut updated = Vec::new();
        for id in &ids {
            match alert_service::reactivate_alert(&ctx, conn, id) {
                Ok(a) => updated.push(a),
                Err(e) => tracing::warn!("Failed to reactivate alert {id}: {e}"),
            }
        }
        Ok::<_, crate::error::AppError>(updated)
    })
    .await?;

    if let Ok(mut guard) = state.rule_cache.write() {
        for a in &results {
            if let Some(rule_id) = &a.rule_id {
                guard
                    .active_alerts
                    .insert((rule_id.clone(), a.device_id.clone()), a.id.clone());
                guard
                    .cooldowns
                    .remove(&(rule_id.clone(), a.device_id.clone()));
            }
        }
    }

    Ok(Json(serde_json::json!({ "reactivated": results.len() })))
}
