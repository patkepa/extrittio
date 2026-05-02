use diesel::prelude::*;
use prost::Message;
use tracing::{info, warn};

use crate::db::models::NewTelemetryRecord;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::evaluate::{evaluate_geofence_for_tenant, evaluate_telemetry_for_tenant};
use crate::rule_engine::types::{PendingAction, TelemetryData};
use crate::services::{device_connections, telemetry_service};
use crate::state::DbPool;
use crate::tenancy::DEFAULT_TENANT_ID;

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
        match serde_json::to_value(&telemetry_msg.metadata) {
            Ok(val) => Some(val),
            Err(e) => {
                warn!("Failed to serialize metadata: {}", e);
                None
            }
        }
    };

    let has_location = telemetry_msg.latitude != 0.0 || telemetry_msg.longitude != 0.0;

    let observed_network_hosts =
        device_connections::network_analyzer_hosts_from_metadata(&telemetry_msg.metadata);
    let declared_connections =
        device_connections::declared_connections_from_metadata(&telemetry_msg.metadata);

    let record = NewTelemetryRecord {
        tenant_id: DEFAULT_TENANT_ID.to_string(),
        device_id: telemetry_msg.device_id.clone(),
        payload: payload.to_vec(),
        temperature: Some(telemetry_msg.temperature),
        humidity: Some(telemetry_msg.humidity),
        battery_level: Some(telemetry_msg.battery_level),
        custom_json,
        latitude: if has_location {
            Some(telemetry_msg.latitude)
        } else {
            None
        },
        longitude: if has_location {
            Some(telemetry_msg.longitude)
        } else {
            None
        },
        speed: if has_location {
            Some(telemetry_msg.speed)
        } else {
            None
        },
        altitude: if has_location {
            Some(telemetry_msg.altitude)
        } else {
            None
        },
        heading: if has_location {
            Some(telemetry_msg.heading)
        } else {
            None
        },
    };

    match telemetry_service::record(
        &mut conn,
        record,
        declared_connections,
        observed_network_hosts,
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

            // Update device latest location (sync, using same connection)
            if has_location {
                use crate::db::schema::devices::dsl;
                let _ = diesel::update(dsl::devices.filter(dsl::id.eq(&telemetry_msg.device_id)))
                    .set((
                        dsl::latest_latitude.eq(Some(telemetry_msg.latitude)),
                        dsl::latest_longitude.eq(Some(telemetry_msg.longitude)),
                    ))
                    .execute(&mut conn);
            }

            // Evaluate rules against this telemetry data
            let data = TelemetryData {
                temperature: telemetry_msg.temperature,
                humidity: telemetry_msg.humidity,
                battery_level: telemetry_msg.battery_level,
                latitude: telemetry_msg.latitude,
                longitude: telemetry_msg.longitude,
                speed: telemetry_msg.speed,
                altitude: telemetry_msg.altitude,
                heading: telemetry_msg.heading,
            };

            let cache = match rule_cache.read() {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to read-lock rule cache: {}", e);
                    return Vec::new();
                }
            };

            let mut actions = evaluate_telemetry_for_tenant(
                &device.tenant_id,
                &telemetry_msg.device_id,
                device.device_type_id,
                device.fleet_id,
                &data,
                &cache,
            );

            let geofence_actions = evaluate_geofence_for_tenant(
                &device.tenant_id,
                &telemetry_msg.device_id,
                device.device_type_id,
                device.fleet_id,
                &data,
                &cache,
            );
            actions.extend(geofence_actions);
            actions
        }
        Err(e) => {
            warn!("Failed to record telemetry: {}", e);
            Vec::new()
        }
    }
}
