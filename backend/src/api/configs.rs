use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::error::AppError;
use crate::repositories::{config_repo, device_repo};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub device_id: String,
    pub config: Value,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    #[serde(flatten)]
    pub entries: serde_json::Map<String, Value>,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/v1/devices/{id}/config",
        get(get_config).put(update_config),
    )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ConfigResponse>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    device_repo::device_exists(&mut conn, &id)?;

    // Get or create config
    let config = config_repo::find_config(&mut conn, &id)?;

    match config {
        Some(c) => {
            let config_val: Value =
                serde_json::from_str(&c.config).unwrap_or(Value::Object(serde_json::Map::default()));
            Ok(Json(ConfigResponse {
                device_id: c.device_id,
                config: config_val,
                updated_at: c.updated_at.and_utc().to_rfc3339(),
            }))
        }
        None => {
            // Return empty config if none exists yet
            Ok(Json(ConfigResponse {
                device_id: id,
                config: Value::Object(serde_json::Map::default()),
                updated_at: Utc::now().to_rfc3339(),
            }))
        }
    }
}

async fn update_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateConfigRequest>,
) -> Result<Json<ConfigResponse>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    device_repo::device_exists(&mut conn, &id)?;

    // Read current config
    let existing = config_repo::find_config(&mut conn, &id)?;

    let current: Value = existing
        .as_ref()
        .map_or(Value::Object(serde_json::Map::default()), |c| serde_json::from_str(&c.config).unwrap_or(Value::Object(serde_json::Map::default())));

    // Merge: null values remove keys, others upsert
    let mut obj = current.as_object().cloned().unwrap_or_default();
    for (key, val) in &body.entries {
        if val.is_null() {
            obj.remove(key);
        } else {
            obj.insert(key.clone(), val.clone());
        }
    }
    let merged = Value::Object(obj);
    let now = Utc::now().naive_utc();

    let config_str = serde_json::to_string(&merged)?;

    let updated = config_repo::upsert_config(&mut conn, &id, &config_str, now)?;

    let config_val: Value =
        serde_json::from_str(&updated.config).unwrap_or(Value::Object(serde_json::Map::default()));

    Ok(Json(ConfigResponse {
        device_id: updated.device_id,
        config: config_val,
        updated_at: updated.updated_at.and_utc().to_rfc3339(),
    }))
}
