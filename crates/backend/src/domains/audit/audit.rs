use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct AuditEventResponse {
    pub id: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub outcome: String,
    pub request_id: String,
    pub metadata: serde_json::Value,
    pub occurred_at: String,
}

#[derive(Serialize, ToSchema)]
pub struct AuditEventListResponse {
    pub data: Vec<AuditEventResponse>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/audit-events", get(list_audit_events))
}

#[utoipa::path(
    get,
    path = "/api/v1/audit-events",
    tag = "audit",
    security(("bearer_auth" = [])),
    params(
        ("limit" = Option<i64>, Query, description = "Maximum rows, from 1 to 200"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
    ),
    responses(
        (status = 200, description = "Tenant audit events", body = AuditEventListResponse),
    ),
)]
pub(crate) async fn list_audit_events(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditEventListResponse>, AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let rows = state
        .application()
        .audit()
        .list(&ctx.tenant_context(), limit, offset)
        .await?;

    Ok(Json(AuditEventListResponse {
        data: rows
            .into_iter()
            .map(|event| AuditEventResponse {
                id: event.id,
                actor_type: event.actor_type,
                actor_id: event.actor_id,
                action: event.action,
                resource_type: event.resource_type,
                resource_id: event.resource_id,
                outcome: event.outcome,
                request_id: event.request_id,
                metadata: event.metadata,
                occurred_at: event.occurred_at.and_utc().to_rfc3339(),
            })
            .collect(),
    }))
}
