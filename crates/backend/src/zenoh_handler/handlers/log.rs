use prost::Message;
use tracing::{info, warn};

use crate::persistence::RepositorySet;
use crate::services::log_service;
use crate::tenancy::DeviceIdentity;

use extrittio_common::extrittio::DeviceLog;

/// Decode a `DeviceLog` protobuf message and insert it into the database.
///
/// Logs and drops messages from unregistered devices or malformed payloads.
pub async fn handle_device_log(
    persistence: &RepositorySet,
    identity: &DeviceIdentity,
    topic_device_id: &str,
    payload: &[u8],
) {
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

    match log_service::record_with_repository(
        persistence.logs.as_ref(),
        identity,
        &log_msg.level,
        &log_msg.message,
    )
    .await
    {
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
