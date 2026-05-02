use axum::{Extension, Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::error::AppError;
use crate::repositories::rule_action_outbox_repo;
use crate::state::{AppState, run_db};

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

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/server/outbox/rule-actions", get(get_summary))
}

#[utoipa::path(
    get,
    path = "/api/v1/server/outbox/rule-actions",
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
    let response = run_db(&state.db_pool, move |conn| {
        policy::require(&ctx, Permission::ReadServerMetrics)?;
        let summary = rule_action_outbox_repo::summarize_for_tenant(conn, ctx.tenant_id_str())?;

        Ok(RuleActionOutboxSummaryResponse {
            pending_count: summary.pending_count,
            processing_count: summary.processing_count,
            failed_count: summary.failed_count,
            dead_letter_count: summary.dead_letter_count,
            succeeded_count: summary.succeeded_count,
            oldest_pending_at: summary
                .oldest_pending_at
                .map(|dt| dt.and_utc().to_rfc3339()),
            oldest_pending_age_seconds: summary.oldest_pending_age_seconds,
        })
    })
    .await?;

    Ok(Json(response))
}
