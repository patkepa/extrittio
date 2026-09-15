use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::HOST, uri::Authority},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::services::{device_service, firmware_service};
use crate::state::AppState;
use extrittio_backend_core::devices::{DeviceDetails, DeviceListQuery, UpdateDeviceRecord};

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
    pub blueprint_id: String,
    pub blueprint_revision_id: String,
    pub blueprint_key: String,
    pub blueprint_name: String,
    pub blueprint_icon: Option<String>,
    pub blueprint_color: Option<String>,
    pub fleet_id: Option<i32>,
    pub fleet_name: Option<String>,
    pub status: String,
    pub last_seen: String,
    pub last_seen_at: Option<String>,
    pub firmware: String,
    pub uptime: String,
    pub uptime_seconds: i32,
    pub declared_connections: Vec<DeviceConnectionResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewDeviceRequest {
    pub name: String,
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
#[serde(deny_unknown_fields)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    #[schema(value_type = Option<i32>)]
    pub fleet_id: Option<Option<i32>>,
    pub firmware: Option<String>,
}

#[cfg(test)]
mod blueprint_request_tests {
    use super::*;

    #[test]
    fn location_batches_accept_only_explicit_device_ids() {
        let request: DeviceLocationsRequest =
            serde_json::from_value(serde_json::json!({"device_ids":["opaque-device"]})).unwrap();
        assert_eq!(request.device_ids, vec!["opaque-device"]);
        assert!(
            serde_json::from_value::<DeviceLocationsRequest>(
                serde_json::json!({"device_type_id":1})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<DeviceLocationsRequest>(
                serde_json::json!({"device_ids":[],"latitude":0})
            )
            .is_err()
        );
    }

    #[test]
    fn device_responses_expose_blueprint_identity_without_fixed_measurements() {
        use extrittio_backend_core::devices::{DeviceBlueprintIdentity, DeviceRecord};
        let response = to_device_response(DeviceDetails {
            device: DeviceRecord {
                id: "device".into(),
                name: "Device".into(),
                fleet_id: None,
                status: "offline".into(),
                firmware: "1".into(),
                last_seen: None,
                uptime_seconds: 0,
                declared_connections: serde_json::json!([]),
            },
            blueprint: DeviceBlueprintIdentity {
                id: "blueprint".into(),
                revision_id: "revision".into(),
                key: "arbitrary".into(),
                name: "Arbitrary".into(),
                icon: None,
                color: None,
            },
            fleet: None,
        });
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["blueprint_id"], "blueprint");
        assert_eq!(json["blueprint_revision_id"], "revision");
        assert_eq!(json["blueprint_key"], "arbitrary");
        assert!(json["blueprint_icon"].is_null());
        for field in [
            "device_type_id",
            "device_type_name",
            "device_type_icon",
            "device_type_color_hex",
            "latest_latitude",
            "latest_longitude",
            "temperature",
            "humidity",
        ] {
            assert!(json.get(field).is_none(), "{field}");
        }
    }

