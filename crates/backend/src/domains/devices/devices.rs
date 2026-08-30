use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::HOST, uri::Authority},
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::domains::device_blueprints::blueprint_service;
use crate::domains::devices::types::{
    CreateDeviceRecord, DeviceDetails, DeviceListQuery, UpdateDeviceRecord,
};
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::services::{
    command_service, device_catalog_service, device_service, device_type_service, firmware_service,
};
use crate::state::AppState;

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
    /// Deprecated compatibility selector. Omit for blueprint-based devices.
    pub device_type_id: Option<i32>,
    pub fleet_id: Option<i32>,
    pub firmware: Option<String>,
    /// Published immutable blueprint revision used to compile this device's contract.
    pub blueprint_revision_id: String,
    /// Device-specific configuration overlay, validated against the blueprint schema.
    pub configuration: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceContractResponse {
    pub id: String,
    pub device_id: String,
    pub blueprint_revision_id: String,
    pub contract_hash: String,
    pub assignment_status: String,
    pub acknowledged_at: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub document: serde_json::Value,
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

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(default)]
pub struct BulkDeviceFilters {
    pub status: Option<String>,
    pub search: Option<String>,
    pub fleet_id: Option<i32>,
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
    DeviceDetails {
        device,
        device_type,
        fleet,
    }: DeviceDetails,
) -> DeviceResponse {
    let last_seen_at = device.last_seen.map(|dt| dt.to_rfc3339());
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
        last_seen: device_service::format_last_seen(
            device.last_seen.map(|value| value.naive_utc()),
        )
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
        .route("/api/v1/devices/{id}/contract", get(get_device_contract))
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
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<ListDevicesQuery>,
) -> Result<Json<PaginatedResponse<DeviceResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let (results, total) = device_catalog_service::list(
        &ctx,
        state.persistence.devices.as_ref(),
        DeviceListQuery {
            status: params.status,
            search: params.search,
            fleet_id: params.fleet_id,
            limit,
            offset,
        },
    )
    .await?;
    let data = results.into_iter().map(to_device_response).collect();
    let response = PaginatedResponse::new(data, total, limit, offset);

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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<DeviceResponse>, AppError> {
    let response = to_device_response(
        device_catalog_service::get(&ctx, state.persistence.devices.as_ref(), &id).await?,
    );

    Ok(Json(response))
}

/// Get the immutable contract currently assigned to a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/contract",
    tag = "devices",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Assigned device contract", body = DeviceContractResponse),
        (status = 404, description = "Device or contract not found"),
    ),
)]
pub(crate) async fn get_device_contract(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<DeviceContractResponse>, AppError> {
    // Ensure the device itself exists in this tenant so legacy devices and
    // unknown IDs have stable, tenant-safe not-found behavior.
    device_catalog_service::get(&ctx, state.persistence.devices.as_ref(), &id).await?;
    let contract =
        device_catalog_service::assigned_contract(&ctx, state.persistence.devices.as_ref(), &id)
            .await?;
    Ok(Json(DeviceContractResponse {
        id: contract.id,
        device_id: contract.device_id,
        blueprint_revision_id: contract.blueprint_revision_id,
        contract_hash: contract.contract_hash,
        assignment_status: contract.assignment_status,
        acknowledged_at: contract.acknowledged_at.map(|value| value.to_rfc3339()),
        error: contract.error,
        created_at: contract.created_at.to_rfc3339(),
        document: contract.document,
    }))
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
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    Json(body): Json<NewDeviceRequest>,
) -> Result<(StatusCode, Json<DeviceResponse>), AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Device name must not be empty".into()));
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    let automatic_zenoh_endpoint =
        automatic_zenoh_endpoint(&headers, state.zenoh_tls_enabled, state.zenoh_port);
    let contract = blueprint_service::compile_device_contract(
        &ctx,
        state.persistence.device_blueprints.as_ref(),
        &body.blueprint_revision_id,
        uuid::Uuid::new_v4().to_string(),
        new_id.clone(),
        automatic_zenoh_endpoint,
        body.configuration,
    )
    .await?;
    let compatibility_type = device_type_service::resolve_for_device_creation(
        &ctx,
        state.persistence.device_types.as_ref(),
        body.device_type_id,
    )
    .await?;
    let created = device_catalog_service::create(
        &ctx,
        state.persistence.devices.as_ref(),
        state.persistence.certificates.as_ref(),
        CreateDeviceRecord {
            id: new_id,
            name: body.name,
            device_type_id: compatibility_type.id,
            fleet_id: body.fleet_id,
            firmware: body.firmware.unwrap_or_else(|| "unknown".to_string()),
            contract: Some(contract),
        },
    )
    .await?;
    let response = to_device_response(created);

    Ok((StatusCode::CREATED, Json(response)))
}

