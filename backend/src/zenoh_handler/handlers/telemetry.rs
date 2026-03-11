use chrono::Utc;
use prost::Message;
use tracing::{info, warn};

use crate::db::models::{NewTelemetryRecord, UpdateDevice};
use crate::repositories::{device_repo, telemetry_repo};
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

    // Verify the device is registered
    match device_repo::device_exists(&mut conn, &telemetry_msg.device_id) {
        Ok(true) => {}
        Ok(false) => {
            warn!(
                "Dropping telemetry from unregistered device: {}",
                telemetry_msg.device_id
            );
            return;
        }
        Err(e) => {
            warn!("DB error checking device: {}", e);
            return;
        }
    }

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

    let new_record = NewTelemetryRecord {
        device_id: telemetry_msg.device_id.clone(),
        payload: payload.to_vec(),
        temperature: Some(telemetry_msg.temperature),
        humidity: Some(telemetry_msg.humidity),
        battery_level: Some(telemetry_msg.battery_level),
        custom_json,
    };

    if let Err(e) = telemetry_repo::insert_telemetry(&mut conn, &new_record) {
        warn!("Failed to insert telemetry record: {}", e);
        return;
    }

    // Update device last_seen
    let now = Utc::now().naive_utc();
    let changeset = UpdateDevice {
        last_seen: Some(now),
        updated_at: Some(now),
        ..Default::default()
    };

    if let Err(e) = device_repo::update_device(&mut conn, &telemetry_msg.device_id, &changeset) {
        warn!("Failed to update device last_seen: {}", e);
    }

    info!(
        "Recorded telemetry from device {}: temp={}, humidity={}, battery={}",
        telemetry_msg.device_id,
        telemetry_msg.temperature,
        telemetry_msg.humidity,
        telemetry_msg.battery_level
    );
}
