use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::AppError;
use crate::services::rule_service;
use crate::state::{AppState, run_db};

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListRulesQuery {
    pub enabled: Option<bool>,
    pub trigger_type: Option<String>,
    pub target_type: Option<String>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct ConditionInput {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct ActionInput {
    pub action_type: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct EnabledInput {
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct ConditionResponse {
    pub id: String,
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub id: String,
    pub action_type: String,
    pub config: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

fn to_rule_response(details: rule_service::RuleWithDetails) -> Result<RuleResponse, AppError> {
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
        .map(|a| {
            let config: serde_json::Value = serde_json::from_str(&a.config)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            ActionResponse {
                id: a.id,
                action_type: a.action_type,
                config,
            }
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
        created_at: DateTime::<chrono::Utc>::from_naive_utc_and_offset(rule.created_at, chrono::Utc).to_rfc3339(),
        updated_at: DateTime::<chrono::Utc>::from_naive_utc_and_offset(rule.updated_at, chrono::Utc).to_rfc3339(),
    })
}

// ---------------------------------------------------------------------------
// Cache refresh helper
// ---------------------------------------------------------------------------

async fn refresh_rule_cache(state: &AppState) {
    let pool = state.db_pool.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        crate::services::rule_service::build_cache(&mut conn).map_err(|e| e.to_string())
    })
    .await;
    match result {
        Ok(Ok(new_cache)) => {
            let mut guard = state.rule_cache.write().unwrap();
            *guard = new_cache;
        }
        Ok(Err(e)) => tracing::warn!("Failed to refresh rule cache: {e}"),
        Err(e) => tracing::warn!("Rule cache refresh task failed: {e}"),
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
            get(get_rule).put(update_rule_handler).delete(delete_rule_handler),
        )
        .route("/api/v1/rules/{id}/enabled", put(toggle_rule))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub(crate) async fn list_rules(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListRulesQuery>,
) -> Result<Json<Vec<RuleResponse>>, AppError> {
    let responses = run_db(&state.db_pool, move |conn| {
        let details_list = rule_service::list_rules_with_details(
            conn,
            params.enabled,
            params.trigger_type.as_deref(),
            params.target_type.as_deref(),
        )?;

        details_list
            .into_iter()
            .map(to_rule_response)
            .collect::<Result<Vec<_>, _>>()
    })
    .await?;

    Ok(Json(responses))
}

pub(crate) async fn get_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RuleResponse>, AppError> {
    let details = run_db(&state.db_pool, move |conn| {
        rule_service::get_rule(conn, &id)
    })
    .await?;

    Ok(Json(to_rule_response(details)?))
}

pub(crate) async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRuleRequest>,
) -> Result<(StatusCode, Json<RuleResponse>), AppError> {
    let conditions: Vec<(String, String, String)> = body
        .conditions
        .into_iter()
        .map(|c| (c.field, c.operator, c.value))
        .collect();

    let actions: Vec<(String, String)> = body
        .actions
        .into_iter()
        .map(|a| (a.action_type, a.config.to_string()))
        .collect();

    let cooldown = body.cooldown_seconds.unwrap_or(0);

    let details = run_db(&state.db_pool, move |conn| {
        rule_service::create_rule(
            conn,
            &body.name,
            body.description,
            &body.trigger_type,
            &body.target_type,
            body.target_id,
            cooldown,
            conditions,
            actions,
        )
    })
    .await?;

    refresh_rule_cache(&state).await;

    Ok((StatusCode::CREATED, Json(to_rule_response(details)?)))
}

pub(crate) async fn update_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRuleRequest>,
) -> Result<Json<RuleResponse>, AppError> {
    let conditions: Option<Vec<(String, String, String)>> = body.conditions.map(|cs| {
        cs.into_iter()
            .map(|c| (c.field, c.operator, c.value))
            .collect()
    });

    let actions: Option<Vec<(String, String)>> = body.actions.map(|acts| {
        acts.into_iter()
            .map(|a| (a.action_type, a.config.to_string()))
            .collect()
    });

    let details = run_db(&state.db_pool, move |conn| {
        rule_service::update_rule(
            conn,
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
    })
    .await?;

    refresh_rule_cache(&state).await;

    Ok(Json(to_rule_response(details)?))
}

pub(crate) async fn delete_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    run_db(&state.db_pool, move |conn| {
        rule_service::delete_rule(conn, &id)
    })
    .await?;

    refresh_rule_cache(&state).await;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn toggle_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<EnabledInput>,
) -> Result<Json<RuleResponse>, AppError> {
    let details = run_db(&state.db_pool, move |conn| {
        rule_service::toggle_rule(conn, &id, body.enabled)?;
        rule_service::get_rule(conn, &id)
    })
    .await?;

    refresh_rule_cache(&state).await;

    Ok(Json(to_rule_response(details)?))
}
