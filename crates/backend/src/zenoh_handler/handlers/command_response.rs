use prost::Message;
use tracing::{info, warn};

use crate::persistence::Persistence;
use crate::services::command_service;
use crate::tenancy::DeviceIdentity;

use extrittio_common::extrittio::DeviceCommandResponse;

/// Decode a `DeviceCommandResponse` protobuf message and update the corresponding
/// command record's status and response payload.
pub async fn handle_command_response(
    persistence: &Persistence,
    identity: &DeviceIdentity,
    topic_device_id: &str,
    payload: &[u8],
) {
    let response = match DeviceCommandResponse::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceCommandResponse: {}", e);
            return;
        }
    };
    if !super::validate_topic_device("command response", topic_device_id, &response.device_id) {
        return;
    }

    if response.correlation_id.is_empty() {
        warn!("Received command response with empty correlation_id");
        return;
    }

    let response_payload = if response.payload.is_empty() {
        None
    } else {
        Some(response.payload.as_str())
    };

    match command_service::handle_response_with_repository(
        persistence.commands.as_ref(),
        identity,
        &response.correlation_id,
        &response.status,
        response_payload,
    )
    .await
    {
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