fn automatic_zenoh_endpoint(headers: &HeaderMap, tls_enabled: bool, port: u16) -> String {
    let host = headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Authority>().ok())
        .map(|authority| authority.host().to_string())
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
    let scheme = if tls_enabled { "tls" } else { "tcp" };
    format!("{scheme}/{host}:{port}")
}

#[cfg(test)]
mod automatic_endpoint_tests {
    use super::*;

    #[test]
    fn derives_tcp_endpoint_from_http_host_without_reusing_the_http_port() {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, "hub.example.test:8080".parse().unwrap());

        assert_eq!(
            automatic_zenoh_endpoint(&headers, false, 7447),
            "tcp/hub.example.test:7447"
        );
    }

    #[test]
    fn derives_bracketed_ipv6_tls_endpoint() {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, "[fd12:3456::20]:8080".parse().unwrap());

        assert_eq!(
            automatic_zenoh_endpoint(&headers, true, 7448),
            "tls/[fd12:3456::20]:7448"
        );
    }
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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceResponse>, AppError> {
    if let Some(ref name) = body.name
        && name.trim().is_empty()
    {
        return Err(AppError::BadRequest("Device name must not be empty".into()));
    }

    let updated = device_catalog_service::update(
        &ctx,
        state.persistence.devices.as_ref(),
        &id,
        UpdateDeviceRecord {
            name: body.name,
            device_type_id: body.device_type_id,
            fleet_id: body.fleet_id,
            firmware: body.firmware,
            updated_at: Some(Utc::now()),
        },
    )
    .await?;
    let response = to_device_response(updated);

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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    device_catalog_service::delete(&ctx, state.persistence.devices.as_ref(), &id).await?;

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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    command_service::send_command_as_user_with_repository(
        &ctx,
        state.persistence.commands.as_ref(),
        state.persistence.devices.as_ref(),
        &state.zenoh_session,
        &id,
        "restart",
        serde_json::json!({}),
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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<TriggerOtaRequest>,
) -> Result<StatusCode, AppError> {
    firmware_service::trigger_ota_with_repository(
        &ctx,
        state.persistence.firmware.as_ref(),
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
#[utoipa::path(
    post, path = "/api/v1/devices/bulk/fleet", tag = "devices", security(("bearer_auth" = [])),
    request_body = BulkFleetRequest, responses((status = 200, body = BulkAffectedResponse))
)]
pub(crate) async fn bulk_change_fleet(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<BulkFleetRequest>,
) -> Result<Json<BulkAffectedResponse>, AppError> {
    let target_fleet_id = body.fleet_id.ok_or_else(|| {
        AppError::BadRequest("fleet_id is required (use null to unassign)".into())
    })?;

    let ids = device_catalog_service::resolve_target_ids(
        &ctx,
        state.persistence.devices.as_ref(),
        device_catalog_service::DeviceTargetSelection {
            device_ids: body.device_ids.as_deref(),
            select_all: body.select_all.unwrap_or(false),
            status: body.filters.as_ref().and_then(|f| f.status.as_deref()),
            search: body.filters.as_ref().and_then(|f| f.search.as_deref()),
            fleet_id: body.filters.as_ref().and_then(|f| f.fleet_id),
            max_size: MAX_BULK_SIZE,
        },
    )
    .await?;
    let affected = device_catalog_service::bulk_assign_fleet(
        &ctx,
        state.persistence.devices.as_ref(),
        ids,
        target_fleet_id,
    )
    .await?;
    let response = BulkAffectedResponse {
        affected: affected as i64,
    };

    Ok(Json(response))
}

/// Bulk delete multiple devices.
#[utoipa::path(
    post, path = "/api/v1/devices/bulk/delete", tag = "devices", security(("bearer_auth" = [])),
    request_body = BulkDeviceRequest, responses((status = 200, body = BulkAffectedResponse))
)]
pub(crate) async fn bulk_delete_devices(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<BulkDeviceRequest>,
) -> Result<Json<BulkAffectedResponse>, AppError> {
    let ids = device_catalog_service::resolve_target_ids(
        &ctx,
        state.persistence.devices.as_ref(),
        device_catalog_service::DeviceTargetSelection {
            device_ids: body.device_ids.as_deref(),
            select_all: body.select_all.unwrap_or(false),
            status: body.filters.as_ref().and_then(|f| f.status.as_deref()),
            search: body.filters.as_ref().and_then(|f| f.search.as_deref()),
            fleet_id: body.filters.as_ref().and_then(|f| f.fleet_id),
            max_size: MAX_BULK_SIZE,
        },
    )
    .await?;
    let deleted =
        device_catalog_service::bulk_delete(&ctx, state.persistence.devices.as_ref(), ids).await?;
    let response = BulkAffectedResponse {
        affected: deleted as i64,
    };

    Ok(Json(response))
}

