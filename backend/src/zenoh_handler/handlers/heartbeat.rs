use chrono::Utc;
use prost::Message;
use tracing::{info, warn};

use crate::db::models::UpdateDevice;
use crate::repositories::device_repo;
use crate::state::DbPool;

use extrittio_common::extrittio::DeviceHeartbeat;

/// Decode a `DeviceHeartbeat` protobuf message and update the device's status,
/// firmware, uptime, and `last_seen` timestamp.
///
/// Logs and drops messages from unregistered devices or malformed payloads.
pub fn handle_heartbeat(db_pool: &DbPool, payload: &[u8]) {
    let heartbeat_msg = match DeviceHeartbeat::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceHeartbeat: {}", e);
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

    // Verify the device is registered
    match device_repo::device_exists(&mut conn, &heartbeat_msg.device_id) {
        Ok(true) => {}
        Ok(false) => {
            warn!(
                "Dropping heartbeat from unregistered device: {}",
                heartbeat_msg.device_id
            );
            return;
        }
        Err(e) => {
            warn!("DB error checking device: {}", e);
            return;
        }
    }

    use extrittio_common::device_status;

    let status = if device_status::is_valid(&heartbeat_msg.status) {
        heartbeat_msg.status.clone()
    } else {
        warn!(
            "Invalid status '{}' from device {}, defaulting to '{}'",
            heartbeat_msg.status, heartbeat_msg.device_id, device_status::ONLINE
        );
        device_status::ONLINE.to_string()
    };

    let now = Utc::now().naive_utc();
    let changeset = UpdateDevice {
        status: Some(status),
        firmware: Some(heartbeat_msg.firmware.clone()),
        #[allow(clippy::cast_possible_truncation)]
        uptime_seconds: Some(heartbeat_msg.uptime_seconds as i32),
        last_seen: Some(now),
        updated_at: Some(now),
        ..Default::default()
    };

    if let Err(e) = device_repo::update_device(&mut conn, &heartbeat_msg.device_id, &changeset) {
        warn!("Failed to update device from heartbeat: {}", e);
        return;
    }

    info!(
        "Heartbeat from device {}: status={}, firmware={}, uptime={}s",
        heartbeat_msg.device_id,
        heartbeat_msg.status,
        heartbeat_msg.firmware,
        heartbeat_msg.uptime_seconds
    );
}
