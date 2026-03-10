use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::db::models::{DeviceShadow, UpdateShadow};
use crate::error::AppError;
use crate::repositories::{device_repo, shadow_repo};
use crate::services::shadow_service;
use crate::state::{run_db, AppState};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ShadowResponse {
    pub device_id: String,
    pub desired: Value,
    pub reported: Value,
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

async fn get_shadow(
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

async fn update_desired(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, AppError> {
    shadow_service::update_desired(&state.db_pool, &state.zenoh_session, &id, &body.state).await?;

    let id_clone = id;
    let response = run_db(&state.db_pool, move |conn| {
        let updated = shadow_repo::find_shadow(conn, &id_clone)?;
        Ok(to_shadow_response(updated))
    })
    .await?;

    Ok(Json(response))
}

async fn update_reported(
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

async fn delete_shadow(
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
