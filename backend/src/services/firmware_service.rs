// Firmware service — business logic for firmware updates

use diesel::Connection;
use diesel::SqliteConnection;

use crate::db::models::{FirmwareUpdate, NewFirmwareBlob, NewFirmwareUpdate};
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
