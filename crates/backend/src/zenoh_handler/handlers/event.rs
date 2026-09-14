use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use tracing::{info, warn};

use crate::tenancy::DeviceIdentity;
use extrittio_backend_core::EventIngressApplication;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceEventEnvelope {
    api_version: u32,
    event_id: String,
    contract_hash: String,
    occurred_at: DateTime<Utc>,
    payload: Value,
}

/// Validate a universal JSON event against the assigned materialized contract,
/// extract declared typed metrics, and persist both atomically.
pub(crate) async fn handle_event(
    application: &EventIngressApplication,
    identity: &DeviceIdentity,
    route_key: &str,
    bytes: &[u8],
    rule_cache: &crate::rule_snapshots::RuleSnapshotStore,
) -> usize {
    let envelope: DeviceEventEnvelope = match serde_json::from_slice(bytes) {
        Ok(envelope) => envelope,
        Err(error) => {
            warn!(device_id = identity.device_id(), route_key, %error, "Invalid event envelope");
            return 0;
        }
    };

    let snapshot = match rule_cache.snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            warn!(device_id = identity.device_id(), %error, "Failed to obtain rule snapshot");
            return 0;
        }
    };
    match application
        .record(
            identity.tenant_id(),
            identity.device_id(),
            route_key,
            extrittio_backend_core::events::EventInput {
                api_version: envelope.api_version,
                event_id: envelope.event_id,
                contract_hash: envelope.contract_hash,
                occurred_at: envelope.occurred_at,
                payload: envelope.payload,
                encoded_size: bytes.len() as u64,
            },
            snapshot,
        )
        .await
    {
        Ok(outcome) if outcome.recorded => {
            info!(
                device_id = identity.device_id(),
                route_key,
                metric_count = outcome.metrics_recorded,
                "Recorded contract event"
            );
            outcome.metrics_recorded
        }
        Ok(_) => 0,
        Err(error) => {
            warn!(device_id = identity.device_id(), route_key, %error, "Failed to record contract event");
            0
        }
    }
}
