use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::db::models::{Device, DeviceType, Fleet, NewDevice, UpdateDevice};
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::repositories::{device_repo, firmware_repo};
use crate::services::{command_service, device_service};
use crate::state::{AppState, run_db};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceResponse {
    pub id: String,
    pub name: String,
    pub device_type_id: i32,
    pub device_type_name: String,
    pub fleet_id: Option<i32>,
    pub fleet_name: Option<String>,
    pub status: String,
    pub last_seen: String,
    pub last_seen_at: Option<String>,
    pub firmware: String,
    pub uptime: String,
    pub uptime_seconds: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewDeviceRequest {
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub firmware: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub device_type_id: Option<i32>,
    #[schema(value_type = Option<i32>)]
    pub fleet_id: Option<Option<i32>>,
    pub firmware: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListDevicesQuery {
    pub status: Option<String>,
    pub search: Option<String>,
    pub fleet_id: Option<i32>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TriggerOtaRequest {
    pub firmware_update_id: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(default)]
pub struct BulkDeviceFilters {
    pub status: Option<String>,
    pub search: Option<String>,
    pub fleet_id: Option<i32>,
}

impl Default for BulkDeviceFilters {
    fn default() -> Self {
        Self { status: None, search: None, fleet_id: None }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkFleetRequest {
    #[serde(default)]
    pub device_ids: Option<Vec<String>>,
    #[serde(default)]
    pub filters: Option<BulkDeviceFilters>,
    #[serde(default)]
    pub select_all: Option<bool>,
    #[serde(default)]
    #[schema(value_type = Option<i32>)]
    pub fleet_id: Option<Option<i32>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkDeviceRequest {
    #[serde(default)]
    pub device_ids: Option<Vec<String>>,
    #[serde(default)]
    pub filters: Option<BulkDeviceFilters>,
    #[serde(default)]
    pub select_all: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkOtaRequest {
    #[serde(default)]
    pub device_ids: Option<Vec<String>>,
    #[serde(default)]
    pub filters: Option<BulkDeviceFilters>,
    #[serde(default)]
    pub select_all: Option<bool>,
    pub firmware_update_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkAffectedResponse {
    pub affected: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkOperationError {
    pub device_id: String,
    pub error: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkResultResponse {
    pub succeeded: i64,
    pub failed: i64,
    pub errors: Vec<BulkOperationError>,
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
    let last_seen_at = device.last_seen.map(|dt| {
        DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
            .to_rfc3339()
    });
    DeviceResponse {
        id: device.id,
        name: device.name,
        device_type_id: device_type.id,
        device_type_name: device_type.name,
        fleet_id: fleet.as_ref().map(|f| f.id),
        fleet_name: fleet.map(|f| f.name),
        status: device.status,
        last_seen: format_last_seen(device.last_seen),
        last_seen_at,
        firmware: device.firmware,
        uptime: format_uptime(device.uptime_seconds),
        uptime_seconds: device.uptime_seconds,
    }
}

const MAX_BULK_SIZE: usize = 500;

/// Resolve the target device IDs from a bulk request body.
fn resolve_target_ids(
    conn: &mut diesel::SqliteConnection,
    device_ids: Option<Vec<String>>,
    filters: Option<&BulkDeviceFilters>,
    select_all: Option<bool>,
) -> Result<Vec<String>, AppError> {
    let ids = if select_all.unwrap_or(false) {
        let f = filters.unwrap_or(&BulkDeviceFilters {
            status: None,
            search: None,
            fleet_id: None,
        });
        device_repo::resolve_device_ids(
            conn,
            f.status.as_deref(),
            f.search.as_deref(),
            f.fleet_id,
        )?
    } else {
        device_ids.ok_or_else(|| {
            AppError::BadRequest("Either device_ids or select_all with filters is required".into())
        })?
    };

    // Empty results are not an error — handlers return success with 0 counts.
    if ids.len() > MAX_BULK_SIZE {
        return Err(AppError::BadRequest(format!(
            "Too many devices ({}). Maximum is {MAX_BULK_SIZE}. Narrow your filters.",
            ids.len()
        )));
    }

    Ok(ids)
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/devices", get(list_devices).post(create_device))
        .route(
            "/api/v1/devices/{id}",
            get(get_device).put(update_device).delete(delete_device),
        )
        .route("/api/v1/devices/{id}/restart", post(restart_device))
        .route("/api/v1/devices/{id}/ota", post(trigger_ota))
        .route(
            "/api/v1/devices/{id}/ota-deployments",
            get(list_ota_deployments),
        )
        .route("/api/v1/devices/bulk/fleet", post(bulk_change_fleet))
        .route("/api/v1/devices/bulk/delete", post(bulk_delete_devices))
        .route("/api/v1/devices/bulk/restart", post(bulk_restart_devices))
        .route("/api/v1/devices/bulk/ota", post(bulk_trigger_ota))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// List devices with optional filtering.
#[utoipa::path(
    get,
    path = "/api/v1/devices",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(ListDevicesQuery),
    responses(
        (status = 200, description = "Paginated list of devices", body = PaginatedResponse<DeviceResponse>),
    ),
)]
pub(crate) async fn list_devices(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDevicesQuery>,
) -> Result<Json<PaginatedResponse<DeviceResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        let (results, total) = device_repo::list_devices(
            conn,
            params.status.as_deref(),
            params.search.as_deref(),
            params.fleet_id,
            limit,
            offset,
        )?;

        let data = results
            .into_iter()
            .map(|(d, dt, f)| to_device_response(d, dt, f))
            .collect();

        Ok(PaginatedResponse::new(data, total, limit, offset))
    })
    .await?;

    Ok(Json(response))
}

/// Get a device by ID.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Device details", body = DeviceResponse),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn get_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DeviceResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let (device, device_type, fleet) = device_repo::find_device_with_joins(conn, &id)?;
        Ok(to_device_response(device, device_type, fleet))
    })
    .await?;

    Ok(Json(response))
}

/// Create a new device.
#[utoipa::path(
    post,
    path = "/api/v1/devices",
    tag = "devices",
    security(("bearer_auth" = [])),
    request_body = NewDeviceRequest,
    responses(
        (status = 201, description = "Device created", body = DeviceResponse),
        (status = 400, description = "Invalid input"),
    ),
)]
pub(crate) async fn create_device(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewDeviceRequest>,
) -> Result<(StatusCode, Json<DeviceResponse>), AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Device name must not be empty".into()));
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    let id_for_read = new_id.clone();

    let response = run_db(&state.db_pool, move |conn| {
        let new_device = NewDevice {
            id: new_id,
            name: body.name,
            device_type_id: body.device_type_id,
            fleet_id: body.fleet_id,
            firmware: body.firmware.unwrap_or_else(|| "unknown".to_string()),
        };

        device_service::create_device(conn, &new_device)?;

        let (device, device_type, fleet) = device_repo::find_device_with_joins(conn, &id_for_read)?;

        Ok(to_device_response(device, device_type, fleet))
    })
    .await?;

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update a device.
#[utoipa::path(
    put,
    path = "/api/v1/devices/{id}",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    request_body = UpdateDeviceRequest,
    responses(
        (status = 200, description = "Device updated", body = DeviceResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn update_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceResponse>, AppError> {
    if let Some(ref name) = body.name
        && name.trim().is_empty()
    {
        return Err(AppError::BadRequest("Device name must not be empty".into()));
    }

    let response = run_db(&state.db_pool, move |conn| {
        // Verify device exists
        device_repo::find_device(conn, &id)?;

        let changeset = UpdateDevice {
            name: body.name,
            device_type_id: body.device_type_id,
            fleet_id: body.fleet_id,
            firmware: body.firmware,
            updated_at: Some(Utc::now().naive_utc()),
            ..Default::default()
        };

        device_repo::update_device(conn, &id, &changeset)?;

        let (device, device_type, fleet) = device_repo::find_device_with_joins(conn, &id)?;

        Ok(to_device_response(device, device_type, fleet))
    })
    .await?;

    Ok(Json(response))
}

/// Delete a device.
#[utoipa::path(
    delete,
    path = "/api/v1/devices/{id}",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 204, description = "Device deleted"),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn delete_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    run_db(&state.db_pool, move |conn| {
        let deleted = device_repo::delete_device(conn, &id)?;
        if !deleted {
            return Err(AppError::NotFound(format!("Device '{id}' not found")));
        }
        Ok(())
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Restart a device.
#[utoipa::path(
    post,
    path = "/api/v1/devices/{id}/restart",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Restart command sent"),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn restart_device(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    command_service::send_command(
        &state.db_pool,
        &state.zenoh_session,
        &id,
        "restart",
        HashMap::default(),
        &state.zenoh_metrics,
    )
    .await?;
    Ok(StatusCode::OK)
}

/// Trigger an OTA firmware update on a device.
#[utoipa::path(
    post,
    path = "/api/v1/devices/{id}/ota",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    request_body = TriggerOtaRequest,
    responses(
        (status = 200, description = "OTA update triggered"),
        (status = 404, description = "Device or firmware not found"),
    ),
)]
pub(crate) async fn trigger_ota(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TriggerOtaRequest>,
) -> Result<StatusCode, AppError> {
    device_service::trigger_ota(
        &state.db_pool,
        &state.zenoh_session,
        &id,
        body.firmware_update_id,
        &state.zenoh_metrics,
    )
    .await?;
    Ok(StatusCode::OK)
}

