use prost::Message;
use tracing::{info, warn};

use crate::services::command_service;
use crate::state::DbPool;

use extrittio_common::extrittio::DeviceCommandResponse;

/// Decode a `DeviceCommandResponse` protobuf message and update the corresponding
/// command record's status and response payload.
pub fn handle_command_response(db_pool: &DbPool, payload: &[u8]) {
    let response = match DeviceCommandResponse::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceCommandResponse: {}", e);
            return;
        }
    };

    if response.correlation_id.is_empty() {
        warn!("Received command response with empty correlation_id");
        return;
    }

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
        }
    };

    let response_payload = if response.payload.is_empty() {
        None
    } else {
        Some(response.payload.as_str())
    };

    match command_service::handle_response(
        &mut conn,
        &response.correlation_id,
        &response.device_id,
        &response.status,
        response_payload,
    ) {
        Ok(Some(new_status)) => {
            info!(
                "Command {} for device {} -> {}",
                response.correlation_id, response.device_id, new_status
            );
        }
        Ok(None) => {
            info!(
                "Command {} for device {} — no update (terminal or mismatch)",
                response.correlation_id, response.device_id
            );
        }
        Err(e) => {
            warn!("Failed to handle command response: {}", e);
        }
    }
}
