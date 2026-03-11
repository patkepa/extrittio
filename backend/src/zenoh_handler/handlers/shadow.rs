use prost::Message;
use std::sync::Arc;
use tracing::{info, warn};

use crate::db::models::DeviceShadow;
use crate::repositories::{device_repo, firmware_repo, shadow_repo};
use crate::services::shadow_service;
use crate::state::DbPool;

use extrittio_common::extrittio::{ShadowGet, ShadowReport};

/// Decode a `ShadowReport` protobuf message, merge the reported state into the
/// device shadow, and update OTA deployment status if applicable.
pub fn handle_shadow_report(db_pool: &DbPool, payload: &[u8]) {
    let report = match ShadowReport::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode ShadowReport: {}", e);
            return;
        }
    };

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
        }
    };

    // Verify device exists
    match device_repo::device_exists(&mut conn, &report.device_id) {
        Ok(true) => {}
        Ok(false) => {
            warn!(
                "Dropping shadow report from unregistered device: {}",
                report.device_id
            );
            return;
        }
        Err(e) => {
            warn!("DB error checking device: {}", e);
            return;
        }
    }

    // Parse incoming reported state as a JSON patch
    let new_reported: serde_json::Value = match serde_json::from_str(&report.state_json) {
        Ok(v) => v,
        Err(e) => {
            warn!("Invalid JSON in ShadowReport: {}", e);
            return;
        }
    };

    let Some(patch) = new_reported.as_object().cloned() else {
        warn!(
            "ShadowReport state_json is not a JSON object for device {}",
            report.device_id
        );
        return;
    };

    // Use shadow_service to merge reported state, recompute delta, and persist
    if let Err(e) = shadow_service::update_reported(&mut conn, &report.device_id, &patch) {
        warn!("Failed to update shadow reported state: {}", e);
        return;
    }

    // Re-read the shadow to get the merged reported state and new version for logging & OTA
    let shadow = match shadow_repo::find_shadow(&mut conn, &report.device_id) {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "Failed to re-read shadow for device {}: {}",
                report.device_id, e
            );
            return;
        }
    };

    info!(
        "Shadow report from device {}: version={}",
        report.device_id, shadow.version
    );

    // Update OTA deployment status if reported state contains ota.status
    let merged_reported: serde_json::Value =
        serde_json::from_str(&shadow.reported).unwrap_or_default();

    use extrittio_common::ota::{fields as ota_fields, status as ota_status_consts};

    if let Some(ota_obj) = merged_reported.get(ota_fields::SHADOW_KEY).and_then(|v| v.as_object())
        && let Some(ota_status_raw) = ota_obj.get(ota_fields::STATUS).and_then(|v| v.as_str())
    {
        let ota_status = ota_status_raw.to_lowercase();
        let is_terminal = ota_status_consts::is_terminal(&ota_status);
        let now = chrono::Utc::now().naive_utc();
        let completed_at = if is_terminal { Some(now) } else { None };
        let error_message = ota_obj
            .get(ota_fields::ERROR)
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);

        let fw_update_id = ota_obj
            .get(ota_fields::FIRMWARE_UPDATE_ID)
            .and_then(serde_json::Value::as_i64)
            .and_then(|id| i32::try_from(id).ok());

        let deployment =
            firmware_repo::find_active_ota_deployment(&mut conn, &report.device_id, fw_update_id);

        if let Ok(Some(dep_id)) = deployment {
            match firmware_repo::update_ota_deployment_status(
                &mut conn,
                dep_id,
                &ota_status,
                error_message.as_deref(),
                completed_at,
            ) {
                Ok(_) => {
                    info!(
                        "OTA deployment {} for device {} -> {}",
                        dep_id, report.device_id, ota_status
                    );
                }
                Err(e) => {
                    warn!("Failed to update OTA deployment status: {}", e);
                }
            }
        }
    }
}

/// Decode a `ShadowGet` protobuf message and publish the shadow delta back to
/// the device if non-empty. DB access runs on a blocking thread.
pub async fn handle_shadow_get(db_pool: &DbPool, session: &Arc<zenoh::Session>, payload: &[u8]) {
    let get_msg = match ShadowGet::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode ShadowGet: {}", e);
            return;
        }
    };

    let device_id = get_msg.device_id.clone();
    let pool = db_pool.clone();

    let shadow: DeviceShadow = match tokio::task::spawn_blocking(move || {
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to get DB connection: {e}")),
        };
        shadow_repo::find_shadow(&mut conn, &device_id)
            .map_err(|e| format!("Shadow not found for device {device_id}: {e}"))
    })
    .await
    {
        Ok(Ok(shadow)) => shadow,
        Ok(Err(msg)) => {
            warn!("{}", msg);
            return;
        }
        Err(e) => {
            warn!("Shadow get handler task panicked: {}", e);
            return;
        }
    };

    // Only send delta if non-empty
    let delta: serde_json::Value = serde_json::from_str(&shadow.delta).unwrap_or_default();
    if delta.as_object().is_some_and(serde_json::Map::is_empty) {
        info!(
            "Shadow get from device {}: already in sync",
            get_msg.device_id
        );
        return;
    }

    shadow_service::publish_delta_if_nonempty(session, &get_msg.device_id, &delta, shadow.version)
        .await;

    info!("Shadow get from device {}: sent delta", get_msg.device_id);
}
