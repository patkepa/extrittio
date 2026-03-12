use chrono::Utc;
use prost::Message;
use tracing::{info, warn};

use crate::db::models::{NewDevice, UpdateDevice};
use crate::repositories::{device_repo, device_type_repo};
use crate::state::DbPool;

use extrittio_common::extrittio::DeviceHeartbeat;

/// Infer the device type name from the firmware version string.
///
/// Returns `"mac-device"` for macOS firmware, `"default"` otherwise.
fn infer_device_type(firmware: &str) -> &'static str {
    if firmware.contains("macos") {
        "mac-device"
    } else {
        "default"
    }
}

/// Decode a `DeviceHeartbeat` protobuf message and update the device's status,
/// firmware, uptime, and `last_seen` timestamp.
///
/// Devices that send a heartbeat but are not yet registered are automatically
/// provisioned with a device type inferred from the firmware version string.
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

    // Auto-register device on first heartbeat
    match device_repo::device_exists(&mut conn, &heartbeat_msg.device_id) {
        Ok(true) => {}
        Ok(false) => {
            let type_name = infer_device_type(&heartbeat_msg.firmware);
            let device_type_id = match device_type_repo::find_device_type_by_name(&mut conn, type_name) {
                Ok(Some(dt)) => dt.id,
                Ok(None) => {
                    warn!("Device type '{}' not found, falling back to default", type_name);
                    match device_type_repo::find_default_device_type_id(&mut conn) {
                        Ok(Some(id)) => id,
                        _ => {
                            warn!("No default device type found, dropping heartbeat");
                            return;
                        }
                    }
                }
                Err(e) => {
                    warn!("DB error looking up device type: {}", e);
                    return;
                }
            };

            let new_device = NewDevice {
                id: heartbeat_msg.device_id.clone(),
                name: heartbeat_msg.device_id.clone(),
                device_type_id,
                fleet_id: None,
                location: String::new(),
                firmware: heartbeat_msg.firmware.clone(),
            };

            if let Err(e) = device_repo::insert_device(&mut conn, &new_device) {
                warn!("Failed to auto-register device {}: {}", heartbeat_msg.device_id, e);
                return;
            }

            info!(
                "Auto-registered device {} as type '{}'",
                heartbeat_msg.device_id, type_name
            );
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
