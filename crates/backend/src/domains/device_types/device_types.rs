use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::domains::device_types::types::DeviceTypeRecord;
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::services::device_type_service;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceTypeResponse {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub color_hex: String,
}

impl From<DeviceTypeRecord> for DeviceTypeResponse {
    fn from(dt: DeviceTypeRecord) -> Self {
        Self {
            id: dt.id,
            name: dt.name,
            icon: dt.icon,
            color_hex: dt.color_hex,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewDeviceTypeRequest {
    pub name: String,
    pub icon: Option<String>,
    pub color_hex: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDeviceTypeRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color_hex: Option<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/device-types",
            get(list_device_types).post(create_device_type),
        )
        .route(
            "/api/v1/device-types/{id}",
            patch(update_device_type).delete(delete_device_type),
        )
}

/// List all device types.
#[utoipa::path(
    get,
    path = "/api/v1/device-types",
    tag = "device-types",
    security(("bearer_auth" = [])),
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated list of device types", body = PaginatedResponse<DeviceTypeResponse>),
    ),
)]
pub(crate) async fn list_device_types(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<DeviceTypeResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let (results, total) =
        device_type_service::list(&ctx, state.persistence.device_types.as_ref(), limit, offset)
            .await?;
    let data = results.into_iter().map(DeviceTypeResponse::from).collect();
    let response = PaginatedResponse::new(data, total, limit, offset);

    Ok(Json(response))
}

/// Create a new device type.
#[utoipa::path(
    post,
    path = "/api/v1/device-types",
    tag = "device-types",
    security(("bearer_auth" = [])),
    request_body = NewDeviceTypeRequest,
    responses(
        (status = 201, description = "Device type created", body = DeviceTypeResponse),
        (status = 400, description = "Invalid input"),
    ),
)]
pub(crate) async fn create_device_type(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<NewDeviceTypeRequest>,
) -> Result<(StatusCode, Json<DeviceTypeResponse>), AppError> {
    let created = device_type_service::create(
        &ctx,
        state.persistence.device_types.as_ref(),
        &body.name,
        body.icon.as_deref(),
        body.color_hex.as_deref(),
    )
    .await?;
    let response = DeviceTypeResponse::from(created);

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update a device type.
#[utoipa::path(
    patch,
    path = "/api/v1/device-types/{id}",
    tag = "device-types",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Device type ID")),
    request_body = UpdateDeviceTypeRequest,
    responses(
        (status = 200, description = "Device type updated", body = DeviceTypeResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Device type not found"),
    ),
)]
pub(crate) async fn update_device_type(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<i32>,
    Json(body): Json<UpdateDeviceTypeRequest>,
) -> Result<Json<DeviceTypeResponse>, AppError> {
    let updated = device_type_service::update(
        &ctx,
        state.persistence.device_types.as_ref(),
        id,
        body.name.as_deref(),
        body.icon.as_deref(),
        body.color_hex.as_deref(),
    )
    .await?;
    let response = DeviceTypeResponse::from(updated);

    Ok(Json(response))
}

/// Delete a device type by ID.
#[utoipa::path(
    delete,
    path = "/api/v1/device-types/{id}",
    tag = "device-types",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Device type ID")),
    responses(
        (status = 204, description = "Device type deleted"),
        (status = 404, description = "Device type not found"),
        (status = 409, description = "Devices still reference this type"),
        (status = 422, description = "Cannot delete the default type"),
    ),
)]
pub(crate) async fn delete_device_type(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    device_type_service::delete(&ctx, state.persistence.device_types.as_ref(), id).await?;

    Ok(StatusCode::NO_CONTENT)
}
