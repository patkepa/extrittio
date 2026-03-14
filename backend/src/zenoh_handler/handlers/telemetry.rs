use prost::Message;
use tracing::{info, warn};

use crate::services::telemetry_service;
use crate::state::DbPool;

use extrittio_common::extrittio::DeviceTelemetry;

/// Decode a `DeviceTelemetry` protobuf message, insert a telemetry record, and
/// update the device's `last_seen` timestamp.
///
/// Logs and drops messages from unregistered devices or malformed payloads.
pub fn handle_telemetry(db_pool: &DbPool, payload: &[u8]) {
    let telemetry_msg = match DeviceTelemetry::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceTelemetry: {}", e);
            return;
        }
    };

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
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
        Ok(false) => {
            warn!(
                "Dropping telemetry from unregistered device: {}",
                telemetry_msg.device_id
            );
        }
        Ok(true) => {
            info!(
                "Recorded telemetry from device {}: temp={}, humidity={}, battery={}",
                telemetry_msg.device_id,
                telemetry_msg.temperature,
                telemetry_msg.humidity,
                telemetry_msg.battery_level
            );
        }
        Err(e) => {
            warn!("Failed to record telemetry: {}", e);
        }
    }
}
