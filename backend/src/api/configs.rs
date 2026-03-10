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

use crate::db::models::{DeviceConfig, NewDeviceConfig};
use crate::db::schema::{device_configs, devices};
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
        "/api/devices/{id}/config",
        get(get_config).put(update_config),
    )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ConfigResponse>, StatusCode> {
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

    // Get or create config
    let config = device_configs::table
        .find(&id)
        .select(DeviceConfig::as_select())
        .first(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
) -> Result<Json<ConfigResponse>, StatusCode> {
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

    // Read current config
    let existing = device_configs::table
        .find(&id)
        .select(DeviceConfig::as_select())
        .first(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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

    let config_str =
        serde_json::to_string(&merged).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if existing.is_some() {
        diesel::update(device_configs::table.find(&id))
            .set((
                device_configs::config.eq(&config_str),
                device_configs::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        diesel::insert_into(device_configs::table)
            .values(&NewDeviceConfig {
                device_id: id.clone(),
                config: config_str,
            })
            .execute(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Re-read
    let updated = device_configs::table
        .find(&id)
        .select(DeviceConfig::as_select())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let config_val: Value =
        serde_json::from_str(&updated.config).unwrap_or(Value::Object(serde_json::Map::default()));

    Ok(Json(ConfigResponse {
        device_id: updated.device_id,
        config: config_val,
        updated_at: updated.updated_at.and_utc().to_rfc3339(),
    }))
}
