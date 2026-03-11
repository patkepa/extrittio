// Device service — business logic for device management

use diesel::Connection;
use diesel::SqliteConnection;
use std::sync::Arc;

use crate::db::models::{NewDevice, NewDeviceShadow, NewOtaDeployment};
use crate::error::AppError;
use crate::repositories::{cert_repo, device_repo, firmware_repo, shadow_repo};
use crate::services::{cert_service, shadow_service};
use crate::state::{DbPool, run_db};

/// Create a device and its associated shadow record atomically.
pub fn create_device(conn: &mut SqliteConnection, new_device: &NewDevice) -> Result<(), AppError> {
    conn.transaction(|conn| {
        device_repo::insert_device(conn, new_device)?;
        let new_shadow = NewDeviceShadow {
            device_id: new_device.id.clone(),
        };
        shadow_repo::insert_shadow(conn, &new_shadow)?;

        // Generate device certificate if CA exists
        if let Some(ca) = cert_repo::get_ca_certificate(conn)? {
            let new_cert = cert_service::generate_device_certificate(&new_device.id, &ca)?;
            cert_repo::insert_device_certificate(conn, &new_cert)?;
        }

        Ok(())
    })
}

/// Orchestrate an OTA update: validate device/firmware compatibility, update
/// the device shadow desired state, and record the deployment.
///
/// All DB operations run in a single transaction. The Zenoh delta publish
/// happens after the transaction commits so the connection is not held across
/// the async boundary.
pub async fn trigger_ota(
    pool: &DbPool,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    firmware_update_id: i32,
) -> Result<(), AppError> {
    let d_id = device_id.to_string();
    let d_id_for_publish = d_id.clone();

    let (delta, version) = run_db(pool, move |conn| {
        conn.transaction(|conn| {
            let device = device_repo::find_device(conn, &d_id)?;
            let fw = firmware_repo::find_firmware_update(conn, firmware_update_id)?;

            if fw.device_type_id != device.device_type_id {
                return Err(AppError::BadRequest(
                    "Firmware device type does not match device".into(),
                ));
            }

            // Build OTA patch and apply via shadow service (DB-only)
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

            let (delta, version) = shadow_service::update_desired_db(conn, &d_id, &patch)?;

            let deployment = NewOtaDeployment {
                device_id: d_id,
                firmware_update_id: fw.id,
            };
            firmware_repo::insert_ota_deployment(conn, &deployment)?;

            Ok((delta, version))
        })
    })
    .await?;

    shadow_service::publish_delta_if_nonempty(zenoh_session, &d_id_for_publish, &delta, version)
        .await;

    Ok(())
}
