use chrono::NaiveDateTime;
use diesel::PgConnection;

use crate::db::models::NewFirmwareUpdate;
use crate::error::AppError;
use crate::repositories::{api_key_repo, device_type_repo, firmware_repo};

/// Parameters for CI firmware ingestion (decoupled from API request type).
pub struct CiIngestParams {
    pub device_type_name: String,
    pub version: String,
    pub artifact_url: String,
    pub sha256: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<NaiveDateTime>,
    pub description: Option<String>,
    pub changelog: Option<String>,
}

/// Ingest a firmware update from CI pipeline.
///
/// Validates the API key, resolves device type by name, checks scope,
/// and inserts the firmware update. Returns (firmware_id, version, device_type_name).
pub fn ingest(
    conn: &mut PgConnection,
    key_hash: &str,
    params: CiIngestParams,
) -> Result<(i32, String, String), AppError> {
    // Look up API key
    let api_key = api_key_repo::find_api_key_by_hash(conn, key_hash)?
        .ok_or(AppError::Unauthorized)?;

    // Update last_used_at
    let _ = api_key_repo::update_last_used(conn, api_key.id);

    // Resolve device type by name
    let device_type =
        device_type_repo::find_device_type_by_name(conn, &params.device_type_name)?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Device type '{}' not found",
                    params.device_type_name
                ))
            })?;

    // Check scope
    if let Some(scoped_id) = api_key.device_type_id {
        if scoped_id != device_type.id {
            return Err(AppError::Forbidden(format!(
                "API key is scoped to device type ID {}, not '{}'",
                scoped_id, params.device_type_name
            )));
        }
    }

    // Insert firmware update
    let new_fw = NewFirmwareUpdate {
        device_type_id: device_type.id,
        version: params.version.clone(),
        url: params.artifact_url,
        description: params.description,
        sha256: params.sha256,
        commit_sha: params.commit_sha,
        branch: params.branch,
        ci_run_url: params.ci_run_url,
        build_timestamp: params.build_timestamp,
        changelog: params.changelog,
        source: Some("ci".to_string()),
    };

    let fw = firmware_repo::insert_firmware_update(conn, &new_fw).map_err(|e| {
        if let diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _,
        ) = &e
        {
            AppError::Conflict(format!(
                "Version '{}' already exists for device type '{}'",
                params.version, params.device_type_name
            ))
        } else {
            AppError::Internal(e.to_string())
        }
    })?;

    Ok((fw.id, fw.version, params.device_type_name))
}
