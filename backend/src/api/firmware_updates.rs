use axum::{
    Json, Router,
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

use crate::db::models::{NewFirmwareBlob, NewFirmwareUpdate};
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse};
use crate::services::{device_type_service, firmware_service};
use crate::state::{AppState, run_db};

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
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewFirmwareUpdateRequest {
    pub device_type_id: i32,
    pub version: Option<String>,
    pub url: String,
    pub sha256: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListFirmwareUpdatesQuery {
    /// Filter by device type.
    pub device_type_id: Option<i32>,
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

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(max_firmware_size: usize) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/ota-deployments", get(list_ota_deployments))
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
    Query(params): Query<ListFirmwareUpdatesQuery>,
) -> Result<Json<PaginatedResponse<FirmwareUpdateResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        let (results, total) = firmware_service::list(conn, params.device_type_id, limit, offset)?;

        let data = results
            .into_iter()
            .map(
                |(fw, dt, blob_size, blob_filename)| FirmwareUpdateResponse {
                    id: fw.id,
                    device_type_id: fw.device_type_id,
                    device_type_name: dt.name,
                    version: fw.version,
                    url: fw.url,
                    sha256: fw.sha256,
                    description: fw.description,
                    created_at: fw.created_at.to_string(),
                    has_blob: blob_size.is_some(),
                    file_size: blob_size,
                    filename: blob_filename,
                    commit_sha: fw.commit_sha,
                    branch: fw.branch,
                    ci_run_url: fw.ci_run_url,
                    build_timestamp: fw
                        .build_timestamp
                        .map(|ts| ts.format("%Y-%m-%dT%H:%M:%S").to_string()),
                    changelog: fw.changelog,
                    source: fw.source,
                },
            )
            .collect();

        Ok(PaginatedResponse::new(data, total, limit, offset))
    })
    .await?;

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
pub(crate) async fn list_ota_deployments(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListOtaDeploymentsQuery>,
) -> Result<Json<PaginatedResponse<GlobalOtaDeploymentResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        let (results, total) = firmware_service::list_all_ota_deployments(
            conn,
            params.status.as_deref(),
            limit,
            offset,
        )?;

        let data = results
            .into_iter()
            .map(|(dep, fw, device, dt, fleet)| GlobalOtaDeploymentResponse {
                id: dep.id,
                device_id: dep.device_id,
                device_name: device.name,
                device_status: device.status,
                current_firmware: device.firmware,
                device_type_id: dt.id,
                device_type_name: dt.name,
                fleet_id: fleet.as_ref().map(|f| f.id),
                fleet_name: fleet.map(|f| f.name),
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
    Json(body): Json<NewFirmwareUpdateRequest>,
) -> Result<(StatusCode, Json<FirmwareUpdateResponse>), AppError> {
    if body.url.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Firmware URL must not be empty".into(),
        ));
    }

    let response = run_db(&state.db_pool, move |conn| {
        // Verify device type exists
        let dt = device_type_service::find_by_id(conn, body.device_type_id)?;

        // Auto-generate version if not provided
        let version = match body.version {
            Some(v) if !v.trim().is_empty() => v.trim().to_string(),
            _ => firmware_service::next_version_for_type(conn, body.device_type_id)?,
        };

        let new_fw = NewFirmwareUpdate {
            device_type_id: body.device_type_id,
            version: version.clone(),
            url: body.url,
            sha256: body.sha256,
            description: body.description,
            commit_sha: None,
            branch: None,
            ci_run_url: None,
            build_timestamp: None,
            changelog: None,
            source: None,
        };

        let created = firmware_service::register_firmware(conn, &new_fw).map_err(
            map_unique_violation("Firmware version already exists for this device type"),
        )?;

        Ok(FirmwareUpdateResponse {
            id: created.id,
            device_type_id: created.device_type_id,
            device_type_name: dt.name,
            version: created.version,
            url: created.url,
            sha256: created.sha256,
            description: created.description,
            created_at: created.created_at.to_string(),
            has_blob: false,
            file_size: None,
            filename: None,
            commit_sha: created.commit_sha,
            branch: created.branch,
            ci_run_url: created.ci_run_url,
            build_timestamp: created
                .build_timestamp
                .map(|ts| ts.format("%Y-%m-%dT%H:%M:%S").to_string()),
            changelog: created.changelog,
            source: created.source,
        })
    })
    .await?;

    Ok((StatusCode::CREATED, Json(response)))
}

