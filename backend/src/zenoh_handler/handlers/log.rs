use prost::Message;
use tracing::{info, warn};

use crate::db::models::NewDeviceLog;
use crate::repositories::{device_repo, log_repo};
use crate::state::DbPool;

use extrittio_proto::extrittio::DeviceLog;

/// Decode a `DeviceLog` protobuf message and insert it into the database.
///
/// Logs and drops messages from unregistered devices or malformed payloads.
pub fn handle_device_log(db_pool: &DbPool, payload: &[u8]) {
    let log_msg = match DeviceLog::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceLog: {}", e);
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
    match device_repo::device_exists(&mut conn, &log_msg.device_id) {
        Ok(true) => {}
        Ok(false) => {
            warn!(
                "Dropping log from unregistered device: {}",
                log_msg.device_id
            );
            return;
        }
        Err(e) => {
            warn!("DB error checking device: {}", e);
            return;
        }
    }

    let valid_levels = ["DEBUG", "INFO", "WARN", "ERROR"];
    let level = log_msg.level.to_uppercase();
    let level = if valid_levels.contains(&level.as_str()) {
        level
    } else {
        "INFO".to_string()
    };

    let new_log = NewDeviceLog {
        device_id: log_msg.device_id.clone(),
        level,
        message: log_msg.message.clone(),
    };

    if let Err(e) = log_repo::insert_log(&mut conn, &new_log) {
        warn!("Failed to insert device log: {}", e);
        return;
    }

    info!(
        "Log from device {}: [{}] {}",
        log_msg.device_id, new_log.level, log_msg.message
    );
}
