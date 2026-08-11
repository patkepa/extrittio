pub mod command_response;
pub mod heartbeat;
pub mod log;
pub mod shadow;
pub mod telemetry;

use crate::persistence::Persistence;
use crate::services::device_catalog_service;
use crate::tenancy::DeviceIdentity;

pub(crate) async fn resolve_ingress_identity(
    persistence: &Persistence,
    message_type: &str,
    device_id: &str,
    warn_if_missing: bool,
) -> Option<DeviceIdentity> {
    match device_catalog_service::resolve_identity(persistence.devices.as_ref(), device_id).await {
        Ok(Some(identity)) => Some(identity),
        Ok(None) => {
            if warn_if_missing {
                tracing::warn!("Dropping {message_type} from unregistered device: {device_id}");
            }
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
