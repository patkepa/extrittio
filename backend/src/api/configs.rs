use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::error::AppError;
use crate::services::config_service;
use crate::state::{AppState, run_db};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct ConfigResponse {
    pub device_id: String,
    #[schema(value_type = HashMap<String, Value>)]
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

/// Get the configuration for a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/config",
    tag = "config",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Device configuration", body = ConfigResponse),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ConfigResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let config = config_service::get_config(conn, &id)?;

        match config {
            Some(c) => {
                let config_val: Value = serde_json::from_str(&c.config)
                    .unwrap_or(Value::Object(serde_json::Map::default()));
                Ok(ConfigResponse {
                    device_id: c.device_id,
                    config: config_val,
                    updated_at: c.updated_at.and_utc().to_rfc3339(),
                })
            }
            None => {
                // Return empty config if none exists yet
                Ok(ConfigResponse {
                    device_id: id,
                    config: Value::Object(serde_json::Map::default()),
                    updated_at: Utc::now().to_rfc3339(),
                })
            }
        }
    })
    .await?;

    Ok(Json(response))
}

/// Update the configuration for a device (merge semantics).
#[utoipa::path(
    put,
    path = "/api/v1/devices/{id}/config",
    tag = "config",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    request_body = Object,
    responses(
        (status = 200, description = "Configuration updated", body = ConfigResponse),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn update_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateConfigRequest>,
) -> Result<Json<ConfigResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let updated = config_service::merge_and_update(conn, &id, &body.entries)?;

        let config_val: Value = serde_json::from_str(&updated.config)
            .unwrap_or(Value::Object(serde_json::Map::default()));

        Ok(ConfigResponse {
            device_id: updated.device_id,
            config: config_val,
            updated_at: updated.updated_at.and_utc().to_rfc3339(),
        })
    })
    .await?;

    Ok(Json(response))
}
