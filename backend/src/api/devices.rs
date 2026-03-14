use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::db::models::{NewDevice, UpdateDevice};
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
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
    pub firmware: String,
    pub location: String,
    pub uptime: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewDeviceRequest {
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub location: Option<String>,
    pub firmware: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub device_type_id: Option<i32>,
    #[schema(value_type = Option<i32>)]
    pub fleet_id: Option<Option<i32>>,
    pub location: Option<String>,
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_device_response(
    (device, device_type, fleet): crate::repositories::device_repo::DeviceWithJoins,
) -> DeviceResponse {
    DeviceResponse {
        id: device.id,
        name: device.name,
        device_type_id: device_type.id,
        device_type_name: device_type.name,
        fleet_id: fleet.as_ref().map(|f| f.id),
        fleet_name: fleet.map(|f| f.name),
        status: device.status,
        last_seen: device_service::format_last_seen(device.last_seen)
            .unwrap_or_else(|| "never".to_string()),
        firmware: device.firmware,
        location: device.location,
        uptime: device_service::format_uptime(Some(device.uptime_seconds))
            .unwrap_or_else(|| "0m".to_string()),
    }
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
        let (results, total) = device_service::list_devices(
            conn,
            params.status.as_deref(),
            params.search.as_deref(),
            params.fleet_id,
            limit,
            offset,
        )?;

        let data = results
            .into_iter()
            .map(to_device_response)
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
        let joined = device_service::get_device(conn, &id)?;
        Ok(to_device_response(joined))
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
            location: body.location.unwrap_or_default(),
            firmware: body.firmware.unwrap_or_else(|| "unknown".to_string()),
        };

        device_service::create_device(conn, &new_device)?;

        let joined = device_service::get_device(conn, &id_for_read)?;

        Ok(to_device_response(joined))
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
        let changeset = UpdateDevice {
            name: body.name,
            device_type_id: body.device_type_id,
            fleet_id: body.fleet_id,
            location: body.location,
            firmware: body.firmware,
            updated_at: Some(Utc::now().naive_utc()),
            ..Default::default()
        };

        let joined = device_service::update_device(conn, &id, &changeset)?;

        Ok(to_device_response(joined))
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
        device_service::delete_device(conn, &id)
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
    )
    .await?;
    Ok(StatusCode::OK)
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
        let (results, total) = device_service::list_ota_deployments(conn, &id, limit, offset)?;

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
