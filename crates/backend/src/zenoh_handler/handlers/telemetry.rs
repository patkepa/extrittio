use prost::Message;
use tracing::{info, warn};

use crate::tenancy::DeviceIdentity;
use extrittio_backend_core::TelemetryIngressApplication;
use extrittio_backend_core::telemetry::TelemetryInput;

use extrittio_common::extrittio::DeviceTelemetry;

/// Decode and atomically persist telemetry, latest state, device projections,
/// and resulting durable rule actions through the core application.
pub(crate) async fn handle_telemetry(
    application: &TelemetryIngressApplication,
    identity: &DeviceIdentity,
    topic_device_id: &str,
    payload: &[u8],
    rule_cache: &crate::rule_snapshots::RuleSnapshotStore,
) -> usize {
    let telemetry = match DeviceTelemetry::decode(payload) {
        Ok(message) => message,
        Err(error) => {
            warn!("Failed to decode DeviceTelemetry: {error}");
            return 0;
        }
    };
    if !super::validate_topic_device("telemetry", topic_device_id, &telemetry.device_id) {
        return 0;
    }

    let input = TelemetryInput {
        payload: payload.to_vec(),
        temperature: telemetry.temperature,
        humidity: telemetry.humidity,
        battery_level: telemetry.battery_level,
        metadata: telemetry.metadata.into_iter().collect(),
        has_location: telemetry.has_location,
        latitude: telemetry.latitude,
        longitude: telemetry.longitude,
        speed: telemetry.speed,
        altitude: telemetry.altitude,
        heading: telemetry.heading,
    };
    match application.record(identity, input, rule_cache).await {
        Ok(outcome) if outcome.recorded => {
            info!(
                "Recorded telemetry from device {}: temp={}, humidity={}, battery={}",
                telemetry.device_id,
                telemetry.temperature,
                telemetry.humidity,
                telemetry.battery_level
            );
            outcome.actions_enqueued
        }
        Ok(_) => {
            warn!(
                "Dropping telemetry from unregistered device: {}",
                telemetry.device_id
            );
            0
        }
        Err(error) => {
            warn!("Failed to atomically record telemetry and rule actions: {error}");
            0
        }
    }
}
