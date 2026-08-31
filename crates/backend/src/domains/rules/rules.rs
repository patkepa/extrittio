use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::domains::rules::types::{RuleDetails, RuleFilter};
use crate::error::AppError;
use crate::services::rule_service;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListRulesQuery {
    pub enabled: Option<bool>,
    pub trigger_type: Option<String>,
    pub target_type: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRuleRequest {
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub cooldown_seconds: Option<i32>,
    pub conditions: Vec<ConditionInput>,
    pub actions: Vec<ActionInput>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRuleRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub trigger_type: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<Option<String>>,
    pub cooldown_seconds: Option<i32>,
    pub conditions: Option<Vec<ConditionInput>>,
    pub actions: Option<Vec<ActionInput>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConditionInput {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ActionInput {
    pub action_type: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct EnabledInput {
    pub enabled: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuleResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub trigger_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub cooldown_seconds: i32,
    pub conditions: Vec<ConditionResponse>,
    pub actions: Vec<ActionResponse>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConditionResponse {
    pub id: String,
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActionResponse {
    pub id: String,
    pub action_type: String,
    pub config: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

fn to_rule_response(details: RuleDetails) -> Result<RuleResponse, AppError> {
    let rule = details.rule;
    let conditions = details
        .conditions
        .into_iter()
        .map(|c| ConditionResponse {
            id: c.id,
            field: c.field,
            operator: c.operator,
            value: c.value,
        })
        .collect();

    let actions = details
        .actions
        .into_iter()
        .map(|a| ActionResponse {
            id: a.id,
            action_type: a.action_type,
            config: a.config,
        })
        .collect();

    Ok(RuleResponse {
        id: rule.id,
        name: rule.name,
        description: rule.description,
        enabled: rule.enabled,
        trigger_type: rule.trigger_type,
        target_type: rule.target_type,
        target_id: rule.target_id,
        cooldown_seconds: rule.cooldown_seconds,
        conditions,
        actions,
        created_at: DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            rule.created_at,
            chrono::Utc,
        )
        .to_rfc3339(),
        updated_at: DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            rule.updated_at,
            chrono::Utc,
        )
        .to_rfc3339(),
    })
}

// ---------------------------------------------------------------------------
// Cache refresh helper
// ---------------------------------------------------------------------------

async fn refresh_rule_cache(state: &AppState) {
    let (rules, zone_snapshots) = state.rule_cache_repositories();
    match rule_service::build_cache_with_repositories(rules, zone_snapshots).await {
        Ok(new_cache) => {
            if let Ok(mut guard) = state.rule_cache.write() {
                *guard = new_cache;
            } else {
                tracing::warn!("Rule cache lock poisoned; skipping refresh");
            }
        }
        Err(error) => tracing::warn!(%error, "Failed to refresh rule cache"),
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/rules", get(list_rules).post(create_rule))
        .route(
            "/api/v1/rules/{id}",
            get(get_rule)
                .put(update_rule_handler)
                .delete(delete_rule_handler),
        )
        .route("/api/v1/rules/{id}/enabled", put(toggle_rule))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/api/v1/rules", tag = "rules", security(("bearer_auth" = [])),
    params(("enabled" = Option<bool>, Query), ("trigger_type" = Option<String>, Query), ("target_type" = Option<String>, Query)),
    responses((status = 200, body = Vec<RuleResponse>))
)]
pub(crate) async fn list_rules(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListRulesQuery>,
) -> Result<Json<Vec<RuleResponse>>, AppError> {
    let details_list = rule_service::list_with_repository(
        &ctx,
        state.persistence.rules.as_ref(),
        RuleFilter {
            enabled: params.enabled,
            trigger_type: params.trigger_type,
            target_type: params.target_type,
        },
    )
    .await?;
    let responses = details_list
        .into_iter()
        .map(to_rule_response)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(responses))
}

#[utoipa::path(
    get, path = "/api/v1/rules/{id}", tag = "rules", security(("bearer_auth" = [])),
    params(("id" = String, Path)), responses((status = 200, body = RuleResponse), (status = 404))
)]
pub(crate) async fn get_rule(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RuleResponse>, AppError> {
    let details =
        rule_service::get_with_repository(&ctx, state.persistence.rules.as_ref(), &id).await?;

    Ok(Json(to_rule_response(details)?))
}

#[utoipa::path(
    post, path = "/api/v1/rules", tag = "rules", security(("bearer_auth" = [])),
    request_body = CreateRuleRequest, responses((status = 201, body = RuleResponse))
)]
pub(crate) async fn create_rule(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRuleRequest>,
) -> Result<(StatusCode, Json<RuleResponse>), AppError> {
    let conditions: Vec<(String, String, String)> = body
        .conditions
        .into_iter()
        .map(|c| (c.field, c.operator, c.value))
        .collect();

    let actions: Vec<(String, serde_json::Value)> = body
        .actions
        .into_iter()
        .map(|a| (a.action_type, a.config))
        .collect();

    let cooldown = body.cooldown_seconds.unwrap_or(0);

    let details = rule_service::create_with_repository(
        &ctx,
        state.persistence.rules.as_ref(),
        &body.name,
        body.description,
        &body.trigger_type,
        &body.target_type,
        body.target_id,
        cooldown,
        conditions,
        actions,
    )
    .await?;

    refresh_rule_cache(&state).await;

    Ok((StatusCode::CREATED, Json(to_rule_response(details)?)))
}

#[utoipa::path(
    put, path = "/api/v1/rules/{id}", tag = "rules", security(("bearer_auth" = [])),
    params(("id" = String, Path)), request_body = UpdateRuleRequest,
    responses((status = 200, body = RuleResponse), (status = 404))
)]
pub(crate) async fn update_rule_handler(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRuleRequest>,
) -> Result<Json<RuleResponse>, AppError> {
    let trigger_type_changing = body.trigger_type.clone();
    let conditions: Option<Vec<(String, String, String)>> = body.conditions.map(|cs| {
        cs.into_iter()
            .map(|c| (c.field, c.operator, c.value))
            .collect()
    });

    let actions: Option<Vec<(String, serde_json::Value)>> = body.actions.map(|acts| {
        acts.into_iter()
            .map(|a| (a.action_type, a.config))
            .collect()
    });

    let rule_id = id.clone();
    let details = rule_service::update_with_repository(
        &ctx,
        state.persistence.rules.as_ref(),
        &id,
        body.name,
        body.description,
        body.trigger_type,
        body.target_type,
        body.target_id,
        body.cooldown_seconds,
        conditions,
        actions,
    )
    .await?;

    // If trigger_type changed, remove any active alert cache entries for this
    // rule to prevent the engine from issuing UpdateAlertValue with mismatched
    // data types (e.g. telemetry value on a status alert).
    if trigger_type_changing.is_some()
        && let Ok(mut guard) = state.rule_cache.write()
    {
        let keys_to_remove: Vec<_> = guard
            .active_alerts
            .keys()
            .filter(|(_, rid, _)| rid == &rule_id)
            .cloned()
            .collect();
        for key in keys_to_remove {
            guard.active_alerts.remove(&key);
        }
    }

    refresh_rule_cache(&state).await;

    Ok(Json(to_rule_response(details)?))
}

#[utoipa::path(
    delete, path = "/api/v1/rules/{id}", tag = "rules", security(("bearer_auth" = [])),
    params(("id" = String, Path)), responses((status = 204), (status = 404))
)]
pub(crate) async fn delete_rule_handler(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    rule_service::delete_with_repository(&ctx, state.persistence.rules.as_ref(), &id).await?;

    refresh_rule_cache(&state).await;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put, path = "/api/v1/rules/{id}/enabled", tag = "rules", security(("bearer_auth" = [])),
    params(("id" = String, Path)), request_body = EnabledInput,
    responses((status = 200, body = RuleResponse), (status = 404))
)]
pub(crate) async fn toggle_rule(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<EnabledInput>,
) -> Result<Json<RuleResponse>, AppError> {
    let details = rule_service::toggle_with_repository(
        &ctx,
        state.persistence.rules.as_ref(),
        &id,
        body.enabled,
    )
    .await?;

    refresh_rule_cache(&state).await;

    Ok(Json(to_rule_response(details)?))
}
