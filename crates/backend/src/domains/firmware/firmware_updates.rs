use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::domains::device_blueprints::blueprint_service;
use crate::domains::firmware::types::{
    FirmwareRecord, GlobalOtaDeploymentRecord, NewFirmwareBlobRecord, NewFirmwareRecord,
};
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse};
use crate::security;
use crate::services::firmware_service;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct FirmwareUpdateResponse {
    pub id: i32,
    pub device_type_id: i32,
    pub device_type_name: String,
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub has_blob: bool,
    pub file_size: Option<i32>,
    pub filename: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<String>,
    pub changelog: Option<String>,
    pub source: String,
    pub blueprint_revision_id: Option<String>,
    pub compatibility: serde_json::Value,
    pub update_strategy: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewFirmwareUpdateRequest {
    pub blueprint_revision_id: String,
    pub version: Option<String>,
    pub url: String,
    pub sha256: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListFirmwareUpdatesQuery {
    /// Filter by device type.
    pub device_type_id: Option<i32>,
    /// Filter by an immutable device blueprint revision.
    pub blueprint_revision_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NextVersionResponse {
    pub next_version: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListOtaDeploymentsQuery {
    /// Filter by status: all, in_progress, completed, pending, downloading, verifying, installing, success, failed.
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GlobalOtaDeploymentResponse {
    pub id: i32,
    pub device_id: String,
    pub device_name: String,
    pub device_status: String,
    pub current_firmware: String,
    pub device_type_id: i32,
    pub device_type_name: String,
    pub fleet_id: Option<i32>,
    pub fleet_name: Option<String>,
    pub firmware_update_id: i32,
    pub firmware_version: String,
    pub status: String,
    pub error_message: Option<String>,
    pub initiated_at: String,
    pub completed_at: Option<String>,
}

impl From<FirmwareRecord> for FirmwareUpdateResponse {
    fn from(firmware: FirmwareRecord) -> Self {
        Self {
            id: firmware.id,
            device_type_id: firmware.device_type_id,
            device_type_name: firmware.device_type_name,
            version: firmware.version,
            url: firmware.url,
            sha256: firmware.sha256,
            description: firmware.description,
            created_at: firmware.created_at.to_string(),
            has_blob: firmware.file_size.is_some(),
            file_size: firmware.file_size,
            filename: firmware.filename,
            commit_sha: firmware.commit_sha,
            branch: firmware.branch,
            ci_run_url: firmware.ci_run_url,
            build_timestamp: firmware
                .build_timestamp
                .map(|timestamp| timestamp.format("%Y-%m-%dT%H:%M:%S").to_string()),
            changelog: firmware.changelog,
            source: firmware.source,
            blueprint_revision_id: firmware.blueprint_revision_id,
            compatibility: firmware.compatibility,
            update_strategy: firmware.update_strategy,
        }
    }
}

impl From<GlobalOtaDeploymentRecord> for GlobalOtaDeploymentResponse {
    fn from(deployment: GlobalOtaDeploymentRecord) -> Self {
        Self {
            id: deployment.id,
            device_id: deployment.device_id,
            device_name: deployment.device_name,
            device_status: deployment.device_status,
            current_firmware: deployment.current_firmware,
            device_type_id: deployment.device_type_id,
            device_type_name: deployment.device_type_name,
            fleet_id: deployment.fleet_id,
            fleet_name: deployment.fleet_name,
            firmware_update_id: deployment.firmware_update_id,
            firmware_version: deployment.firmware_version,
            status: deployment.status,
            error_message: deployment.error_message,
            initiated_at: deployment.initiated_at.to_string(),
            completed_at: deployment.completed_at.map(|time| time.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(max_firmware_size: usize) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/ota-deployments", get(list_all_ota_deployments))
        .route(
            "/api/v1/firmware-updates",
            get(list_firmware_updates).post(create_firmware_update),
        )
        .route(
            "/api/v1/firmware-updates/upload",
            axum::routing::post(upload_firmware_update)
                .layer(DefaultBodyLimit::max(max_firmware_size)),
        )
        .route(
            "/api/v1/firmware-updates/{id}",
            axum::routing::delete(delete_firmware_update),
        )
        .route(
            "/api/v1/firmware-updates/{id}/download",
            get(download_firmware_blob),
        )
        .route(
            "/api/v1/firmware-updates/next-version/{device_type_id}",
            get(get_next_version),
        )
        .route(
            "/api/v1/firmware-updates/next-version/blueprint/{revision_id}",
            get(get_next_blueprint_version),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// List firmware updates.
#[utoipa::path(
    get,
    path = "/api/v1/firmware-updates",
    tag = "firmware",
    security(("bearer_auth" = [])),
    params(ListFirmwareUpdatesQuery),
    responses(
        (status = 200, description = "Paginated list of firmware updates", body = PaginatedResponse<FirmwareUpdateResponse>),
    ),
)]
pub(crate) async fn list_firmware_updates(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<ListFirmwareUpdatesQuery>,
) -> Result<Json<PaginatedResponse<FirmwareUpdateResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let page = firmware_service::list_with_repository(
        &ctx,
        state.persistence.firmware.as_ref(),
        params.device_type_id,
        params.blueprint_revision_id,
        limit,
        offset,
    )
    .await?;
    let response = PaginatedResponse::new(
        page.records.into_iter().map(Into::into).collect(),
        page.total,
        limit,
        offset,
    );

    Ok(Json(response))
}

/// List OTA deployments across all devices.
#[utoipa::path(
    get,
    path = "/api/v1/ota-deployments",
    tag = "firmware",
    security(("bearer_auth" = [])),
    params(ListOtaDeploymentsQuery),
    responses(
        (status = 200, description = "Paginated list of OTA deployments", body = PaginatedResponse<GlobalOtaDeploymentResponse>),
    ),
)]
pub(crate) async fn list_all_ota_deployments(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<ListOtaDeploymentsQuery>,
) -> Result<Json<PaginatedResponse<GlobalOtaDeploymentResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let page = firmware_service::list_all_deployments_with_repository(
        &ctx,
        state.persistence.firmware.as_ref(),
        params.status,
        limit,
        offset,
    )
    .await?;
    let response = PaginatedResponse::new(
        page.records.into_iter().map(Into::into).collect(),
        page.total,
        limit,
        offset,
    );

    Ok(Json(response))
}

/// Register a firmware update (URL-based, no file upload).
#[utoipa::path(
    post,
    path = "/api/v1/firmware-updates",
    tag = "firmware",
    security(("bearer_auth" = [])),
    request_body = NewFirmwareUpdateRequest,
    responses(
        (status = 201, description = "Firmware update created", body = FirmwareUpdateResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Device type not found"),
        (status = 409, description = "Version already exists"),
    ),
)]
pub(crate) async fn create_firmware_update(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<NewFirmwareUpdateRequest>,
) -> Result<(StatusCode, Json<FirmwareUpdateResponse>), AppError> {
    if body.url.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Firmware URL must not be empty".into(),
        ));
    }
    security::validate_public_https_url(&body.url, "Firmware URL")?;
    if !body
        .sha256
        .as_deref()
        .is_some_and(|hash| hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(AppError::BadRequest(
            "SHA-256 is required for external firmware URLs".into(),
        ));
    }

    let revision = blueprint_service::get_revision(
        &ctx,
        state.persistence.device_blueprints.as_ref(),
        &body.blueprint_revision_id,
    )
    .await?;
    let blueprint: extrittio_device_contract::DeviceBlueprint =
        serde_json::from_value(revision.document).map_err(|error| {
            AppError::Internal(format!(
                "Stored device blueprint revision is invalid: {error}"
            ))
        })?;
    let firmware_definition = blueprint.spec.firmware.ok_or_else(|| {
        AppError::UnprocessableEntity(
            "The selected blueprint does not declare firmware update behavior".into(),
        )
    })?;
    let update_strategy = serde_json::to_value(firmware_definition.strategy)?
        .as_str()
        .map(ToOwned::to_owned);
    let compatibility = serde_json::to_value(firmware_definition.compatibility)?;
    let compatibility_type = crate::services::device_type_service::resolve_for_device_creation(
        &ctx,
        state.persistence.device_types.as_ref(),
        None,
    )
    .await?;

    let version = match body.version {
        Some(version) if !version.trim().is_empty() => version.trim().to_string(),
        _ => {
            firmware_service::next_blueprint_version_with_repository(
                &ctx,
                state.persistence.firmware.as_ref(),
                &body.blueprint_revision_id,
            )
            .await?
        }
    };
    let created = firmware_service::create_with_repository(
        &ctx,
        state.persistence.firmware.as_ref(),
        NewFirmwareRecord {
            device_type_id: compatibility_type.id,
            version,
            url: body.url,
            sha256: body.sha256,
            description: body.description,
            commit_sha: None,
            branch: None,
            ci_run_url: None,
            build_timestamp: None,
            changelog: None,
            source: None,
            blueprint_revision_id: Some(body.blueprint_revision_id),
            compatibility,
            update_strategy,
        },
        None,
    )
    .await?;
    let response = created.into();

    Ok((StatusCode::CREATED, Json(response)))
}

