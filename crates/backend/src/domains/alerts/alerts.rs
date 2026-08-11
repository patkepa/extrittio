use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    routing::{get, put},
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::domains::alerts::types::{
    AlertListFilter, AlertRecord, AlertTransition, CooldownRecord,
};
use crate::error::AppError;
use crate::pagination;
use crate::services::alert_service;
use crate::state::AppState;
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkAlertIds {
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
pub struct AlertSummary {
    pub active: AlertSeverityCounts,
    pub acknowledged: AlertSeverityCounts,
    pub total_active: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AlertSeverityCounts {
    pub info: i64,
    pub warning: i64,
    pub critical: i64,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

fn to_alert_response(alert: AlertRecord) -> AlertResponse {
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

#[utoipa::path(
    get, path = "/api/v1/alerts", tag = "alerts", security(("bearer_auth" = [])),
    params(
        ("status" = Option<String>, Query), ("severity" = Option<String>, Query),
        ("device_id" = Option<String>, Query), ("rule_id" = Option<String>, Query),
        ("since" = Option<String>, Query), ("before" = Option<String>, Query),
        ("limit" = Option<i64>, Query), ("offset" = Option<i64>, Query)
    ),
    responses((status = 200, body = crate::pagination::PaginatedResponse<AlertResponse>))
)]
pub(crate) async fn list_alerts(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListAlertsQuery>,
) -> Result<Json<pagination::PaginatedResponse<AlertResponse>>, AppError> {
    let since = util::parse_timestamp(params.since.as_deref())?;
    let before = util::parse_timestamp(params.before.as_deref())?;
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let (alerts, total) = alert_service::list_with_repository(
        &ctx,
        state.persistence.alerts.as_ref(),
        AlertListFilter {
            status: params.status,
            severity: params.severity,
            device_id: params.device_id,
            rule_id: params.rule_id,
            since,
            before,
            limit,
            offset,
        },
    )
    .await?;
    let response = pagination::PaginatedResponse::new(
        alerts.into_iter().map(to_alert_response).collect(),
        total,
        limit,
        offset,
    );

    Ok(Json(response))
}

#[utoipa::path(
    get, path = "/api/v1/alerts/summary", tag = "alerts", security(("bearer_auth" = [])),
    responses((status = 200, body = AlertSummary))
)]
pub(crate) async fn get_summary(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<AlertSummary>, AppError> {
    let rows =
        alert_service::summary_with_repository(&ctx, state.persistence.alerts.as_ref()).await?;

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

#[utoipa::path(
    get, path = "/api/v1/alerts/{id}", tag = "alerts", security(("bearer_auth" = [])),
    params(("id" = String, Path)), responses((status = 200, body = AlertResponse), (status = 404))
)]
pub(crate) async fn get_alert(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert =
        alert_service::get_with_repository(&ctx, state.persistence.alerts.as_ref(), &id).await?;
    Ok(Json(to_alert_response(alert)))
}

#[utoipa::path(
    put, path = "/api/v1/alerts/{id}/acknowledge", tag = "alerts", security(("bearer_auth" = [])),
    params(("id" = String, Path)), responses((status = 200, body = AlertResponse), (status = 404))
)]
pub(crate) async fn acknowledge_alert(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = alert_service::transition_with_repository(
        &ctx,
        state.persistence.alerts.as_ref(),
        &id,
        AlertTransition::Acknowledge,
    )
    .await?;

    // Keep the alert in active_alerts cache so the rule engine continues to
    // track it (UpdateAlertValue) rather than creating a duplicate alert.

    Ok(Json(to_alert_response(alert)))
}

#[utoipa::path(
    put, path = "/api/v1/alerts/{id}/resolve", tag = "alerts", security(("bearer_auth" = [])),
    params(("id" = String, Path)), responses((status = 200, body = AlertResponse), (status = 404))
)]
pub(crate) async fn resolve_alert_handler(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = alert_service::transition_with_repository(
        &ctx,
        state.persistence.alerts.as_ref(),
        &id,
        AlertTransition::Resolve,
    )
    .await?;

    // Remove from active_alerts cache and set cooldown to prevent immediate re-fire
    if let Some(rule_id) = &alert.rule_id {
        let rid = rule_id.clone();
        let did = alert.device_id.clone();
        let now = chrono::Utc::now().naive_utc();
        if let Ok(mut guard) = state.rule_cache.write() {
            guard
                .active_alerts
                .remove(&(alert.tenant_id.clone(), rid.clone(), did.clone()));
            guard
                .cooldowns
                .insert((alert.tenant_id.clone(), rid.clone(), did.clone()), now);
        }
        let repository = state.persistence.alerts.clone();
        let tenant_id = alert.tenant_id.clone();
        tokio::spawn(async move {
            if let Err(error) = alert_service::persist_cooldowns_with_repository(
                repository.as_ref(),
                vec![CooldownRecord {
                    tenant_id,
                    rule_id: rid,
                    device_id: did,
                    last_fired_at: now,
                }],
            )
            .await
            {
                tracing::warn!(%error, "Failed to persist cooldown on alert resolve");
            }
        });
    }

    Ok(Json(to_alert_response(alert)))
}

