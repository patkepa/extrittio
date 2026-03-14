use prost::Message;
use tracing::{info, warn};

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

    #[allow(clippy::cast_sign_loss)]
    if let Err(e) = device_service::update_from_heartbeat(
        &mut conn,
        &heartbeat_msg.device_id,
        &heartbeat_msg.status,
        &heartbeat_msg.firmware,
        heartbeat_msg.uptime_seconds as u64,
    ) {
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
