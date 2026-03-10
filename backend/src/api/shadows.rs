use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::db::models::{DeviceShadow, UpdateShadow};
use crate::db::schema::devices;
use crate::error::AppError;
use crate::repositories::shadow_repo;
use crate::services::shadow_service;
use crate::state::AppState;

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
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    devices::table
        .find(&id)
        .select(devices::id)
        .first::<String>(&mut conn)?;

    let shadow = shadow_repo::find_shadow(&mut conn, &id)?;

    Ok(Json(to_shadow_response(shadow)))
}

async fn update_desired(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, AppError> {
    let mut conn = state.db_pool.get()?;

    shadow_service::update_desired(&mut conn, &state.zenoh_session, &id, &body.state).await?;

    let updated = shadow_repo::find_shadow(&mut conn, &id)?;
    Ok(Json(to_shadow_response(updated)))
}

async fn update_reported(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, AppError> {
    let mut conn = state.db_pool.get()?;

    shadow_service::update_reported(&mut conn, &id, &body.state)?;

    let updated = shadow_repo::find_shadow(&mut conn, &id)?;
    Ok(Json(to_shadow_response(updated)))
}

async fn delete_shadow(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db_pool.get()?;

    let now = Utc::now().naive_utc();
    let changeset = UpdateShadow {
        desired: Some("{}".to_string()),
        reported: Some("{}".to_string()),
        delta: Some("{}".to_string()),
        version: Some(1),
        updated_at: Some(now),
    };

    let rows = shadow_repo::update_shadow(&mut conn, &id, &changeset)?;

    if rows == 0 {
        return Err(AppError::NotFound(format!("Shadow for device '{id}' not found")));
    }

    Ok(StatusCode::NO_CONTENT)
}
