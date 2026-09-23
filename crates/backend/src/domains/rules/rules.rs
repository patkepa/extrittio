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
use crate::error::AppError;
use crate::state::AppState;
use extrittio_backend_core::rules::{RuleConditionInput, RuleDetails, RuleFilter};

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
    pub selector: ConditionSelector,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConditionSelector {
    Metric {
        blueprint_id: String,
        blueprint_revision_id: String,
        stream_key: String,
        field_path: String,
    },
    Status,
    Geofence {
        zone_id: String,
        field: String,
    },
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
    pub selector: ConditionSelector,
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

fn condition_input(input: ConditionInput) -> RuleConditionInput {
    let (field, blueprint_id, blueprint_revision_id, zone_id) = match input.selector {
        ConditionSelector::Metric {
            blueprint_id,
            blueprint_revision_id,
            stream_key,
            field_path,
        } => (
            extrittio_backend_core::rule_engine::metric::MetricSelector {
                stream_key,
                field_path,
            }
            .field_key(),
            Some(blueprint_id),
            Some(blueprint_revision_id),
            None,
        ),
        ConditionSelector::Status => ("status".into(), None, None, None),
        ConditionSelector::Geofence { zone_id, field } => (field, None, None, Some(zone_id)),
    };
    RuleConditionInput {
        field,
        blueprint_id,
        blueprint_revision_id,
        operator: input.operator,
        value: input.value,
        zone_id,
    }
}

fn condition_response(
    condition: extrittio_backend_core::rules::RuleConditionRecord,
) -> Result<ConditionResponse, AppError> {
    let selector = match (
        condition.blueprint_id,
        condition.blueprint_revision_id,
        condition.zone_id,
    ) {
        (Some(blueprint_id), Some(blueprint_revision_id), None) => {
            let metric = extrittio_backend_core::rule_engine::metric::MetricSelector::parse(
                &condition.field,
            )
            .ok_or_else(|| AppError::Internal("Stored rule metric selector is invalid".into()))?;
            ConditionSelector::Metric {
                blueprint_id,
                blueprint_revision_id,
                stream_key: metric.stream_key,
                field_path: metric.field_path,
            }
        }
        (None, None, None) if condition.field == "status" => ConditionSelector::Status,
        (None, None, Some(zone_id)) => ConditionSelector::Geofence {
            zone_id,
            field: condition.field,
        },
        _ => return Err(AppError::Internal("Stored rule selector is invalid".into())),
    };
    Ok(ConditionResponse {
        id: condition.id,
        selector,
        operator: condition.operator,
        value: condition.value,
    })
}

fn to_rule_response(details: RuleDetails) -> Result<RuleResponse, AppError> {
    let rule = details.rule;
    let conditions = details
        .conditions
        .into_iter()
        .map(condition_response)
        .collect::<Result<Vec<_>, _>>()?;

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

#[cfg(test)]
mod selector_tests {
    use super::*;

    #[test]
    fn metric_selector_round_trips_dotted_stream_and_escaped_pointer() {
        let input = ConditionInput {
            selector: ConditionSelector::Metric {
                blueprint_id: "blueprint-a".into(),
                blueprint_revision_id: "revision-a".into(),
                stream_key: "machine.v2".into(),
                field_path: "/a~1b/count".into(),
            },
            operator: "gte".into(),
            value: "9007199254740993".into(),
        };
        let stored = condition_input(input);
        assert_eq!(stored.field, "machine.v2./a~1b/count");
        let response = condition_response(extrittio_backend_core::rules::RuleConditionRecord {
            id: "condition-a".into(),
            field: stored.field,
            blueprint_id: stored.blueprint_id,
            blueprint_revision_id: stored.blueprint_revision_id,
            operator: stored.operator,
            value: stored.value,
            condition_group: 0,
            zone_id: stored.zone_id,
        })
        .expect("structured stored selector");
        let encoded = serde_json::to_value(response.selector).expect("serialize selector");
        assert_eq!(encoded["stream_key"], "machine.v2");
        assert_eq!(encoded["field_path"], "/a~1b/count");
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
    let details_list = state
        .application()
        .rules()
        .list(
            &ctx.tenant_context(),
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
    let details = state
        .application()
        .rules()
        .get(&ctx.tenant_context(), &id)
        .await?;

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
    let conditions: Vec<RuleConditionInput> =
        body.conditions.into_iter().map(condition_input).collect();

    let actions: Vec<(String, serde_json::Value)> = body
        .actions
        .into_iter()
        .map(|a| (a.action_type, a.config))
        .collect();

    let cooldown = body.cooldown_seconds.unwrap_or(0);

    let details = state
        .application()
        .rules()
        .create(
            &ctx.tenant_context(),
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
    let conditions: Option<Vec<RuleConditionInput>> = body
        .conditions
        .map(|cs| cs.into_iter().map(condition_input).collect());

    let actions: Option<Vec<(String, serde_json::Value)>> = body.actions.map(|acts| {
        acts.into_iter()
            .map(|a| (a.action_type, a.config))
            .collect()
    });

    let details = state
        .application()
        .rules()
        .update(
            &ctx.tenant_context(),
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
    state
        .application()
        .rules()
        .delete(&ctx.tenant_context(), &id)
        .await?;

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
    let details = state
        .application()
        .rules()
        .toggle(&ctx.tenant_context(), &id, body.enabled)
        .await?;

    Ok(Json(to_rule_response(details)?))
}