/// Upload firmware binary as multipart form data.
#[utoipa::path(
    post,
    path = "/api/v1/firmware-updates/upload",
    tag = "firmware",
    security(("bearer_auth" = [])),
    request_body(content_type = "multipart/form-data", description = "Multipart form: blueprint_revision_id (required), version (optional), description (optional), file (required)"),
    responses(
        (status = 201, description = "Firmware uploaded", body = FirmwareUpdateResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Device type not found"),
        (status = 409, description = "Version already exists"),
    ),
)]
#[allow(clippy::too_many_lines)]
pub(crate) async fn upload_firmware_update(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<FirmwareUpdateResponse>), AppError> {
    let mut blueprint_revision_id: Option<String> = None;
    let mut version: Option<String> = None;
    let mut description: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "blueprint_revision_id" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?;
                if !text.trim().is_empty() {
                    blueprint_revision_id = Some(text.trim().to_string());
                }
            }
            "version" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?;
                if !text.trim().is_empty() {
                    version = Some(text.trim().to_string());
                }
            }
            "description" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?;
                if !text.trim().is_empty() {
                    description = Some(text.trim().to_string());
                }
            }
            "file" => {
                filename = field.file_name().map(std::string::ToString::to_string);
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let blueprint_revision_id = blueprint_revision_id
        .ok_or_else(|| AppError::BadRequest("Missing blueprint_revision_id".into()))?;
    let file_data = file_data.ok_or_else(|| AppError::BadRequest("Missing file".into()))?;

    let revision = blueprint_service::get_revision(
        &ctx,
        state.persistence.device_blueprints.as_ref(),
        &blueprint_revision_id,
    )
    .await?;
    let blueprint: extrittio_device_contract::DeviceBlueprint =
        serde_json::from_value(revision.document).map_err(|error| {
            AppError::Internal(format!(
                "Stored device blueprint revision is invalid: {error}"
            ))
        })?;
    let firmware_definition = blueprint.spec.firmware.ok_or_else(|| {
        AppError::UnprocessableEntity(
            "The selected blueprint does not declare firmware update behavior".into(),
        )
    })?;
    let update_strategy = serde_json::to_value(firmware_definition.strategy)?
        .as_str()
        .map(ToOwned::to_owned);
    let compatibility = serde_json::to_value(firmware_definition.compatibility)?;
    let compatibility_type = crate::services::device_type_service::resolve_for_device_creation(
        &ctx,
        state.persistence.device_types.as_ref(),
        None,
    )
    .await?;

    // Sanitize filename: strip path components to prevent traversal
    let filename = filename
        .as_deref()
        .and_then(|f| f.rsplit(['/', '\\']).next())
        .filter(|f| !f.is_empty())
        .unwrap_or("firmware.bin")
        .to_string();

    if file_data.is_empty() {
        return Err(AppError::BadRequest("File is empty".into()));
    }

    // Compute SHA-256
    let sha256_hex = Sha256::digest(&file_data)
        .iter()
        .fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        });

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let file_size = file_data.len() as i32;

    let version = match version {
        Some(version) => version,
        None => {
            firmware_service::next_blueprint_version_with_repository(
                &ctx,
                state.persistence.firmware.as_ref(),
                &blueprint_revision_id,
            )
            .await?
        }
    };

    let storage_key = state
        .firmware_store
        .allocate_key(ctx.tenant_id_str(), &filename);
    state
        .firmware_store
        .put(&storage_key, file_data)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Firmware object upload failed");
            AppError::Internal("Firmware storage is unavailable".to_string())
        })?;

    let storage_backend = state.firmware_store.backend().to_string();
    let response = firmware_service::create_with_repository(
        &ctx,
        state.persistence.firmware.as_ref(),
        NewFirmwareRecord {
            device_type_id: compatibility_type.id,
            version,
            url: String::new(),
            sha256: Some(sha256_hex),
            description,
            commit_sha: None,
            branch: None,
            ci_run_url: None,
            build_timestamp: None,
            changelog: None,
            source: None,
            blueprint_revision_id: Some(blueprint_revision_id),
            compatibility,
            update_strategy,
        },
        Some(NewFirmwareBlobRecord {
            size: file_size,
            filename,
            storage_key: storage_key.clone(),
            storage_backend,
        }),
    )
    .await
    .map(FirmwareUpdateResponse::from);

    let response = match response {
        Ok(response) => response,
        Err(error) => {
            if let Err(cleanup_error) = state.firmware_store.delete(&storage_key).await {
                tracing::error!(%cleanup_error, "Failed to clean up unreferenced firmware object");
            }
            return Err(error);
        }
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Download a firmware binary blob.
#[utoipa::path(
    get,
    path = "/api/v1/firmware-updates/{id}/download",
    tag = "firmware",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Firmware update ID")),
    responses(
        (status = 200, description = "Firmware binary", content_type = "application/octet-stream"),
        (status = 404, description = "Firmware not found"),
    ),
)]
pub(crate) async fn download_firmware_blob(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Response, AppError> {
    let blob =
        firmware_service::get_blob_with_repository(&ctx, state.persistence.firmware.as_ref(), id)
            .await?;

    let data = match (blob.data, blob.storage_key.as_deref()) {
        (Some(data), _) => data,
        (None, Some(storage_key)) => {
            if blob.storage_backend != state.firmware_store.backend() {
                tracing::error!(
                    stored_backend = %blob.storage_backend,
                    configured_backend = state.firmware_store.backend(),
                    "Firmware object backend does not match the configured store"
                );
                return Err(AppError::Internal(
                    "Firmware storage configuration does not match stored metadata".to_string(),
                ));
            }
            state
                .firmware_store
                .get(storage_key)
                .await
                .map_err(|error| {
                    tracing::error!(%error, firmware_update_id = id, "Firmware object download failed");
                    AppError::Internal("Firmware storage is unavailable".to_string())
                })?
        }
        (None, None) => {
            return Err(AppError::Internal(
                "Firmware blob has no storage location".to_string(),
            ));
        }
    };

    if data.len() != blob.size as usize {
        tracing::error!(
            firmware_update_id = id,
            expected_size = blob.size,
            actual_size = data.len(),
            "Firmware object size does not match metadata"
        );
        return Err(AppError::Internal(
            "Firmware object failed integrity validation".to_string(),
        ));
    }

    // Sanitize filename for Content-Disposition to prevent header injection
    let safe_filename: String = blob
        .filename
        .chars()
        .filter(|c| *c != '"' && *c != '\r' && *c != '\n' && *c != '\0')
        .collect();
    let content_disposition = format!("attachment; filename=\"{}\"", safe_filename);

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .header(header::CONTENT_LENGTH, blob.size.to_string())
        .body(Body::from(data))
        .unwrap())
}

