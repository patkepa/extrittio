use diesel::prelude::*;
use prost::Message;
use std::sync::Arc;
use tracing::{info, warn};

use crate::db::schema::ota_deployments;
use crate::repositories::{device_repo, shadow_repo};
use crate::services::shadow_service;
use crate::state::DbPool;

use extrittio_proto::extrittio::{ShadowGet, ShadowReport};

/// Decode a `ShadowReport` protobuf message, merge the reported state into the
/// device shadow, and update OTA deployment status if applicable.
#[allow(clippy::too_many_lines)]
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

    let patch = match new_reported.as_object() {
        Some(obj) => obj.clone(),
        None => {
            warn!(
                "ShadowReport state_json is not a JSON object for device {}",
                report.device_id
            );
            return;
        }
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

    if let Some(ota_obj) = merged_reported.get("ota").and_then(|v| v.as_object())
        && let Some(ota_status_raw) = ota_obj.get("status").and_then(|v| v.as_str())
    {
        // Normalize status to lowercase to avoid case-sensitivity mismatches
        let ota_status = ota_status_raw.to_lowercase();
        let is_terminal = ota_status == "success" || ota_status == "failed";
        let now = chrono::Utc::now().naive_utc();
        let completed_at = if is_terminal { Some(now) } else { None };
        let error_message = ota_obj
            .get("error")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);

        // Match deployment by firmware_update_id when available for precise targeting,
        // fall back to latest non-terminal deployment otherwise
        let fw_update_id = ota_obj
            .get("firmware_update_id")
            .and_then(serde_json::Value::as_i64)
            .and_then(|id| i32::try_from(id).ok());

        let deployment = if let Some(fwid) = fw_update_id {
            ota_deployments::table
                .filter(ota_deployments::device_id.eq(&report.device_id))
                .filter(ota_deployments::firmware_update_id.eq(fwid))
                .filter(ota_deployments::status.ne("success"))
                .filter(ota_deployments::status.ne("failed"))
                .order(ota_deployments::initiated_at.desc())
                .select(ota_deployments::id)
                .first::<i32>(&mut conn)
                .optional()
        } else {
            ota_deployments::table
                .filter(ota_deployments::device_id.eq(&report.device_id))
                .filter(ota_deployments::status.ne("success"))
                .filter(ota_deployments::status.ne("failed"))
                .order(ota_deployments::initiated_at.desc())
                .select(ota_deployments::id)
                .first::<i32>(&mut conn)
                .optional()
        };

        if let Ok(Some(dep_id)) = deployment {
            if is_terminal {
                if let Err(e) = diesel::update(ota_deployments::table.find(dep_id))
                    .set((
                        ota_deployments::status.eq(&ota_status),
                        ota_deployments::error_message.eq(error_message),
                        ota_deployments::completed_at.eq(completed_at),
                    ))
                    .execute(&mut conn)
                {
                    warn!("Failed to update OTA deployment status: {}", e);
                } else {
                    info!(
                        "OTA deployment {} for device {} -> {}",
                        dep_id, report.device_id, ota_status
                    );
                }
            } else if let Err(e) = diesel::update(ota_deployments::table.find(dep_id))
                .set(ota_deployments::status.eq(&ota_status))
                .execute(&mut conn)
            {
                warn!("Failed to update OTA deployment status: {}", e);
            }
        }
    }
}

/// Decode a `ShadowGet` protobuf message and publish the shadow delta back to
/// the device if non-empty.
pub async fn handle_shadow_get(
    db_pool: &DbPool,
    session: &Arc<zenoh::Session>,
    payload: &[u8],
) {
    let get_msg = match ShadowGet::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode ShadowGet: {}", e);
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

    let shadow = match shadow_repo::find_shadow(&mut conn, &get_msg.device_id) {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "Shadow not found for device {}: {}",
                get_msg.device_id, e
            );
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
