use chrono::{DateTime, Utc};
use extrittio_device_contract::{
    CompiledContractDocument, FieldValueType, PayloadEncoding, RouteDirection, SchemaFormat,
    validate_instance,
};
use serde::Deserialize;
use serde_json::Value;
use tracing::{info, warn};

use crate::domains::events::types::{DeviceMetricSample, MetricValue, RecordDeviceEvent};
use crate::persistence::Persistence;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::evaluate::evaluate_telemetry_for_tenant;
use crate::rule_engine::types::TelemetryData;
use crate::tenancy::DeviceIdentity;

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
pub async fn handle_event(
    persistence: &Persistence,
    identity: &DeviceIdentity,
    route_key: &str,
    bytes: &[u8],
    rule_cache: &std::sync::RwLock<RuleCache>,
) -> usize {
    let envelope: DeviceEventEnvelope = match serde_json::from_slice(bytes) {
        Ok(envelope) => envelope,
        Err(error) => {
            warn!(device_id = identity.device_id(), route_key, %error, "Invalid event envelope");
            return 0;
        }
    };
    if envelope.api_version != 1 || uuid::Uuid::parse_str(&envelope.event_id).is_err() {
        warn!(
            device_id = identity.device_id(),
            route_key,
            event_id = envelope.event_id,
            "Unsupported event envelope version or invalid event ID"
        );
        return 0;
    }

    let assigned = match persistence
        .devices
        .assigned_contract(identity.tenant_id(), identity.device_id())
        .await
    {
        Ok(Some(contract)) => contract,
        Ok(None) => {
            warn!(
                device_id = identity.device_id(),
                "Device has no assigned contract"
            );
            return 0;
        }
        Err(error) => {
            warn!(device_id = identity.device_id(), %error, "Failed to load assigned contract");
            return 0;
        }
    };
    if envelope.contract_hash != assigned.contract_hash {
        warn!(
            device_id = identity.device_id(),
            supplied_hash = envelope.contract_hash,
            assigned_hash = assigned.contract_hash,
            "Dropping event produced from an unassigned contract"
        );
        return 0;
    }
    let contract: CompiledContractDocument = match serde_json::from_value(assigned.document) {
        Ok(contract) => contract,
        Err(error) => {
            warn!(contract_id = assigned.id, %error, "Stored device contract is invalid");
            return 0;
        }
    };
    if contract.device_id != identity.device_id()
        || bytes.len() as u64 > contract.runtime.max_message_bytes
    {
        warn!(
            device_id = identity.device_id(),
            route_key, "Event violates contract identity or size limit"
        );
        return 0;
    }
    let route = match contract.routes.get(route_key) {
        Some(route)
            if route.direction == RouteDirection::DeviceToCloud
                && route.encoding == PayloadEncoding::Json =>
        {
            route
        }
        _ => {
            warn!(
                device_id = identity.device_id(),
                route_key, "Event route is not declared as device-to-cloud JSON"
            );
            return 0;
        }
    };
    let schema = match contract.schemas.get(&route.message_schema) {
        Some(schema) if schema.format == SchemaFormat::JsonSchema => &schema.schema,
        _ => {
            warn!(
                device_id = identity.device_id(),
                route_key, "Event route does not reference a JSON Schema"
            );
            return 0;
        }
    };
    if let Err(error) = validate_instance(
        extrittio_device_contract::SchemaProfile::ExtrittioV1,
        schema,
        &envelope.payload,
    ) {
        warn!(device_id = identity.device_id(), route_key, %error, "Event payload failed contract validation");
        return 0;
    }

    let mut metrics = Vec::new();
    let mut rule_metrics = std::collections::BTreeMap::new();
    for (stream_key, stream) in &contract.streams {
        if stream.route != route_key {
            continue;
        }
        for (field_path, field) in &stream.fields {
            let Some(value) = envelope.payload.pointer(field_path) else {
                continue;
            };
            let Some(value) = metric_value(field.value_type, value) else {
                warn!(
                    device_id = identity.device_id(),
                    route_key, field_path, "Metric value does not match its contract type"
                );
                return 0;
            };
            let numeric_value = match &value {
                MetricValue::Float64(value) => Some(*value),
                MetricValue::Int64(value) => Some(*value as f64),
                _ => None,
            };
            if let Some(value) = numeric_value {
                let canonical = format!(
                    "{}.{}",
                    stream_key,
                    field_path.trim_start_matches('/').replace('/', ".")
                );
                rule_metrics.insert(canonical, value);
                if let Some(semantic) = &field.semantic {
                    rule_metrics.insert(semantic.clone(), value);
                }
            }
            metrics.push(DeviceMetricSample {
                stream_key: stream_key.clone(),
                field_path: field_path.clone(),
                value,
            });
        }
    }

    let pending_actions = match persistence.devices.ingress_context(identity).await {
        Ok(Some(context)) => {
            let cache = match rule_cache.read() {
                Ok(cache) => cache,
                Err(error) => {
                    warn!(device_id = identity.device_id(), %error, "Failed to read-lock rule cache");
                    return 0;
                }
            };
            evaluate_telemetry_for_tenant(
                identity.tenant_id_str(),
                identity.device_id(),
                context.device_type_id,
                context.fleet_id,
                context.blueprint_id.as_deref(),
                &TelemetryData {
                    temperature: 0.0,
                    humidity: 0.0,
                    battery_level: 0.0,
                    latitude: 0.0,
                    longitude: 0.0,
                    speed: 0.0,
                    altitude: 0.0,
                    heading: 0.0,
                    metrics: rule_metrics,
                },
                &cache,
            )
        }
        Ok(None) => return 0,
        Err(error) => {
            warn!(device_id = identity.device_id(), %error, "Failed to load event device context");
            return 0;
        }
    };

    let metric_count = metrics.len();
    match persistence
        .events
        .record(
            identity.tenant_id(),
            RecordDeviceEvent {
                event_id: envelope.event_id,
                device_id: identity.device_id().to_string(),
                contract_id: assigned.id,
                route_key: route_key.to_string(),
                occurred_at: envelope.occurred_at,
                received_at: Utc::now(),
                payload: envelope.payload,
                metrics,
                pending_actions,
            },
        )
        .await
    {
        Ok(outcome) if outcome.recorded => {
            info!(
                device_id = identity.device_id(),
                route_key, metric_count, "Recorded contract event"
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

fn metric_value(value_type: FieldValueType, value: &Value) -> Option<MetricValue> {
    match value_type {
        FieldValueType::Float64 => value.as_f64().map(MetricValue::Float64),
        FieldValueType::Int64 => value.as_i64().map(MetricValue::Int64),
        FieldValueType::String => value
            .as_str()
            .map(|value| MetricValue::String(value.to_string())),
        FieldValueType::Boolean => value.as_bool().map(MetricValue::Boolean),
        FieldValueType::Json => Some(MetricValue::Json(value.clone())),
    }
}
