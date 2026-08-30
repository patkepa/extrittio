use extrittio_device_contract::{CompiledContractDocument, PayloadEncoding, RouteDirection};
use tracing::warn;

use crate::persistence::Persistence;
use crate::rule_engine::cache::RuleCache;
use crate::tenancy::DeviceIdentity;

/// Dispatch an arbitrary device address by matching it against the device's
/// materialized contract. Addresses absent from the contract are ignored.
pub async fn handle_contract_ingress(
    persistence: &Persistence,
    identity: &DeviceIdentity,
    topic: &str,
    payload: &[u8],
    rule_cache: &std::sync::RwLock<RuleCache>,
) -> bool {
    let assigned = match persistence
        .devices
        .assigned_contract(identity.tenant_id(), identity.device_id())
        .await
    {
        Ok(Some(contract)) => contract,
        Ok(None) => return false,
        Err(error) => {
            warn!(device_id = identity.device_id(), %error, "Failed to resolve contract ingress");
            return false;
        }
    };
    let contract: CompiledContractDocument = match serde_json::from_value(assigned.document) {
        Ok(contract) => contract,
        Err(error) => {
            warn!(contract_id = assigned.id, %error, "Stored device contract is invalid");
            return false;
        }
    };
    let Some((route_key, route)) = contract.routes.iter().find(|(_, route)| {
        route.direction == RouteDirection::DeviceToCloud && route.address == topic
    }) else {
        return false;
    };

    if contract
        .commands
        .values()
        .any(|command| command.response_route == *route_key)
    {
        super::command_response::handle_command_response(
            persistence,
            identity,
            identity.device_id(),
            payload,
        )
        .await;
    } else if route.encoding == PayloadEncoding::Json {
        super::event::handle_event(persistence, identity, route_key, payload, rule_cache).await;
    } else {
        warn!(
            device_id = identity.device_id(),
            route_key, "No contract ingress decoder is available for this route"
        );
    }
    true
}
