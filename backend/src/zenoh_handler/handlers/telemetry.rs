use prost::Message;
use tracing::{info, warn};

use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::evaluate::evaluate_telemetry;
use crate::rule_engine::types::{PendingAction, TelemetryData};
use crate::services::telemetry_service;
use crate::state::DbPool;

use extrittio_common::extrittio::DeviceTelemetry;

/// Decode a `DeviceTelemetry` protobuf message, insert a telemetry record, and
/// update the device's `last_seen` timestamp.
///
/// After a successful record, evaluates applicable rules against the telemetry
/// data and returns the resulting `PendingAction`s for the subscriber to
/// execute asynchronously.
///
/// Logs and drops messages from unregistered devices or malformed payloads.
pub fn handle_telemetry(
    db_pool: &DbPool,
    payload: &[u8],
    rule_cache: &std::sync::RwLock<RuleCache>,
) -> Vec<PendingAction> {
    let telemetry_msg = match DeviceTelemetry::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceTelemetry: {}", e);
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

    // Build custom_json from metadata map (if non-empty)
    let custom_json = if telemetry_msg.metadata.is_empty() {
        None
    } else {
        match serde_json::to_string(&telemetry_msg.metadata) {
            Ok(json) => Some(json),
            Err(e) => {
                warn!("Failed to serialize metadata: {}", e);
                None
            }
        }
    };

    match telemetry_service::record(
        &mut conn,
        &telemetry_msg.device_id,
        Some(telemetry_msg.temperature),
        Some(telemetry_msg.humidity),
        Some(telemetry_msg.battery_level),
        custom_json,
        payload.to_vec(),
    ) {
        Ok(None) => {
            warn!(
                "Dropping telemetry from unregistered device: {}",
                telemetry_msg.device_id
            );
            Vec::new()
        }
        Ok(Some(device)) => {
            info!(
                "Recorded telemetry from device {}: temp={}, humidity={}, battery={}",
                telemetry_msg.device_id,
                telemetry_msg.temperature,
                telemetry_msg.humidity,
                telemetry_msg.battery_level
            );

            // Evaluate rules against this telemetry data
            let data = TelemetryData {
                temperature: telemetry_msg.temperature,
                humidity: telemetry_msg.humidity,
                battery_level: telemetry_msg.battery_level,
            };

            let cache = match rule_cache.read() {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to read-lock rule cache: {}", e);
                    return Vec::new();
                }
            };

            evaluate_telemetry(
                &telemetry_msg.device_id,
                device.device_type_id,
                device.fleet_id,
                &data,
                &cache,
            )
        }
        Err(e) => {
            warn!("Failed to record telemetry: {}", e);
            Vec::new()
        }
    }
}
