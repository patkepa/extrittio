pub mod command_response;
pub mod heartbeat;
pub mod log;
pub mod shadow;
pub mod telemetry;

pub(crate) fn validate_topic_device(
    message_type: &str,
    topic_device_id: &str,
    payload_device_id: &str,
) -> bool {
    if topic_device_id == payload_device_id {
        return true;
    }

    tracing::warn!(
        "{message_type} device identity mismatch: topic device_id={topic_device_id}, payload device_id={payload_device_id}; dropping message"
    );
    false
}
