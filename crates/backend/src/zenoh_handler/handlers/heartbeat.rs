use prost::Message;
use tracing::{info, warn};

use crate::persistence::Persistence;
use crate::rule_engine::cache::RuleCache;
use crate::services::device_ingress_service;
use crate::tenancy::DeviceIdentity;

use extrittio_common::extrittio::DeviceHeartbeat;

/// Decode a heartbeat and commit device state, transition log, and resulting
/// rule actions through one backend-neutral write set.
pub async fn handle_heartbeat(
    persistence: &Persistence,
    resolved_identity: Option<DeviceIdentity>,
    topic_device_id: &str,
    payload: &[u8],
    rule_cache: &std::sync::RwLock<RuleCache>,
    allow_auto_register: bool,
) -> usize {
    let heartbeat = match DeviceHeartbeat::decode(payload) {
        Ok(message) => message,
        Err(error) => {
            warn!("Failed to decode DeviceHeartbeat: {error}");
            return 0;
        }
    };
    if !super::validate_topic_device("heartbeat", topic_device_id, &heartbeat.device_id) {
        return 0;
    }

    let identity = match resolved_identity {
        Some(identity) => identity,
        None if allow_auto_register => {
            match device_ingress_service::auto_register(
                persistence.devices.as_ref(),
                &heartbeat.device_id,
                &heartbeat.firmware,
            )
            .await
            {
                Ok(Some(context)) => context.identity,
                Ok(None) => {
                    warn!(
                        "Unable to auto-register heartbeat device {}: no compatible device type",
                        heartbeat.device_id
                    );
                    return 0;
                }
                Err(error) => {
                    warn!("Failed to auto-register heartbeat device: {error}");
                    return 0;
                }
            }
        }
        None => {
            warn!(
                "Dropping heartbeat from unregistered device: {}",
                heartbeat.device_id
            );
            return 0;
        }
    };

    match device_ingress_service::apply_heartbeat(
        persistence.devices.as_ref(),
        identity,
        &heartbeat.status,
        &heartbeat.firmware,
        heartbeat.uptime_seconds,
        rule_cache,
    )
    .await
    {
        Ok(enqueued) => {
            info!(
                "Heartbeat from device {}: status={}, firmware={}, uptime={}s",
                heartbeat.device_id, heartbeat.status, heartbeat.firmware, heartbeat.uptime_seconds
            );
            enqueued
        }
        Err(error) => {
            warn!("Failed to atomically record heartbeat and rule actions: {error}");
            0
        }
    }
}
