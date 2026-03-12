use chrono::Utc;
use prost::Message;
use tracing::{info, warn};

use crate::db::models::{NewDeviceLog, UpdateDevice};
use crate::repositories::{device_repo, log_repo};
use crate::services::device_service;
use crate::state::DbPool;

use extrittio_common::extrittio::DeviceHeartbeat;

/// Decode a `DeviceHeartbeat` protobuf message and update the device's status,
/// firmware, uptime, and `last_seen` timestamp.
///
/// Devices that send a heartbeat but are not yet registered are automatically
/// provisioned via the device service.
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

    // Auto-register device on first heartbeat (returns None on failure)
    if device_service::auto_register_device(
        &mut conn,
        &heartbeat_msg.device_id,
        &heartbeat_msg.firmware,
    )
    .is_none()
    {
        return;
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

    // Check current status to detect transitions
    let previous_status = device_repo::find_device(&mut conn, &heartbeat_msg.device_id)
        .ok()
        .map(|d| d.status);

    let now = Utc::now().naive_utc();
    let changeset = UpdateDevice {
        status: Some(status.clone()),
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

    // Log status transitions
    if let Some(prev) = &previous_status {
        if prev != &status {
            let message = format!("Device status changed from {prev} to {status}");
            let _ = log_repo::insert_log(
                &mut conn,
                &NewDeviceLog {
                    device_id: heartbeat_msg.device_id.clone(),
                    level: "INFO".to_string(),
                    message,
                },
            );
        }
    }

    info!(
        "Heartbeat from device {}: status={}, firmware={}, uptime={}s",
        heartbeat_msg.device_id,
        heartbeat_msg.status,
        heartbeat_msg.firmware,
        heartbeat_msg.uptime_seconds
    );
}
