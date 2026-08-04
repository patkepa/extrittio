use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::domains::shadows::types::ShadowRecord;
use crate::error::AppError;
use crate::services::shadow_service;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct ShadowResponse {
    pub device_id: String,
    #[schema(value_type = HashMap<String, Value>)]
    pub desired: Value,
    #[schema(value_type = HashMap<String, Value>)]
    pub reported: Value,
    #[schema(value_type = HashMap<String, Value>)]
    pub delta: Value,
    pub version: i32,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateShadowRequest {
    #[serde(flatten)]
    pub state: serde_json::Map<String, Value>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_shadow_response(shadow: ShadowRecord) -> ShadowResponse {
    ShadowResponse {
        device_id: shadow.device_id,
        desired: shadow.desired,
        reported: shadow.reported,
        delta: shadow.delta,
        version: shadow.version,
        updated_at: shadow.updated_at.naive_utc().to_string(),
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/devices/{id}/shadow",
            get(get_shadow).delete(delete_shadow),
        )
        .route(
            "/api/v1/devices/{id}/shadow/desired",
            axum::routing::put(update_desired),
        )
        .route(
            "/api/v1/devices/{id}/shadow/reported",
            axum::routing::put(update_reported),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Get the device shadow (desired, reported, delta).
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/shadow",
    tag = "shadows",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Device shadow", body = ShadowResponse),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn get_shadow(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<ShadowResponse>, AppError> {
    let shadow = shadow_service::get_shadow(&ctx, state.persistence.shadows.as_ref(), &id).await?;
    let response = to_shadow_response(shadow);

    Ok(Json(response))
}

/// Update the desired state of a device shadow.
#[utoipa::path(
    put,
    path = "/api/v1/devices/{id}/shadow/desired",
    tag = "shadows",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    request_body = Object,
    responses(
        (status = 200, description = "Shadow updated", body = ShadowResponse),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn update_desired(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, AppError> {
    let updated = shadow_service::update_desired(
        &ctx,
        state.persistence.shadows.as_ref(),
        &state.zenoh_session,
        &id,
        &body.state,
        &state.zenoh_metrics,
    )
    .await?;
    let response = to_shadow_response(updated);

    Ok(Json(response))
}

/// Update the reported state of a device shadow.
#[utoipa::path(
    put,
    path = "/api/v1/devices/{id}/shadow/reported",
    tag = "shadows",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    request_body = Object,
    responses(
        (status = 200, description = "Shadow updated", body = ShadowResponse),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn update_reported(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, AppError> {
    let updated =
        shadow_service::update_reported(&ctx, state.persistence.shadows.as_ref(), &id, &body.state)
            .await?;
    let response = to_shadow_response(updated);

    Ok(Json(response))
}

/// Reset a device shadow to empty state.
#[utoipa::path(
    delete,
    path = "/api/v1/devices/{id}/shadow",
    tag = "shadows",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 204, description = "Shadow reset"),
        (status = 404, description = "Shadow not found"),
    ),
)]
pub(crate) async fn delete_shadow(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    shadow_service::delete_shadow(&ctx, state.persistence.shadows.as_ref(), &id).await?;

    Ok(StatusCode::NO_CONTENT)
}