/// Bulk change fleet assignment for multiple devices.
pub(crate) async fn bulk_change_fleet(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkFleetRequest>,
) -> Result<Json<BulkAffectedResponse>, AppError> {
    let target_fleet_id = body.fleet_id.ok_or_else(|| {
        AppError::BadRequest("fleet_id is required (use null to unassign)".into())
    })?;

    let response = run_db(&state.db_pool, move |conn| {
        let ids = resolve_target_ids(conn, body.device_ids, body.filters.as_ref(), body.select_all)?;
        let affected = device_repo::bulk_update_fleet(conn, &ids, target_fleet_id, Utc::now().naive_utc())?;
        Ok(BulkAffectedResponse { affected: affected as i64 })
    })
    .await?;

    Ok(Json(response))
}

/// Bulk delete multiple devices.
pub(crate) async fn bulk_delete_devices(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkDeviceRequest>,
) -> Result<Json<BulkAffectedResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let ids = resolve_target_ids(conn, body.device_ids, body.filters.as_ref(), body.select_all)?;
        let deleted = device_repo::bulk_delete_devices(conn, &ids)?;
        Ok(BulkAffectedResponse { affected: deleted as i64 })
    })
    .await?;

    Ok(Json(response))
}

/// Bulk restart multiple devices.
pub(crate) async fn bulk_restart_devices(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkDeviceRequest>,
) -> Result<Json<BulkResultResponse>, AppError> {
    let ids = run_db(&state.db_pool, move |conn| {
        resolve_target_ids(conn, body.device_ids, body.filters.as_ref(), body.select_all)
    })
    .await?;

    let mut succeeded: i64 = 0;
    let mut failed: i64 = 0;
    let mut errors = Vec::new();

    for device_id in &ids {
        match command_service::send_command(
            &state.db_pool,
            &state.zenoh_session,
            device_id,
            "restart",
            HashMap::default(),
            &state.zenoh_metrics,
        )
        .await
        {
            Ok(_) => succeeded += 1,
            Err(e) => {
                failed += 1;
                errors.push(BulkOperationError {
                    device_id: device_id.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(Json(BulkResultResponse { succeeded, failed, errors }))
}

/// Bulk trigger OTA firmware update on multiple devices.
pub(crate) async fn bulk_trigger_ota(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkOtaRequest>,
) -> Result<Json<BulkResultResponse>, AppError> {
    let firmware_update_id = body.firmware_update_id;

    let ids = run_db(&state.db_pool, move |conn| {
        resolve_target_ids(conn, body.device_ids, body.filters.as_ref(), body.select_all)
    })
    .await?;

    let mut succeeded: i64 = 0;
    let mut failed: i64 = 0;
    let mut errors = Vec::new();

    for device_id in &ids {
        match device_service::trigger_ota(
            &state.db_pool,
            &state.zenoh_session,
            device_id,
            firmware_update_id,
            &state.zenoh_metrics,
        )
        .await
        {
            Ok(_) => succeeded += 1,
            Err(e) => {
                failed += 1;
                errors.push(BulkOperationError {
                    device_id: device_id.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(Json(BulkResultResponse { succeeded, failed, errors }))
}

// ---------------------------------------------------------------------------
// OTA Deployment history
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
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

/// List OTA deployments for a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/ota-deployments",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Device ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated OTA deployment history", body = PaginatedResponse<OtaDeploymentResponse>),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn list_ota_deployments(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<OtaDeploymentResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        // Verify device exists
        device_repo::find_device(conn, &id)?;

        let (results, total) = firmware_repo::list_ota_deployments(conn, &id, limit, offset)?;

        let data = results
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
            .collect();

        Ok(PaginatedResponse::new(data, total, limit, offset))
    })
    .await?;

    Ok(Json(response))
}