/// Delete a firmware update.
#[utoipa::path(
    delete,
    path = "/api/v1/firmware-updates/{id}",
    tag = "firmware",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Firmware update ID")),
    responses(
        (status = 204, description = "Firmware update deleted"),
        (status = 404, description = "Firmware update not found"),
    ),
)]
pub(crate) async fn delete_firmware_update(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    let blob =
        firmware_service::delete_with_repository(&ctx, state.persistence.firmware.as_ref(), id)
            .await?;

    if let Some(blob) = blob
        && let Some(storage_key) = blob.storage_key
    {
        if blob.storage_backend == state.firmware_store.backend() {
            if let Err(error) = state.firmware_store.delete(&storage_key).await {
                tracing::error!(
                    %error,
                    firmware_update_id = id,
                    "Firmware metadata deleted but object cleanup failed"
                );
            }
        } else {
            tracing::error!(
                firmware_update_id = id,
                stored_backend = %blob.storage_backend,
                configured_backend = state.firmware_store.backend(),
                "Firmware metadata deleted but object backend was not configured for cleanup"
            );
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Get the next auto-generated version for a device type.
#[utoipa::path(
    get,
    path = "/api/v1/firmware-updates/next-version/{device_type_id}",
    tag = "firmware",
    security(("bearer_auth" = [])),
    params(("device_type_id" = i32, Path, description = "Device type ID")),
    responses(
        (status = 200, description = "Next version string", body = NextVersionResponse),
    ),
)]
pub(crate) async fn get_next_version(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(device_type_id): Path<i32>,
) -> Result<Json<NextVersionResponse>, AppError> {
    let version = firmware_service::next_version_with_repository(
        &ctx,
        state.persistence.firmware.as_ref(),
        device_type_id,
    )
    .await?;

    Ok(Json(NextVersionResponse {
        next_version: version,
    }))
}

/// Get the next auto-generated version for a published blueprint revision.
#[utoipa::path(
    get,
    path = "/api/v1/firmware-updates/next-version/blueprint/{revision_id}",
    tag = "firmware",
    security(("bearer_auth" = [])),
    params(("revision_id" = String, Path, description = "Published device blueprint revision ID")),
    responses(
        (status = 200, description = "Next version string", body = NextVersionResponse),
    ),
)]
pub(crate) async fn get_next_blueprint_version(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(revision_id): Path<String>,
) -> Result<Json<NextVersionResponse>, AppError> {
    let version = firmware_service::next_blueprint_version_with_repository(
        &ctx,
        state.persistence.firmware.as_ref(),
        &revision_id,
    )
    .await?;

    Ok(Json(NextVersionResponse {
        next_version: version,
    }))
}