    #[test]
    fn device_writes_reject_retired_type_selectors() {
        let create = serde_json::json!({"name":"sensor","blueprint_revision_id":"revision"});
        assert!(serde_json::from_value::<NewDeviceRequest>(create.clone()).is_ok());
        for field in ["device_type_id", "device_type", "temperature", "humidity"] {
            let mut input = create.clone();
            input[field] = serde_json::json!(1);
            assert!(
                serde_json::from_value::<NewDeviceRequest>(input).is_err(),
                "{field}"
            );
            assert!(
                serde_json::from_value::<UpdateDeviceRequest>(serde_json::json!({field:1}))
                    .is_err(),
                "{field}"
            );
        }
        assert!(
            serde_json::from_value::<NewDeviceRequest>(serde_json::json!({"name":"sensor"}))
                .is_err()
        );
        assert!(
            serde_json::from_value::<UpdateDeviceRequest>(
                serde_json::json!({"name":"renamed","fleet_id":7})
            )
            .is_ok()
        );
    }
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
    pub contract_id: String,
    pub event_id: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timestamp: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceLocationsRequest {
    pub device_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceLocationResponse {
    pub device_id: String,
    pub location: LocationResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkOperationError {
    pub device_id: String,
    pub error: String,
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct BulkResultResponse {
    pub succeeded: i64,
    pub failed: i64,
    pub errors: Vec<BulkOperationError>,
}

impl BulkResultResponse {
    fn record<T>(&mut self, device_id: &str, result: Result<T, AppError>) {
        match result {
            Ok(_) => self.succeeded += 1,
            Err(error) => {
                self.failed += 1;
                self.errors.push(BulkOperationError {
                    device_id: device_id.to_owned(),
                    error: error.to_string(),
                });
            }
        }
    }
}

fn bulk_target_selection<'a>(
    device_ids: Option<&'a [String]>,
    select_all: Option<bool>,
    filters: Option<&'a BulkDeviceFilters>,
) -> extrittio_backend_core::DeviceTargetSelection<'a> {
    extrittio_backend_core::DeviceTargetSelection {
        device_ids,
        select_all: select_all.unwrap_or(false),
        status: filters.and_then(|filter| filter.status.as_deref()),
        search: filters.and_then(|filter| filter.search.as_deref()),
        fleet_id: filters.and_then(|filter| filter.fleet_id),
        max_size: MAX_BULK_SIZE,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_device_response(
    DeviceDetails {
        device,
        blueprint,
        fleet,
    }: DeviceDetails,
) -> DeviceResponse {
    let last_seen_at = device.last_seen.map(|dt| dt.to_rfc3339());
    let declared_connections =
        serde_json::from_value(device.declared_connections.clone()).unwrap_or_default();

    DeviceResponse {
        id: device.id,
        name: device.name,
        blueprint_id: blueprint.id,
        blueprint_revision_id: blueprint.revision_id,
        blueprint_key: blueprint.key,
        blueprint_name: blueprint.name,
        blueprint_icon: blueprint.icon,
        blueprint_color: blueprint.color,
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
        declared_connections,
    }
}

const MAX_BULK_SIZE: usize = 500;

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/devices/locations/latest",
            post(get_device_locations),
        )
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

    let (results, total) = state
        .application()
        .devices()
        .list(
            &ctx.tenant_context(),
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
        state
            .application()
            .devices()
            .get(&ctx.tenant_context(), &id)
            .await?,
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
    state
        .application()
        .devices()
        .get(&ctx.tenant_context(), &id)
        .await?;
    let contract = state
        .application()
        .devices()
        .assigned_contract(&ctx.tenant_context(), &id)
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
    let created = state
        .application()
        .devices()
        .provision(
            &ctx.tenant_context(),
            extrittio_backend_core::ProvisionDevice {
                name: body.name,
                blueprint_revision_id: body.blueprint_revision_id,
                fleet_id: body.fleet_id,
                firmware: body.firmware,
                configuration: body.configuration,
                automatic_zenoh_endpoint: automatic_zenoh_endpoint(
                    &headers,
                    state.messaging().zenoh_tls_enabled(),
                    state.messaging().zenoh_port(),
                ),
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
    let updated = state
        .application()
        .devices()
        .update(
            &ctx.tenant_context(),
            &id,
            UpdateDeviceRecord {
                name: body.name,
                fleet_id: body.fleet_id,
                firmware: body.firmware,
                updated_at: None,
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
    state
        .application()
        .devices()
        .delete(&ctx.tenant_context(), &id)
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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    state
        .application()
        .commands()
        .send(&ctx.tenant_context(), &id, "restart", serde_json::json!({}))
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
    firmware_service::trigger_ota(
        &ctx,
        state.application().firmware(),
        state.messaging().zenoh_session(),
        &id,
        body.firmware_update_id,
        state.http().public_url(),
        state.http().jwt_secret(),
        state.messaging().zenoh_metrics(),
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

    let affected = state
        .application()
        .assign_selected_devices(
            &ctx.tenant_context(),
            bulk_target_selection(
                body.device_ids.as_deref(),
                body.select_all,
                body.filters.as_ref(),
            ),
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
    let deleted = state
        .application()
        .delete_selected_devices(
            &ctx.tenant_context(),
            bulk_target_selection(
                body.device_ids.as_deref(),
                body.select_all,
                body.filters.as_ref(),
            ),
        )
        .await?;
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
    let outcomes = state
        .application()
        .restart_selected_devices(
            &ctx.tenant_context(),
            bulk_target_selection(
                body.device_ids.as_deref(),
                body.select_all,
                body.filters.as_ref(),
            ),
        )
        .await?;
    let mut result = BulkResultResponse::default();
    for (device_id, outcome) in outcomes {
        result.record(&device_id, outcome.map_err(AppError::from));
    }

    Ok(Json(result))
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
    let publisher = crate::outbound::shadow_delta::ZenohDesiredDeltaPublisher {
        session: state.messaging().zenoh_session(),
        metrics: state.messaging().zenoh_metrics(),
    };
    let outcomes = state
        .application()
        .deploy_selected_devices(
            &ctx.tenant_context(),
            bulk_target_selection(
                body.device_ids.as_deref(),
                body.select_all,
                body.filters.as_ref(),
            ),
            body.firmware_update_id,
            state.http().public_url(),
            &crate::auth::SystemClock,
            &crate::domains::firmware::download::JwtFirmwareDownloadSigner(
                state.http().jwt_secret(),
            ),
            &publisher,
        )
        .await?;
    let mut result = BulkResultResponse::default();
    for (device_id, outcome) in outcomes {
        result.record(&device_id, outcome.map_err(AppError::from));
    }

    Ok(Json(result))
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

    let page = state
        .application()
        .firmware()
        .list_device_deployments(&ctx.tenant_context(), &id, limit, offset)
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

/// Get the latest fresh location declared by the currently assigned contract.
#[utoipa::path(
    post, path = "/api/v1/devices/locations/latest", tag = "devices",
    security(("bearer_auth" = [])), request_body = DeviceLocationsRequest,
    responses((status = 200, description = "Fresh contract locations for up to 500 devices", body = Vec<DeviceLocationResponse>)),
)]
pub(crate) async fn get_device_locations(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(request): Json<DeviceLocationsRequest>,
) -> Result<Json<Vec<DeviceLocationResponse>>, AppError> {
    let locations = state
        .application()
        .events()
        .latest_locations(&ctx.tenant_context(), request.device_ids)
        .await?;
    Ok(Json(
        locations
            .into_iter()
            .map(|r| DeviceLocationResponse {
                device_id: r.device_id,
                location: LocationResponse {
                    contract_id: r.location.contract_id,
                    event_id: r.location.event_id,
                    latitude: r.location.latitude,
                    longitude: r.location.longitude,
                    timestamp: r.location.occurred_at.to_rfc3339(),
                },
            })
            .collect(),
    ))
}

/// Get the latest fresh location declared by the currently assigned contract.
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
        .application()
        .events()
        .latest_location(&ctx.tenant_context(), &device_id)
        .await?
        .map(|r| LocationResponse {
            contract_id: r.contract_id,
            event_id: r.event_id,
            latitude: r.latitude,
            longitude: r.longitude,
            timestamp: r.occurred_at.to_rfc3339(),
        });
    Ok(Json(result))
}
