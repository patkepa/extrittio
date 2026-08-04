use diesel::Connection;
use prost::Message;
use tracing::{info, warn};

use crate::error::AppError;
use crate::repositories::device_repo;
use crate::rule_engine::actions::enqueue_pending_actions;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::evaluate::evaluate_status_change_for_tenant;
use crate::services::device_service;
use crate::state::DbPool;
use crate::tenancy::DEFAULT_TENANT_ID;

use extrittio_common::extrittio::DeviceHeartbeat;

/// Decode a `DeviceHeartbeat` protobuf message and update the device's status,
/// firmware, uptime, and `last_seen` timestamp.
///
/// Devices that send a heartbeat but are not yet registered are automatically
/// provisioned via the device service.
///
/// A status transition and its resulting durable rule actions are committed in
/// one transaction.
pub fn handle_heartbeat(
    db_pool: &DbPool,
    topic_device_id: &str,
    payload: &[u8],
    rule_cache: &std::sync::RwLock<RuleCache>,
    allow_auto_register: bool,
) -> usize {
    let heartbeat_msg = match DeviceHeartbeat::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceHeartbeat: {}", e);
            return 0;
        }
    };
    if !super::validate_topic_device("heartbeat", topic_device_id, &heartbeat_msg.device_id) {
        return 0;
    }

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return 0;
        }
    };

    let identity = match device_repo::resolve_device_identity(&mut conn, &heartbeat_msg.device_id) {
        Ok(identity) => identity,
        Err(diesel::result::Error::NotFound) if allow_auto_register => {
            // Unprovisioned devices are intentionally assigned to the default
            // tenant. Once registered, all ingestion uses the resolved identity.
            if device_service::auto_register_device(
                &mut conn,
                DEFAULT_TENANT_ID,
                &heartbeat_msg.device_id,
                &heartbeat_msg.firmware,
            )
            .is_none()
            {
                return 0;
            }
            match device_repo::resolve_device_identity(&mut conn, &heartbeat_msg.device_id) {
                Ok(identity) => identity,
                Err(error) => {
                    warn!("Failed to resolve auto-registered device identity: {error}");
                    return 0;
                }
            }
        }
        Err(diesel::result::Error::NotFound) => {
            warn!(
                "Dropping heartbeat from unregistered device: {}",
                heartbeat_msg.device_id
            );
            return 0;
        }
        Err(error) => {
            warn!("Failed to resolve heartbeat device identity: {error}");
            return 0;
        }
    };

    let result = conn.transaction::<usize, AppError, _>(|conn| {
        #[allow(clippy::cast_sign_loss)]
        let status_change = device_service::update_from_heartbeat(
            conn,
            &identity,
            &heartbeat_msg.status,
            &heartbeat_msg.firmware,
            heartbeat_msg.uptime_seconds as u64,
        )?;

        info!(
            "Heartbeat from device {}: status={}, firmware={}, uptime={}s",
            heartbeat_msg.device_id,
            heartbeat_msg.status,
            heartbeat_msg.firmware,
            heartbeat_msg.uptime_seconds
        );

        let Some(change) = status_change else {
            return Ok(0);
        };
        let device = device_repo::find_device_for_tenant(
            conn,
            identity.tenant_id_str(),
            identity.device_id(),
        )?;
        let cache = rule_cache.read().map_err(|error| {
            AppError::Internal(format!("failed to read-lock rule cache: {error}"))
        })?;
        let actions = evaluate_status_change_for_tenant(
            &device.tenant_id,
            &heartbeat_msg.device_id,
            device.device_type_id,
            device.fleet_id,
            &change,
            &cache,
        );
        enqueue_pending_actions(conn, &actions)
    });

    match result {
        Ok(enqueued) => enqueued,
        Err(error) => {
            warn!("Failed to atomically record heartbeat and rule actions: {error}");
            0
        }
    }
}