/// Upload firmware binary as multipart form data.
#[utoipa::path(
    post,
    path = "/api/v1/firmware-updates/upload",
    tag = "firmware",
    security(("bearer_auth" = [])),
    request_body(content_type = "multipart/form-data", description = "Multipart form: device_type_id (required), version (optional), description (optional), file (required)"),
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
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<FirmwareUpdateResponse>), AppError> {
    let mut device_type_id: Option<i32> = None;
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
            "device_type_id" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?;
                device_type_id = Some(
                    text.parse()
                        .map_err(|_| AppError::BadRequest("Invalid device_type_id".into()))?,
                );
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

    let device_type_id =
        device_type_id.ok_or_else(|| AppError::BadRequest("Missing device_type_id".into()))?;
    let file_data = file_data.ok_or_else(|| AppError::BadRequest("Missing file".into()))?;

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

    let sha256_for_response = sha256_hex.clone();
    let filename_for_response = filename.clone();

    let response = run_db(&state.db_pool, move |conn| {
        // Verify device type exists
        let dt = device_type_service::find_by_id(conn, device_type_id)?;

        // Auto-generate version if not provided
        let version = match version {
            Some(v) => v,
            None => firmware_service::next_version_for_type(conn, device_type_id)?,
        };

        // Insert firmware update (with placeholder URL) and blob via service
        let new_fw = NewFirmwareUpdate {
            device_type_id,
            version: version.clone(),
            url: String::new(),
            sha256: Some(sha256_hex),
            description,
            commit_sha: None,
            branch: None,
            ci_run_url: None,
            build_timestamp: None,
            changelog: None,
            source: None,
        };

        let blob = NewFirmwareBlob {
            firmware_update_id: 0, // overwritten inside upload_firmware
            data: file_data,
            size: file_size,
            filename,
        };

        let updated = firmware_service::upload_firmware(conn, &new_fw, blob).map_err(
            map_unique_violation("Firmware version already exists for this device type"),
        )?;

        Ok(FirmwareUpdateResponse {
            id: updated.id,
            device_type_id: updated.device_type_id,
            device_type_name: dt.name,
            version: updated.version,
            url: updated.url,
            sha256: updated.sha256,
            description: updated.description,
            created_at: updated.created_at.to_string(),
            has_blob: true,
            file_size: Some(file_size),
            filename: Some(filename_for_response),
            commit_sha: updated.commit_sha,
            branch: updated.branch,
            ci_run_url: updated.ci_run_url,
            build_timestamp: updated
                .build_timestamp
                .map(|ts| ts.format("%Y-%m-%dT%H:%M:%S").to_string()),
            changelog: updated.changelog,
            source: updated.source,
        })
    })
    .await?;

    // Override sha256 from the computed value (may differ from DB if DB stored None)
    let mut response = response;
    response.sha256 = Some(sha256_for_response);

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
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Response, AppError> {
    let blob = run_db(&state.db_pool, move |conn| {
        firmware_service::download_blob(conn, id)
    })
    .await?;

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
        .body(Body::from(blob.data))
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
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    run_db(&state.db_pool, move |conn| {
        firmware_service::delete(conn, id)
    })
    .await?;

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
    Path(device_type_id): Path<i32>,
) -> Result<Json<NextVersionResponse>, AppError> {
    let version = run_db(&state.db_pool, move |conn| {
        firmware_service::next_version_for_type(conn, device_type_id)
    })
    .await?;

    Ok(Json(NextVersionResponse {
        next_version: version,
    }))
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
