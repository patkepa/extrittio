use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::db::models::{DeviceShadow, UpdateShadow};
use crate::error::AppError;
use crate::repositories::{device_repo, shadow_repo};
use crate::services::shadow_service;
use crate::state::{AppState, run_db};

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

fn to_shadow_response(shadow: DeviceShadow) -> ShadowResponse {
    ShadowResponse {
        device_id: shadow.device_id,
        desired: serde_json::from_str(&shadow.desired)
            .unwrap_or(Value::Object(serde_json::Map::default())),
        reported: serde_json::from_str(&shadow.reported)
            .unwrap_or(Value::Object(serde_json::Map::default())),
        delta: serde_json::from_str(&shadow.delta)
            .unwrap_or(Value::Object(serde_json::Map::default())),
        version: shadow.version,
        updated_at: shadow.updated_at.to_string(),
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
    Path(id): Path<String>,
) -> Result<Json<ShadowResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        // Verify device exists
        device_repo::find_device(conn, &id)?;
        let shadow = shadow_repo::find_shadow(conn, &id)?;
        Ok(to_shadow_response(shadow))
    })
    .await?;

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
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, AppError> {
    shadow_service::update_desired(&state.db_pool, &state.zenoh_session, &id, &body.state, &state.zenoh_metrics).await?;

    let id_clone = id;
    let response = run_db(&state.db_pool, move |conn| {
        let updated = shadow_repo::find_shadow(conn, &id_clone)?;
        Ok(to_shadow_response(updated))
    })
    .await?;

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
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        shadow_service::update_reported(conn, &id, &body.state)?;
        let updated = shadow_repo::find_shadow(conn, &id)?;
        Ok(to_shadow_response(updated))
    })
    .await?;

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
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    run_db(&state.db_pool, move |conn| {
        let now = Utc::now().naive_utc();
        let changeset = UpdateShadow {
            desired: Some("{}".to_string()),
            reported: Some("{}".to_string()),
            delta: Some("{}".to_string()),
            version: Some(1),
            updated_at: Some(now),
        };

        let rows = shadow_repo::update_shadow(conn, &id, &changeset)?;

        if rows == 0 {
            return Err(AppError::NotFound(format!(
                "Shadow for device '{id}' not found"
            )));
        }

        Ok(())
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
