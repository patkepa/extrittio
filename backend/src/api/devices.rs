use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::db::models::{NewDevice, UpdateDevice};
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::repositories::telemetry_repo;
use crate::services::{command_service, device_service};
use crate::state::{AppState, run_db};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DeviceConnectionResponse {
    pub id: String,
    pub label: String,
    pub connection_type: String,
    pub device_id: Option<String>,
    pub external_id: Option<String>,
    pub address: Option<String>,
    pub device_type: Option<String>,
    pub status: Option<String>,
    pub source: Option<String>,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceResponse {
    pub id: String,
    pub name: String,
    pub device_type_id: i32,
    pub device_type_name: String,
    pub device_type_icon: String,
    pub device_type_color_hex: String,
    pub fleet_id: Option<i32>,
    pub fleet_name: Option<String>,
    pub status: String,
    pub last_seen: String,
    pub last_seen_at: Option<String>,
    pub firmware: String,
    pub uptime: String,
    pub uptime_seconds: i32,
    pub latest_latitude: Option<f64>,
    pub latest_longitude: Option<f64>,
    pub declared_connections: Vec<DeviceConnectionResponse>,
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
        Self {
            status: None,
            search: None,
            fleet_id: None,
        }
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
pub struct LocationResponse {
    pub latitude: f64,
    pub longitude: f64,
    pub speed: Option<f32>,
    pub altitude: Option<f32>,
    pub heading: Option<f32>,
    pub timestamp: String,
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

fn to_device_response(
    (device, device_type, fleet): device_service::DeviceWithJoins,
) -> DeviceResponse {
    let last_seen_at = device
        .last_seen
        .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339());
    let declared_connections =
        serde_json::from_value(device.declared_connections.clone()).unwrap_or_default();

    DeviceResponse {
        id: device.id,
        name: device.name,
        device_type_id: device_type.id,
        device_type_name: device_type.name,
        device_type_icon: device_type.icon,
        device_type_color_hex: device_type.color_hex,
        fleet_id: fleet.as_ref().map(|f| f.id),
        fleet_name: fleet.map(|f| f.name),
        status: device.status,
        last_seen: device_service::format_last_seen(device.last_seen)
            .unwrap_or_else(|| "never".to_string()),
        last_seen_at,
        firmware: device.firmware,
        uptime: device_service::format_uptime(Some(device.uptime_seconds))
            .unwrap_or_else(|| "0m".to_string()),
        uptime_seconds: device.uptime_seconds,
        latest_latitude: device.latest_latitude,
        latest_longitude: device.latest_longitude,
        declared_connections,
    }
}

const MAX_BULK_SIZE: usize = 500;

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
        .route(
            "/api/v1/devices/{id}/location/latest",
            get(get_device_latest_location),
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

        let data = results.into_iter().map(to_device_response).collect();

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
        (status = 409, description = "Device name already exists"),
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

        let joined = device_service::get_device(conn, &id_for_read)?;

        Ok(to_device_response(joined))
    })
    .await
    .map_err(map_unique_violation(
        "A device with this name already exists",
    ))?;

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
        (status = 409, description = "Device name already exists"),
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
            firmware: body.firmware,
            updated_at: Some(Utc::now().naive_utc()),
            ..Default::default()
        };

        let joined = device_service::update_device(conn, &id, &changeset)?;

        Ok(to_device_response(joined))
    })
    .await
    .map_err(map_unique_violation(
        "A device with this name already exists",
    ))?;

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
        &state.public_url,
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
        let ids = device_service::resolve_target_ids(
            conn,
            body.device_ids.as_deref(),
            body.select_all.unwrap_or(false),
            body.filters.as_ref().and_then(|f| f.status.as_deref()),
            body.filters.as_ref().and_then(|f| f.search.as_deref()),
            body.filters.as_ref().and_then(|f| f.fleet_id),
            MAX_BULK_SIZE,
        )?;
        let affected = device_service::bulk_change_fleet(conn, &ids, target_fleet_id)?;
        Ok(BulkAffectedResponse {
            affected: affected as i64,
        })
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
        let ids = device_service::resolve_target_ids(
            conn,
            body.device_ids.as_deref(),
            body.select_all.unwrap_or(false),
            body.filters.as_ref().and_then(|f| f.status.as_deref()),
            body.filters.as_ref().and_then(|f| f.search.as_deref()),
            body.filters.as_ref().and_then(|f| f.fleet_id),
            MAX_BULK_SIZE,
        )?;
        let deleted = device_service::bulk_delete(conn, &ids)?;
        Ok(BulkAffectedResponse {
            affected: deleted as i64,
        })
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
        device_service::resolve_target_ids(
            conn,
            body.device_ids.as_deref(),
            body.select_all.unwrap_or(false),
            body.filters.as_ref().and_then(|f| f.status.as_deref()),
            body.filters.as_ref().and_then(|f| f.search.as_deref()),
            body.filters.as_ref().and_then(|f| f.fleet_id),
            MAX_BULK_SIZE,
        )
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

    Ok(Json(BulkResultResponse {
        succeeded,
        failed,
        errors,
    }))
}

/// Bulk trigger OTA firmware update on multiple devices.
pub(crate) async fn bulk_trigger_ota(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkOtaRequest>,
) -> Result<Json<BulkResultResponse>, AppError> {
    let firmware_update_id = body.firmware_update_id;

    let ids = run_db(&state.db_pool, move |conn| {
        device_service::resolve_target_ids(
            conn,
            body.device_ids.as_deref(),
            body.select_all.unwrap_or(false),
            body.filters.as_ref().and_then(|f| f.status.as_deref()),
            body.filters.as_ref().and_then(|f| f.search.as_deref()),
            body.filters.as_ref().and_then(|f| f.fleet_id),
            MAX_BULK_SIZE,
        )
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
            &state.public_url,
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

    Ok(Json(BulkResultResponse {
        succeeded,
        failed,
        errors,
    }))
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

/// Get the latest location recorded for a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/location/latest",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Latest location or null", body = Option<LocationResponse>),
    ),
)]
pub(crate) async fn get_device_latest_location(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> Result<Json<Option<LocationResponse>>, AppError> {
    let result = run_db(&state.db_pool, move |conn| {
        let record = telemetry_repo::get_latest_location(conn, &device_id)?;
        Ok(record.map(|r| LocationResponse {
            latitude: r.latitude.unwrap_or(0.0),
            longitude: r.longitude.unwrap_or(0.0),
            speed: r.speed,
            altitude: r.altitude,
            heading: r.heading,
            timestamp: r.received_at.and_utc().to_rfc3339(),
        }))
    })
    .await?;
    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a `UniqueViolation` database error to `AppError::Conflict` with the
/// given message, passing through all other errors unchanged.
fn map_unique_violation(msg: &'static str) -> impl FnOnce(AppError) -> AppError {
    move |e| match e {
        AppError::Database(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _,
        )) => AppError::Conflict(msg.into()),
        other => other,
    }
}
