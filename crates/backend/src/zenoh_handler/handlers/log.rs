use prost::Message;
use tracing::{info, warn};

use crate::services::log_service;
use crate::state::DbPool;

use extrittio_common::extrittio::DeviceLog;

/// Decode a `DeviceLog` protobuf message and insert it into the database.
///
/// Logs and drops messages from unregistered devices or malformed payloads.
pub fn handle_device_log(db_pool: &DbPool, topic_device_id: &str, payload: &[u8]) {
    let log_msg = match DeviceLog::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceLog: {}", e);
            return;
        }
    };
    if !super::validate_topic_device("device log", topic_device_id, &log_msg.device_id) {
        return;
    }

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
        }
    };
    let Some(identity) =
        super::resolve_ingress_identity(&mut conn, "device log", &log_msg.device_id)
    else {
        return;
    };

    match log_service::record(&mut conn, &identity, &log_msg.level, &log_msg.message) {
        Ok(false) => {
            warn!(
                "Dropping log from unregistered device: {}",
                log_msg.device_id
            );
        }
        Ok(true) => {
            info!(
                "Log from device {}: [{}] {}",
                log_msg.device_id, log_msg.level, log_msg.message
            );
        }
        Err(e) => {
            warn!("Failed to record device log: {}", e);
        }
    }
}
