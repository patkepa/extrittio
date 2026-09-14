use extrittio_backend_core::application::{ContractIngressApplication, ContractIngressRoute};
use tracing::warn;

use crate::persistence::RepositorySet;
use crate::tenancy::DeviceIdentity;

/// Dispatch an arbitrary device address by matching it against the device's
/// materialized contract. Addresses absent from the contract are ignored.
pub async fn handle_contract_ingress(
    persistence: &RepositorySet,
    identity: &DeviceIdentity,
    topic: &str,
    payload: &[u8],
    rule_cache: &crate::rule_snapshots::RuleSnapshotStore,
) -> bool {
    let route = match ContractIngressApplication::new(persistence.devices.clone())
        .resolve(identity, topic)
        .await
    {
        Ok(Some(route)) => route,
        Ok(None) => return false,
        Err(error) => {
            warn!(device_id = identity.device_id(), %error, "Failed to resolve contract ingress");
            return false;
        }
    };
    match route {
        ContractIngressRoute::CommandResponse => {
            super::command_response::handle_command_response(
                persistence,
                identity,
                identity.device_id(),
                payload,
            )
            .await;
        }
        ContractIngressRoute::JsonEvent { route_key } => {
            super::event::handle_event(persistence, identity, &route_key, payload, rule_cache)
                .await;
        }
        ContractIngressRoute::Unsupported { route_key } => {
            warn!(
                device_id = identity.device_id(),
                route_key, "No contract ingress decoder is available for this route"
            );
        }
    }
    true
}
