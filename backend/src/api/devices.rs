use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::models::{Device, DeviceType, Fleet, NewDevice, UpdateDevice};
use crate::error::AppError;
use crate::repositories::{device_repo, firmware_repo};
use crate::services::{command_service, device_service};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DeviceResponse {
    pub id: String,
    pub name: String,
    pub device_type_id: i32,
    pub device_type_name: String,
    pub fleet_id: Option<i32>,
    pub fleet_name: Option<String>,
    pub status: String,
    pub last_seen: String,
    pub firmware: String,
    pub location: String,
    pub uptime: String,
}

#[derive(Debug, Deserialize)]
pub struct NewDeviceRequest {
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub location: Option<String>,
    pub firmware: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub device_type_id: Option<i32>,
    pub fleet_id: Option<Option<i32>>,
    pub location: Option<String>,
    pub firmware: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListDevicesQuery {
    pub status: Option<String>,
    pub search: Option<String>,
    pub fleet_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct TriggerOtaRequest {
    pub firmware_update_id: i32,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_uptime(seconds: i32) -> String {
    if seconds <= 0 {
        return "0m".to_string();
    }
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h")
    } else {
        format!("{minutes}m")
    }
}

fn format_last_seen(last_seen: Option<NaiveDateTime>) -> String {
    match last_seen {
        None => "never".to_string(),
        Some(dt) => {
            let now = Utc::now().naive_utc();
            let duration = now.signed_duration_since(dt);
            let secs = duration.num_seconds();

            if secs < 60 {
                "just now".to_string()
            } else if secs < 3600 {
                let mins = secs / 60;
                if mins == 1 {
                    "1 minute ago".to_string()
                } else {
                    format!("{mins} minutes ago")
                }
            } else if secs < 86400 {
                let hours = secs / 3600;
                if hours == 1 {
                    "1 hour ago".to_string()
                } else {
                    format!("{hours} hours ago")
                }
            } else {
                let days = secs / 86400;
                if days == 1 {
                    "1 day ago".to_string()
                } else {
                    format!("{days} days ago")
                }
            }
        }
    }
}

fn to_device_response(
    device: Device,
    device_type: DeviceType,
    fleet: Option<Fleet>,
) -> DeviceResponse {
    DeviceResponse {
        id: device.id,
        name: device.name,
        device_type_id: device_type.id,
        device_type_name: device_type.name,
        fleet_id: fleet.as_ref().map(|f| f.id),
        fleet_name: fleet.map(|f| f.name),
        status: device.status,
        last_seen: format_last_seen(device.last_seen),
        firmware: device.firmware,
        location: device.location,
        uptime: format_uptime(device.uptime_seconds),
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/devices", get(list_devices).post(create_device))
        .route(
            "/api/devices/{id}",
            get(get_device).put(update_device).delete(delete_device),
        )
        .route("/api/devices/{id}/restart", post(restart_device))
        .route("/api/devices/{id}/ota", post(trigger_ota))
        .route(
            "/api/devices/{id}/ota-deployments",
            get(list_ota_deployments),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_devices(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDevicesQuery>,
) -> Result<Json<Vec<DeviceResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    let results = device_repo::list_devices(
        &mut conn,
        params.status.as_deref(),
        params.search.as_deref(),
        params.fleet_id,
    )?;

    let response: Vec<DeviceResponse> = results
        .into_iter()
        .map(|(d, dt, f)| to_device_response(d, dt, f))
        .collect();

    Ok(Json(response))
}

async fn get_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DeviceResponse>, AppError> {
    let mut conn = state.db_pool.get()?;

    let (device, device_type, fleet) = device_repo::find_device_with_joins(&mut conn, &id)?;

    Ok(Json(to_device_response(device, device_type, fleet)))
}

async fn create_device(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewDeviceRequest>,
) -> Result<(StatusCode, Json<DeviceResponse>), AppError> {
    let mut conn = state.db_pool.get()?;

    let new_id = uuid::Uuid::new_v4().to_string();

    let new_device = NewDevice {
        id: new_id.clone(),
        name: body.name,
        device_type_id: body.device_type_id,
        fleet_id: body.fleet_id,
        location: body.location.unwrap_or_default(),
        firmware: body.firmware.unwrap_or_else(|| "unknown".to_string()),
    };

    device_service::create_device(&mut conn, &new_device)?;

    let (device, device_type, fleet) =
        device_repo::find_device_with_joins(&mut conn, &new_id)?;

    Ok((
        StatusCode::CREATED,
        Json(to_device_response(device, device_type, fleet)),
    ))
}

async fn update_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceResponse>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    let _existing = device_repo::find_device(&mut conn, &id)?;

    let changeset = UpdateDevice {
        name: body.name,
        device_type_id: body.device_type_id,
        fleet_id: body.fleet_id,
        location: body.location,
        firmware: body.firmware,
        updated_at: Some(Utc::now().naive_utc()),
        ..Default::default()
    };

    device_repo::update_device(&mut conn, &id, &changeset)?;

    let (device, device_type, fleet) =
        device_repo::find_device_with_joins(&mut conn, &id)?;

    Ok(Json(to_device_response(device, device_type, fleet)))
}

async fn delete_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db_pool.get()?;

    let deleted = device_repo::delete_device(&mut conn, &id)?;

    if !deleted {
        return Err(AppError::NotFound(format!("Device '{id}' not found")));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn restart_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db_pool.get()?;
    command_service::send_command(&mut conn, &state.zenoh_session, &id, "restart", HashMap::default()).await?;
    Ok(StatusCode::OK)
}

async fn trigger_ota(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TriggerOtaRequest>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db_pool.get()?;
    device_service::trigger_ota(&mut conn, &state.zenoh_session, &id, body.firmware_update_id).await?;
    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// OTA Deployment history
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct OtaDeploymentResponse {
    pub id: i32,
    pub device_id: String,
    pub firmware_update_id: i32,
    pub firmware_version: String,
    pub status: String,
    pub error_message: Option<String>,
    pub initiated_at: String,
    pub completed_at: Option<String>,
}

async fn list_ota_deployments(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<OtaDeploymentResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    device_repo::find_device(&mut conn, &id)?;

    let results = firmware_repo::list_ota_deployments(&mut conn, &id)?;

    Ok(Json(
        results
            .into_iter()
            .map(|(dep, fw)| OtaDeploymentResponse {
                id: dep.id,
                device_id: dep.device_id,
                firmware_update_id: dep.firmware_update_id,
                firmware_version: fw.version,
                status: dep.status,
                error_message: dep.error_message,
                initiated_at: dep.initiated_at.to_string(),
                completed_at: dep.completed_at.map(|t| t.to_string()),
            })
            .collect(),
    ))
}
