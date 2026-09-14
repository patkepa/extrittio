use prost::Message;
use tracing::{info, warn};

use crate::domains::telemetry::types::TelemetryWrite;
use crate::persistence::RepositorySet;
use crate::rule_engine::types::TelemetryData;
use crate::tenancy::DeviceIdentity;

use extrittio_common::extrittio::DeviceTelemetry;

const MAX_OPTIMISTIC_RETRIES: usize = 3;

/// Decode and atomically persist telemetry, latest state, device projections,
/// observed hosts, and resulting durable rule actions.
pub async fn handle_telemetry(
    persistence: &RepositorySet,
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

    let custom_json = if telemetry.metadata.is_empty() {
        None
    } else {
        serde_json::to_value(&telemetry.metadata).ok()
    };
    // Preserve locations from pre-presence-flag clients while allowing updated
    // clients to report the valid coordinate (0, 0).
    let has_location =
        telemetry.has_location || telemetry.latitude != 0.0 || telemetry.longitude != 0.0;
    let data = TelemetryData {
        temperature: telemetry.temperature,
        humidity: telemetry.humidity,
        battery_level: telemetry.battery_level,
        latitude: has_location.then_some(telemetry.latitude),
        longitude: has_location.then_some(telemetry.longitude),
        speed: telemetry.speed,
        altitude: telemetry.altitude,
        heading: telemetry.heading,
        metrics: std::collections::BTreeMap::new(),
    };

    for _ in 0..MAX_OPTIMISTIC_RETRIES {
        let context = match persistence.device_ingress.ingress_context(identity).await {
            Ok(Some(context)) => context,
            Ok(None) => {
                warn!(
                    "Dropping telemetry from unregistered device: {}",
                    telemetry.device_id
                );
                return 0;
            }
            Err(error) => {
                warn!("Failed to load telemetry device context: {error}");
                return 0;
            }
        };
        let observed_at = chrono::Utc::now().naive_utc();
        let snapshot = match rule_cache.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                warn!("Failed to obtain rule snapshot: {error}");
                return 0;
            }
        };
        let rule_evaluation = extrittio_backend_core::rule_snapshots::DeviceRuleEvaluation {
            snapshot,
            tenant: identity.tenant_id().clone(),
            device_id: identity.device_id().to_owned(),
            device_type_id: context.device_type_id,
            fleet_id: context.fleet_id,
            blueprint_id: context.blueprint_id,
            input: extrittio_backend_core::rule_snapshots::RuleEvaluationInput::Telemetry {
                data: data.clone(),
                geofence: true,
            },
            observed_at,
        };
        let outcome = persistence
            .telemetry
            .record(
                identity,
                TelemetryWrite {
                    expected_device_type_id: context.device_type_id,
                    expected_fleet_id: context.fleet_id,
                    payload: payload.to_vec(),
                    temperature: Some(telemetry.temperature),
                    humidity: Some(telemetry.humidity),
                    battery_level: Some(telemetry.battery_level),
                    custom_json: custom_json.clone(),
                    latitude: has_location.then_some(telemetry.latitude),
                    longitude: has_location.then_some(telemetry.longitude),
                    speed: has_location.then_some(telemetry.speed),
                    altitude: has_location.then_some(telemetry.altitude),
                    heading: has_location.then_some(telemetry.heading),
                    rule_evaluation,
                    observed_at,
                },
            )
            .await;
        match outcome {
            Ok(outcome) if outcome.recorded => {
                info!(
                    "Recorded telemetry from device {}: temp={}, humidity={}, battery={}",
                    telemetry.device_id,
                    telemetry.temperature,
                    telemetry.humidity,
                    telemetry.battery_level
                );
                return outcome.actions_enqueued;
            }
            Ok(_) => continue,
            Err(error) => {
                warn!("Failed to atomically record telemetry and rule actions: {error}");
                return 0;
            }
        }
    }
    warn!("Telemetry device context changed repeatedly; dropping sample");
    0
}
