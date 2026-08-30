use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::domains::activity::types::{ActivityEventRecord, ActivityQuery};
use crate::error::AppError;
use crate::services::activity_service;
use crate::state::AppState;
use crate::util;

#[derive(Debug, Deserialize, IntoParams)]
pub struct ActivityQueryParams {
    /// Event origin: device, audit, alert, command, or deployment.
    pub source: Option<String>,
    /// Normalized severity: debug, info, warning, or error.
    pub severity: Option<String>,
    /// Domain category, such as device, users, rules, or firmware-updates.
    pub category: Option<String>,
    /// Restrict events to one device when a device identity is available.
    pub device_id: Option<String>,
    /// Case-insensitive search across messages, event types, actors, resources, and request IDs.
    pub search: Option<String>,
    /// Only return events at or after this timestamp.
    pub since: Option<String>,
    /// Only return events at or before this timestamp.
    pub until: Option<String>,
    /// Maximum rows to return (default 100, max 200).
    pub limit: Option<i64>,
    /// Pagination offset.
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityEventResponse {
    /// Stable source-qualified event identifier.
    pub id: String,
    /// Originating stream: device, audit, alert, command, or deployment.
    pub source: String,
    /// Normalized severity: debug, info, warning, or error.
    pub severity: String,
    /// Machine-readable event name.
    pub event_type: String,
    /// Product domain associated with the event.
    pub category: String,
    /// Human-readable event summary.
    pub message: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub request_id: Option<String>,
    pub metadata: serde_json::Value,
    pub occurred_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityEventListResponse {
    pub data: Vec<ActivityEventResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl From<ActivityEventRecord> for ActivityEventResponse {
    fn from(event: ActivityEventRecord) -> Self {
        Self {
            id: event.id,
            source: event.source,
            severity: event.severity,
            event_type: event.event_type,
            category: event.category,
            message: event.message,
            actor_type: event.actor_type,
            actor_id: event.actor_id,
            resource_type: event.resource_type,
            resource_id: event.resource_id,
            request_id: event.request_id,
            metadata: event.metadata,
            occurred_at: event.occurred_at.and_utc().to_rfc3339(),
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/activity-events", get(list_activity_events))
}

#[utoipa::path(
    get,
    path = "/api/v1/activity-events",
    tag = "logs",
    security(("bearer_auth" = [])),
    params(ActivityQueryParams),
    responses(
        (status = 200, description = "Tenant-wide operational and audit event stream", body = ActivityEventListResponse),
        (status = 400, description = "Invalid event filter"),
    ),
)]
pub(crate) async fn list_activity_events(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<ActivityQueryParams>,
) -> Result<Json<ActivityEventListResponse>, AppError> {
    let limit = params.limit.unwrap_or(100).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).clamp(0, 100_000);
    let page = activity_service::list(
        &ctx,
        state.persistence.activity.as_ref(),
        ActivityQuery {
            source: params.source,
            severity: params.severity,
            category: params.category,
            device_id: params.device_id,
            search: params.search,
            since: util::parse_timestamp(params.since.as_deref())?,
            until: util::parse_timestamp(params.until.as_deref())?,
            limit,
            offset,
        },
    )
    .await?;

    Ok(Json(ActivityEventListResponse {
        data: page.data.into_iter().map(Into::into).collect(),
        total: page.total,
        limit,
        offset,
    }))
}
