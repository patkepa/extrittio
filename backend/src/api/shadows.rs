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
use tracing::warn;
use std::sync::Arc;

use crate::db::models::{DeviceShadow, UpdateShadow};
use crate::db::schema::{device_shadows, devices};
use crate::shadow_utils::compute_shadow_delta;
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

fn merge_json(existing: &Value, patch: &serde_json::Map<String, Value>) -> Value {
    let mut obj = existing.as_object().cloned().unwrap_or_default();
    for (key, val) in patch {
        if val.is_null() {
            obj.remove(key);
        } else {
            obj.insert(key.clone(), val.clone());
        }
    }
    Value::Object(obj)
}

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
            "/api/devices/{id}/shadow",
            get(get_shadow).delete(delete_shadow),
        )
        .route(
            "/api/devices/{id}/shadow/desired",
            axum::routing::put(update_desired),
        )
        .route(
            "/api/devices/{id}/shadow/reported",
            axum::routing::put(update_reported),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn get_shadow(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ShadowResponse>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify device exists
    devices::table
        .find(&id)
        .select(devices::id)
        .first::<String>(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    let shadow: DeviceShadow = device_shadows::table
        .find(&id)
        .select(DeviceShadow::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    Ok(Json(to_shadow_response(shadow)))
}

async fn update_desired(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let shadow: DeviceShadow = device_shadows::table
        .find(&id)
        .select(DeviceShadow::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    let current_desired: Value =
        serde_json::from_str(&shadow.desired).unwrap_or(Value::Object(serde_json::Map::default()));
    let current_reported: Value =
        serde_json::from_str(&shadow.reported).unwrap_or(Value::Object(serde_json::Map::default()));

    let new_desired = merge_json(&current_desired, &body.state);
    let new_delta = compute_shadow_delta(&new_desired, &current_reported);
    let now = Utc::now().naive_utc();

    let desired_str = serde_json::to_string(&new_desired).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let delta_str = serde_json::to_string(&new_delta).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let changeset = UpdateShadow {
        desired: Some(desired_str),
        delta: Some(delta_str.clone()),
        version: Some(shadow.version + 1),
        updated_at: Some(now),
        ..UpdateShadow::default()
    };

    diesel::update(device_shadows::table.find(&id))
        .set(&changeset)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Publish delta to device via Zenoh if non-empty
    if new_delta.as_object().is_some_and(|obj| !obj.is_empty()) {
        let delta_msg = extrittio_proto::extrittio::ShadowDelta {
            device_id: id.clone(),
            delta_json: delta_str,
            version: i64::from(shadow.version + 1),
        };
        let payload = prost::Message::encode_to_vec(&delta_msg);
        let topic = format!("extrittio/devices/{id}/shadow/delta");
        if let Err(e) = state.zenoh_session.put(&topic, payload).await {
            warn!("Failed to publish shadow delta to device {}: {}", id, e);
        }
    }

    // Re-read updated shadow
    let updated: DeviceShadow = device_shadows::table
        .find(&id)
        .select(DeviceShadow::as_select())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(to_shadow_response(updated)))
}

async fn update_reported(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateShadowRequest>,
) -> Result<Json<ShadowResponse>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let shadow: DeviceShadow = device_shadows::table
        .find(&id)
        .select(DeviceShadow::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    let current_desired: Value =
        serde_json::from_str(&shadow.desired).unwrap_or(Value::Object(serde_json::Map::default()));
    let current_reported: Value =
        serde_json::from_str(&shadow.reported).unwrap_or(Value::Object(serde_json::Map::default()));

    let new_reported = merge_json(&current_reported, &body.state);
    let new_delta = compute_shadow_delta(&current_desired, &new_reported);
    let now = Utc::now().naive_utc();

    let reported_str = serde_json::to_string(&new_reported).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let delta_str = serde_json::to_string(&new_delta).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let changeset = UpdateShadow {
        reported: Some(reported_str),
        delta: Some(delta_str),
        version: Some(shadow.version + 1),
        updated_at: Some(now),
        ..UpdateShadow::default()
    };

    diesel::update(device_shadows::table.find(&id))
        .set(&changeset)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let updated: DeviceShadow = device_shadows::table
        .find(&id)
        .select(DeviceShadow::as_select())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(to_shadow_response(updated)))
}

async fn delete_shadow(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let now = Utc::now().naive_utc();
    let changeset = UpdateShadow {
        desired: Some("{}".to_string()),
        reported: Some("{}".to_string()),
        delta: Some("{}".to_string()),
        version: Some(1),
        updated_at: Some(now),
    };

    let rows = diesel::update(device_shadows::table.find(&id))
        .set(&changeset)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if rows == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}
