use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct RuleActionOutboxSummaryResponse {
    pub pending_count: i64,
    pub processing_count: i64,
    pub failed_count: i64,
    pub dead_letter_count: i64,
    pub succeeded_count: i64,
    pub oldest_pending_at: Option<String>,
    pub oldest_pending_age_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DeadLetterQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct DeadLetterEventResponse {
    pub id: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: serde_json::Value,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, ToSchema)]
pub struct DeadLetterListResponse {
    pub data: Vec<DeadLetterEventResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplayDeadLettersRequest {
    #[serde(default)]
    pub event_ids: Vec<String>,
    #[serde(default)]
    pub replay_all: bool,
}

#[derive(Serialize, ToSchema)]
pub struct ReplayDeadLettersResponse {
    pub replayed_count: usize,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/server/outbox/rule-actions", get(get_summary))
        .route(
            "/api/v1/server/outbox/rule-actions/dead-letters",
            get(list_dead_letters),
        )
        .route(
            "/api/v1/server/outbox/rule-actions/dead-letters/replay",
            post(replay_dead_letters),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/server/outbox/rule-actions/dead-letters",
    tag = "server-metrics",
    security(("bearer_auth" = [])),
    params(
        ("limit" = Option<i64>, Query, description = "Maximum rows, from 1 to 200"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
    ),
    responses(
        (status = 200, description = "Tenant dead-letter events", body = DeadLetterListResponse),
    ),
)]
pub(crate) async fn list_dead_letters(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<DeadLetterQuery>,
) -> Result<Json<DeadLetterListResponse>, AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let events = state
        .application()
        .outbox()
        .dead_letters(&ctx.tenant_context(), limit, offset)
        .await?
        .into_iter()
        .map(|event| DeadLetterEventResponse {
            id: event.id,
            event_type: event.event_type,
            aggregate_type: event.aggregate_type,
            aggregate_id: event.aggregate_id,
            payload: event.payload,
            attempts: event.attempts,
            max_attempts: event.max_attempts,
            last_error: event.last_error,
            created_at: event.created_at.and_utc().to_rfc3339(),
            updated_at: event.updated_at.and_utc().to_rfc3339(),
        })
        .collect();

    Ok(Json(DeadLetterListResponse { data: events }))
}

#[utoipa::path(
    post,
    path = "/api/v1/server/outbox/rule-actions/dead-letters/replay",
    tag = "server-metrics",
    security(("bearer_auth" = [])),
    request_body = ReplayDeadLettersRequest,
    responses(
        (status = 200, description = "Dead-letter events scheduled for replay", body = ReplayDeadLettersResponse),
        (status = 400, description = "No replay target was selected", body = crate::error::ErrorBody),
    ),
)]
pub(crate) async fn replay_dead_letters(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(request): Json<ReplayDeadLettersRequest>,
) -> Result<Json<ReplayDeadLettersResponse>, AppError> {
    let replayed_count = state
        .application()
        .outbox()
        .replay(&ctx.tenant_context(), request.event_ids, request.replay_all)
        .await?;

    Ok(Json(ReplayDeadLettersResponse { replayed_count }))
}

#[utoipa::path(
    get,
    path = "/api/v1/server/outbox/rule-actions",
    operation_id = "get_rule_action_outbox_summary",
    tag = "server-metrics",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Rule action outbox summary", body = RuleActionOutboxSummaryResponse),
    ),
)]
pub(crate) async fn get_summary(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<RuleActionOutboxSummaryResponse>, AppError> {
    let summary = state
        .application()
        .outbox()
        .summary(&ctx.tenant_context())
        .await?;
    let response = RuleActionOutboxSummaryResponse {
        pending_count: summary.pending_count,
        processing_count: summary.processing_count,
        failed_count: summary.failed_count,
        dead_letter_count: summary.dead_letter_count,
        succeeded_count: summary.succeeded_count,
        oldest_pending_at: summary
            .oldest_pending_at
            .map(|dt| dt.and_utc().to_rfc3339()),
        oldest_pending_age_seconds: summary.oldest_pending_age_seconds,
    };

    Ok(Json(response))
}