#[utoipa::path(
    put, path = "/api/v1/alerts/bulk-acknowledge", tag = "alerts", security(("bearer_auth" = [])),
    request_body = BulkAlertIds, responses((status = 200, body = serde_json::Value))
)]
pub(crate) async fn bulk_acknowledge(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAlertIds>,
) -> Result<Json<serde_json::Value>, AppError> {
    let results = alert_service::transition_many_with_repository(
        &ctx,
        state.persistence.alerts.as_ref(),
        body.ids,
        AlertTransition::Acknowledge,
    )
    .await?;

    // Keep alerts in active_alerts cache so the rule engine continues to
    // track them rather than creating duplicate alerts.

    Ok(Json(serde_json::json!({ "acknowledged": results.len() })))
}

#[utoipa::path(
    put, path = "/api/v1/alerts/bulk-resolve", tag = "alerts", security(("bearer_auth" = [])),
    request_body = BulkAlertIds, responses((status = 200, body = serde_json::Value))
)]
pub(crate) async fn bulk_resolve(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAlertIds>,
) -> Result<Json<serde_json::Value>, AppError> {
    let results = alert_service::transition_many_with_repository(
        &ctx,
        state.persistence.alerts.as_ref(),
        body.ids,
        AlertTransition::Resolve,
    )
    .await?;

    // Remove from cache and set cooldowns to prevent immediate re-fire
    let now = chrono::Utc::now().naive_utc();
    let mut cooldown_entries = Vec::new();
    if let Ok(mut guard) = state.rule_cache.write() {
        for a in &results {
            if let Some(rule_id) = &a.rule_id {
                guard.active_alerts.remove(&(
                    a.tenant_id.clone(),
                    rule_id.clone(),
                    a.device_id.clone(),
                ));
                guard.cooldowns.insert(
                    (a.tenant_id.clone(), rule_id.clone(), a.device_id.clone()),
                    now,
                );
                cooldown_entries.push((a.tenant_id.clone(), rule_id.clone(), a.device_id.clone()));
            }
        }
    }
    if !cooldown_entries.is_empty() {
        let repository = state.persistence.alerts.clone();
        tokio::spawn(async move {
            let cooldowns = cooldown_entries
                .into_iter()
                .map(|(tenant_id, rule_id, device_id)| CooldownRecord {
                    tenant_id,
                    rule_id,
                    device_id,
                    last_fired_at: now,
                })
                .collect();
            if let Err(error) =
                alert_service::persist_cooldowns_with_repository(repository.as_ref(), cooldowns)
                    .await
            {
                tracing::warn!(%error, "Failed to persist cooldowns on bulk resolve");
            }
        });
    }

    Ok(Json(serde_json::json!({ "resolved": results.len() })))
}

#[utoipa::path(
    put, path = "/api/v1/alerts/{id}/reactivate", tag = "alerts", security(("bearer_auth" = [])),
    params(("id" = String, Path)), responses((status = 200, body = AlertResponse), (status = 404))
)]
pub(crate) async fn reactivate_alert_handler(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, AppError> {
    let alert = alert_service::transition_with_repository(
        &ctx,
        state.persistence.alerts.as_ref(),
        &id,
        AlertTransition::Reactivate,
    )
    .await?;

    // Re-add to active_alerts cache and clear any stale cooldown so the rule
    // engine can resume tracking this alert immediately.
    if let Ok(mut guard) = state.rule_cache.write()
        && let Some(rule_id) = &alert.rule_id
    {
        guard.active_alerts.insert(
            (
                alert.tenant_id.clone(),
                rule_id.clone(),
                alert.device_id.clone(),
            ),
            alert.id.clone(),
        );
        guard.cooldowns.remove(&(
            alert.tenant_id.clone(),
            rule_id.clone(),
            alert.device_id.clone(),
        ));
    }

    Ok(Json(to_alert_response(alert)))
}

#[utoipa::path(
    put, path = "/api/v1/alerts/bulk-reactivate", tag = "alerts", security(("bearer_auth" = [])),
    request_body = BulkAlertIds, responses((status = 200, body = serde_json::Value))
)]
pub(crate) async fn bulk_reactivate(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAlertIds>,
) -> Result<Json<serde_json::Value>, AppError> {
    let results = alert_service::transition_many_with_repository(
        &ctx,
        state.persistence.alerts.as_ref(),
        body.ids,
        AlertTransition::Reactivate,
    )
    .await?;

    if let Ok(mut guard) = state.rule_cache.write() {
        for a in &results {
            if let Some(rule_id) = &a.rule_id {
                guard.active_alerts.insert(
                    (a.tenant_id.clone(), rule_id.clone(), a.device_id.clone()),
                    a.id.clone(),
                );
                guard.cooldowns.remove(&(
                    a.tenant_id.clone(),
                    rule_id.clone(),
                    a.device_id.clone(),
                ));
            }
        }
    }

    Ok(Json(serde_json::json!({ "reactivated": results.len() })))
}
