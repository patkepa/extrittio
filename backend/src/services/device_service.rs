// Device service — business logic for device management

use diesel::SqliteConnection;
use std::sync::Arc;

use crate::db::models::{NewDevice, NewDeviceShadow, NewOtaDeployment};
use crate::error::AppError;
use crate::repositories::{device_repo, firmware_repo, shadow_repo};
use crate::services::shadow_service;

/// Create a device and its associated shadow record atomically.
pub fn create_device(
    conn: &mut SqliteConnection,
    new_device: &NewDevice,
) -> Result<(), AppError> {
    device_repo::insert_device(conn, new_device)?;
    let new_shadow = NewDeviceShadow {
        device_id: new_device.id.clone(),
    };
    shadow_repo::insert_shadow(conn, &new_shadow)?;
    Ok(())
}

/// Orchestrate an OTA update: validate device/firmware compatibility, update
/// the device shadow desired state, and record the deployment.
pub async fn trigger_ota(
    conn: &mut SqliteConnection,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    firmware_update_id: i32,
) -> Result<(), AppError> {
    let device = device_repo::find_device(conn, device_id)?;
    let fw = firmware_repo::find_firmware_update(conn, firmware_update_id)?;

    if fw.device_type_id != device.device_type_id {
        return Err(AppError::BadRequest(
            "Firmware device type does not match device".into(),
        ));
    }

    // Build OTA patch and apply via shadow service
    let mut ota_payload = serde_json::json!({
        "firmware_version": fw.version,
        "firmware_url": fw.url,
        "firmware_update_id": fw.id,
    });
    if let Some(ref hash) = fw.sha256 {
        ota_payload["sha256"] = serde_json::Value::String(hash.clone());
    }

    let mut patch = serde_json::Map::new();
    patch.insert("ota".to_string(), ota_payload);

    shadow_service::update_desired(conn, zenoh_session, device_id, &patch).await?;

    let deployment = NewOtaDeployment {
        device_id: device_id.to_string(),
        firmware_update_id: fw.id,
    };
    firmware_repo::insert_ota_deployment(conn, &deployment)?;

    Ok(())
}
