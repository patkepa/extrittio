// Firmware service — business logic for firmware updates

use diesel::Connection;
use diesel::SqliteConnection;

use crate::db::models::{FirmwareBlob, FirmwareUpdate, NewFirmwareBlob, NewFirmwareUpdate};
use crate::error::AppError;
use crate::repositories::firmware_repo;

/// Register a firmware update with a URL (no file upload).
pub fn register_firmware(
    conn: &mut SqliteConnection,
    new_fw: &NewFirmwareUpdate,
) -> Result<FirmwareUpdate, AppError> {
    let fw = firmware_repo::insert_firmware_update(conn, new_fw)?;
    Ok(fw)
}

/// Upload a firmware file: create metadata, store the blob, and update the URL
/// to point to the download endpoint. All writes are atomic.
pub fn upload_firmware(
    conn: &mut SqliteConnection,
    new_fw: &NewFirmwareUpdate,
    blob: NewFirmwareBlob,
) -> Result<FirmwareUpdate, AppError> {
    conn.transaction(|conn| {
        let fw = firmware_repo::insert_firmware_update(conn, new_fw)?;

        let blob_with_id = NewFirmwareBlob {
            firmware_update_id: fw.id,
            ..blob
        };
        firmware_repo::insert_firmware_blob(conn, &blob_with_id)?;

        // Update URL to point to download endpoint
        let url = format!("/api/v1/firmware-updates/{}/download", fw.id);
        firmware_repo::update_firmware_url(conn, fw.id, &url)?;

        // Re-read to get updated URL
        let updated = firmware_repo::find_firmware_update(conn, fw.id)?;
        Ok(updated)
    })
}

/// Generate the next semantic version for a device type.
pub fn next_version_for_type(
    conn: &mut SqliteConnection,
    device_type_id: i32,
) -> Result<String, AppError> {
    let latest = firmware_repo::find_next_version(conn, device_type_id)?;
    Ok(match latest {
        Some(v) => increment_version(&v),
        None => "1.0.0".to_string(),
    })
}

/// Increment the patch component of a semver string.
pub fn increment_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 3 {
        if let Ok(patch) = parts[2].parse::<u32>() {
            return format!("{}.{}.{}", parts[0], parts[1], patch + 1);
        }
    }
    format!("{version}.1")
}

/// List firmware updates with pagination and optional type filter.
pub fn list(
    conn: &mut SqliteConnection,
    device_type_id: Option<i32>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<firmware_repo::FirmwareUpdateRow>, i64), AppError> {
    Ok(firmware_repo::list_firmware_updates(conn, device_type_id, limit, offset)?)
}

/// Delete a firmware update by ID.
pub fn delete(conn: &mut SqliteConnection, id: i32) -> Result<(), AppError> {
    let deleted = firmware_repo::delete_firmware_update(conn, id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Firmware update {id} not found")));
    }
    Ok(())
}

/// Download a firmware blob by firmware update ID.
pub fn download_blob(
    conn: &mut SqliteConnection,
    firmware_update_id: i32,
) -> Result<FirmwareBlob, AppError> {
    Ok(firmware_repo::find_firmware_blob(conn, firmware_update_id)?)
}
