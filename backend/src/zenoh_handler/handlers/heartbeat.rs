use prost::Message;
use tracing::{info, warn};

use crate::repositories::device_repo;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::evaluate::evaluate_status_change_for_tenant;
use crate::rule_engine::types::PendingAction;
use crate::services::device_service;
use crate::state::DbPool;

use extrittio_common::extrittio::DeviceHeartbeat;

/// Decode a `DeviceHeartbeat` protobuf message and update the device's status,
/// firmware, uptime, and `last_seen` timestamp.
///
/// Devices that send a heartbeat but are not yet registered are automatically
/// provisioned via the device service.
///
/// When a status change is detected, evaluates applicable rules and returns
/// `PendingAction`s for the subscriber to execute asynchronously.
pub fn handle_heartbeat(
    db_pool: &DbPool,
    payload: &[u8],
    rule_cache: &std::sync::RwLock<RuleCache>,
) -> Vec<PendingAction> {
    let heartbeat_msg = match DeviceHeartbeat::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceHeartbeat: {}", e);
            return Vec::new();
        }
    };

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return Vec::new();
        }
    };

    // Auto-register device on first heartbeat (returns None on failure)
    if device_service::auto_register_device(
        &mut conn,
        &heartbeat_msg.device_id,
        &heartbeat_msg.firmware,
    )
    .is_none()
    {
        return Vec::new();
    }

    #[allow(clippy::cast_sign_loss)]
    let status_change = match device_service::update_from_heartbeat(
        &mut conn,
        &heartbeat_msg.device_id,
        &heartbeat_msg.status,
        &heartbeat_msg.firmware,
        heartbeat_msg.uptime_seconds as u64,
    ) {
        Ok(sc) => sc,
        Err(e) => {
            warn!("Failed to update device from heartbeat: {}", e);
            return Vec::new();
        }
    };

    info!(
        "Heartbeat from device {}: status={}, firmware={}, uptime={}s",
        heartbeat_msg.device_id,
        heartbeat_msg.status,
        heartbeat_msg.firmware,
        heartbeat_msg.uptime_seconds
    );

    // If status changed, evaluate rules
    if let Some(change) = status_change {
        // Load device record to get device_type_id and fleet_id
        let device = match device_repo::find_device(&mut conn, &heartbeat_msg.device_id) {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to load device for rule evaluation: {}", e);
                return Vec::new();
            }
        };

        let cache = match rule_cache.read() {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read-lock rule cache: {}", e);
                return Vec::new();
            }
        };

        evaluate_status_change_for_tenant(
            &device.tenant_id,
            &heartbeat_msg.device_id,
            device.device_type_id,
            device.fleet_id,
            &change,
            &cache,
        )
    } else {
        Vec::new()
    }
}
