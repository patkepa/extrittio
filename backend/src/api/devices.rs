use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{NaiveDateTime, Utc};
use serde_json::Value;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

use crate::api::commands;
use crate::db::models::{Device, DeviceShadow, DeviceType, Fleet, FirmwareUpdate, NewDevice, NewDeviceShadow, NewOtaDeployment, OtaDeployment, UpdateDevice, UpdateShadow};
use crate::db::schema::{device_shadows, device_types, devices, firmware_updates, fleets, ota_deployments};
use crate::shadow_utils::compute_shadow_delta;
use crate::state::AppState;
use extrittio_proto::extrittio::ShadowDelta;

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
) -> Result<Json<Vec<DeviceResponse>>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut query = devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .into_boxed();

    if let Some(ref status) = params.status {
        query = query.filter(devices::status.eq(status));
    }

    if let Some(ref search) = params.search {
        let pattern = format!("%{search}%");
        query = query.filter(
            devices::name
                .like(pattern.clone())
                .or(device_types::name.like(pattern.clone()))
                .or(devices::location.like(pattern)),
        );
    }

    if let Some(fleet_id) = params.fleet_id {
        query = query.filter(devices::fleet_id.eq(fleet_id));
    }

    let results: Vec<(Device, DeviceType, Option<Fleet>)> = query
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .load(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<DeviceResponse> = results
        .into_iter()
        .map(|(d, dt, f)| to_device_response(d, dt, f))
        .collect();

    Ok(Json(response))
}

async fn get_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DeviceResponse>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (device, device_type, fleet): (Device, DeviceType, Option<Fleet>) = devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .filter(devices::id.eq(&id))
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    Ok(Json(to_device_response(device, device_type, fleet)))
}

async fn create_device(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewDeviceRequest>,
) -> Result<(StatusCode, Json<DeviceResponse>), StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let new_id = uuid::Uuid::new_v4().to_string();

    let new_device = NewDevice {
        id: new_id.clone(),
        name: body.name,
        device_type_id: body.device_type_id,
        fleet_id: body.fleet_id,
        location: body.location.unwrap_or_default(),
        firmware: body.firmware.unwrap_or_else(|| "unknown".to_string()),
    };

    diesel::insert_into(devices::table)
        .values(&new_device)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create shadow record for the new device
    let new_shadow = NewDeviceShadow {
        device_id: new_id.clone(),
    };
    diesel::insert_into(device_shadows::table)
        .values(&new_shadow)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (device, device_type, fleet): (Device, DeviceType, Option<Fleet>) = devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .filter(devices::id.eq(&new_id))
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(to_device_response(device, device_type, fleet)),
    ))
}

async fn update_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceResponse>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify device exists
    let _existing: Device = devices::table
        .find(&id)
        .select(Device::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    let changeset = UpdateDevice {
        name: body.name,
        device_type_id: body.device_type_id,
        fleet_id: body.fleet_id,
        location: body.location,
        firmware: body.firmware,
        updated_at: Some(Utc::now().naive_utc()),
        ..Default::default()
    };

    diesel::update(devices::table.find(&id))
        .set(&changeset)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (device, device_type, fleet): (Device, DeviceType, Option<Fleet>) = devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .filter(devices::id.eq(&id))
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(to_device_response(device, device_type, fleet)))
}

async fn delete_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows_deleted = diesel::delete(devices::table.find(&id))
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if rows_deleted == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn restart_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    commands::send_command_internal(&state, &id, "restart", HashMap::default()).await?;
    Ok(StatusCode::OK)
}

async fn trigger_ota(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TriggerOtaRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify device exists and get its device_type_id
    let device: Device = devices::table
        .find(&id)
        .select(Device::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    // Fetch firmware update
    let fw: FirmwareUpdate = firmware_updates::table
        .find(body.firmware_update_id)
        .select(FirmwareUpdate::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    if fw.device_type_id != device.device_type_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Load current shadow
    let shadow: DeviceShadow = device_shadows::table
        .find(&id)
        .select(DeviceShadow::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    // Build OTA desired state: merge firmware update info into desired
    let mut desired: Value =
        serde_json::from_str(&shadow.desired).unwrap_or(Value::Object(serde_json::Map::default()));
    let desired_obj = desired
        .as_object_mut()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut ota_payload = serde_json::json!({
        "firmware_version": fw.version,
        "firmware_url": fw.url,
        "firmware_update_id": fw.id,
    });
    if let Some(ref hash) = fw.sha256 {
        ota_payload["sha256"] = Value::String(hash.clone());
    }
    desired_obj.insert("ota".to_string(), ota_payload);

    let reported: Value =
        serde_json::from_str(&shadow.reported).unwrap_or(Value::Object(serde_json::Map::default()));

    // Compute delta using shared utility
    let new_delta = compute_shadow_delta(&desired, &reported);
    let now = Utc::now().naive_utc();

    let desired_str = serde_json::to_string(&desired).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let delta_str = serde_json::to_string(&new_delta).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let changeset = UpdateShadow {
        desired: Some(desired_str),
        delta: Some(delta_str),
        version: Some(shadow.version + 1),
        updated_at: Some(now),
        ..Default::default()
    };

    diesel::update(device_shadows::table.find(&id))
        .set(&changeset)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create OTA deployment record for tracking
    let new_deployment = NewOtaDeployment {
        device_id: id.clone(),
        firmware_update_id: fw.id,
    };
    diesel::insert_into(ota_deployments::table)
        .values(&new_deployment)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Publish delta to device via Zenoh
    if new_delta.as_object().is_some_and(|obj| !obj.is_empty()) {
        let delta_json = serde_json::to_string(&new_delta).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let delta_msg = ShadowDelta {
            device_id: id.clone(),
            delta_json,
            version: i64::from(shadow.version + 1),
        };
        let payload = prost::Message::encode_to_vec(&delta_msg);
        let topic = format!("extrittio/devices/{id}/shadow/delta");
        if let Err(e) = state.zenoh_session.put(&topic, payload).await {
            warn!("Failed to publish OTA shadow delta to device {}: {}", id, e);
            return Err(StatusCode::BAD_GATEWAY);
        }
    }

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
) -> Result<Json<Vec<OtaDeploymentResponse>>, StatusCode> {
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

    let results: Vec<(OtaDeployment, FirmwareUpdate)> = ota_deployments::table
        .inner_join(firmware_updates::table)
        .filter(ota_deployments::device_id.eq(&id))
        .select((OtaDeployment::as_select(), FirmwareUpdate::as_select()))
        .order(ota_deployments::initiated_at.desc())
        .load(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
