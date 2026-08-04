pub mod command_response;
pub mod heartbeat;
pub mod log;
pub mod shadow;
pub mod telemetry;

use diesel::PgConnection;

use crate::repositories::device_repo;
use crate::tenancy::DeviceIdentity;

pub(crate) fn resolve_ingress_identity(
    conn: &mut PgConnection,
    message_type: &str,
    device_id: &str,
) -> Option<DeviceIdentity> {
    match device_repo::resolve_device_identity(conn, device_id) {
        Ok(identity) => Some(identity),
        Err(diesel::result::Error::NotFound) => {
            tracing::warn!("Dropping {message_type} from unregistered device: {device_id}");
            None
        }
        Err(error) => {
            tracing::warn!("Failed to resolve device identity for {message_type}: {error}");
            None
        }
    }
}

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
