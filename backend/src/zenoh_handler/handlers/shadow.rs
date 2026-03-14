use prost::Message;
use std::sync::Arc;
use tracing::{info, warn};

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
    let shadow = match shadow_service::get_shadow(&mut conn, &report.device_id) {
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

    if let Err(e) = shadow_service::process_ota_from_report(&mut conn, &report.device_id, &merged_reported) {
        warn!("Failed to process OTA from shadow report: {}", e);
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

    let shadow = match tokio::task::spawn_blocking(move || {
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to get DB connection: {e}")),
        };
        shadow_service::get_shadow(&mut conn, &device_id)
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