/// Bulk restart multiple devices.
#[utoipa::path(
    post, path = "/api/v1/devices/bulk/restart", tag = "devices", security(("bearer_auth" = [])),
    request_body = BulkDeviceRequest, responses((status = 200, body = BulkResultResponse))
)]
pub(crate) async fn bulk_restart_devices(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<BulkDeviceRequest>,
) -> Result<Json<BulkResultResponse>, AppError> {
    command_service::authorize_send_commands(&ctx)?;

    let ids = device_catalog_service::resolve_target_ids(
        &ctx,
        state.persistence.devices.as_ref(),
        device_catalog_service::DeviceTargetSelection {
            device_ids: body.device_ids.as_deref(),
            select_all: body.select_all.unwrap_or(false),
            status: body.filters.as_ref().and_then(|f| f.status.as_deref()),
            search: body.filters.as_ref().and_then(|f| f.search.as_deref()),
            fleet_id: body.filters.as_ref().and_then(|f| f.fleet_id),
            max_size: MAX_BULK_SIZE,
        },
    )
    .await?;

    let mut succeeded: i64 = 0;
    let mut failed: i64 = 0;
    let mut errors = Vec::new();

    for device_id in &ids {
        match command_service::send_command_as_user_with_repository(
            &ctx,
            state.persistence.commands.as_ref(),
            state.persistence.devices.as_ref(),
            &state.zenoh_session,
            device_id,
            "restart",
            serde_json::json!({}),
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
#[utoipa::path(
    post, path = "/api/v1/devices/bulk/ota", tag = "devices", security(("bearer_auth" = [])),
    request_body = BulkOtaRequest, responses((status = 200, body = BulkResultResponse))
)]
pub(crate) async fn bulk_trigger_ota(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<BulkOtaRequest>,
) -> Result<Json<BulkResultResponse>, AppError> {
    let firmware_update_id = body.firmware_update_id;
    device_service::authorize_deploy_firmware(&ctx)?;

    let ids = device_catalog_service::resolve_target_ids(
        &ctx,
        state.persistence.devices.as_ref(),
        device_catalog_service::DeviceTargetSelection {
            device_ids: body.device_ids.as_deref(),
            select_all: body.select_all.unwrap_or(false),
            status: body.filters.as_ref().and_then(|f| f.status.as_deref()),
            search: body.filters.as_ref().and_then(|f| f.search.as_deref()),
            fleet_id: body.filters.as_ref().and_then(|f| f.fleet_id),
            max_size: MAX_BULK_SIZE,
        },
    )
    .await?;

    let mut succeeded: i64 = 0;
    let mut failed: i64 = 0;
    let mut errors = Vec::new();

    for device_id in &ids {
        match firmware_service::trigger_ota_with_repository(
            &ctx,
            state.persistence.firmware.as_ref(),
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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<OtaDeploymentResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let page = firmware_service::list_device_deployments_with_repository(
        &ctx,
        state.persistence.firmware.as_ref(),
        &id,
        limit,
        offset,
    )
    .await?;
    let data = page
        .records
        .into_iter()
        .map(|deployment| OtaDeploymentResponse {
            id: deployment.id,
            device_id: deployment.device_id,
            firmware_update_id: deployment.firmware_update_id,
            firmware_version: deployment.firmware_version,
            status: deployment.status,
            error_message: deployment.error_message,
            initiated_at: deployment.initiated_at.to_string(),
            completed_at: deployment.completed_at.map(|time| time.to_string()),
        })
        .collect();
    let response = PaginatedResponse::new(data, page.total, limit, offset);

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
    Extension(ctx): Extension<RequestContext>,
    Path(device_id): Path<String>,
) -> Result<Json<Option<LocationResponse>>, AppError> {
    let result = state
        .persistence
        .telemetry
        .latest_location(ctx.tenant_id(), &device_id)
        .await?
        .map(|r| LocationResponse {
            latitude: r.latitude.unwrap_or(0.0),
            longitude: r.longitude.unwrap_or(0.0),
            speed: r.speed,
            altitude: r.altitude,
            heading: r.heading,
            timestamp: r.received_at.and_utc().to_rfc3339(),
        });
    Ok(Json(result))
}
