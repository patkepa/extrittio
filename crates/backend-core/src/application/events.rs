use super::require_permission;
use crate::events::{
    DeviceEventRepository, DeviceLocationQuery, DeviceLocationRecord, DeviceMetricQuery,
    DeviceMetricRecord,
};
use crate::{ApplicationError, Permission, TenantContext};
use std::sync::Arc;

fn location_batch_ids(device_ids: Vec<String>) -> Result<Vec<String>, ApplicationError> {
    if device_ids.len() > 500 || device_ids.iter().any(|id| id.trim().is_empty()) {
        return Err(ApplicationError::InvalidInput(
            "Location batches accept at most 500 nonempty device IDs".into(),
        ));
    }
    let ids: std::collections::BTreeSet<_> = device_ids.into_iter().collect();
    Ok(ids.into_iter().collect())
}

#[cfg(test)]
mod location_batch_tests {
    use super::location_batch_ids;
    #[test]
    fn location_batches_are_bounded_and_preserve_opaque_ids() {
        assert!(location_batch_ids(vec!["device".into(); 501]).is_err());
        assert!(location_batch_ids(vec!["  ".into()]).is_err());
        assert!(location_batch_ids(Vec::new()).unwrap().is_empty());
        assert_eq!(
            location_batch_ids(vec![
                "opaque-b".into(),
                "opaque-a".into(),
                "opaque-b".into()
            ])
            .unwrap(),
            vec!["opaque-a", "opaque-b"]
        );
    }
}

#[derive(Clone)]
pub struct EventApplication {
    repository: Arc<dyn DeviceEventRepository>,
    devices: Arc<dyn DeviceRepository>,
    clock: Arc<dyn Clock>,
}
impl EventApplication {
    pub async fn latest_locations(
        &self,
        ctx: &TenantContext,
        device_ids: Vec<String>,
    ) -> Result<Vec<crate::events::LocatedDeviceRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadTelemetry)?;
        require_permission(ctx, Permission::ReadDevices)?;
        let ids = location_batch_ids(device_ids)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .repository
            .latest_locations(ctx.tenant_id(), ids, self.clock.now())
            .await?)
    }

    pub fn new(
        repository: Arc<dyn DeviceEventRepository>,
        devices: Arc<dyn DeviceRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            devices,
            clock,
        }
    }

    pub async fn latest_location(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<DeviceLocationRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadTelemetry)?;
        let assigned = self
            .devices
            .assigned_contract(ctx.tenant_id(), device_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!("Device '{device_id}' has no assigned contract"))
            })?;
        let contract: CompiledContractDocument = serde_json::from_value(assigned.document)
            .map_err(|error| {
                ApplicationError::Internal(format!("Stored device contract is invalid: {error}"))
            })?;
        if contract.device_id != device_id {
            return Err(ApplicationError::Internal(
                "Stored contract identity mismatch".into(),
            ));
        }
        let Some(binding) = contract.location else {
            return Ok(None);
        };
        let now = self.clock.now();
        let since = i64::try_from(binding.max_age_ms)
            .ok()
            .and_then(chrono::Duration::try_milliseconds)
            .and_then(|age| now.checked_sub_signed(age))
            .ok_or_else(|| {
                ApplicationError::Internal(
                    "Location freshness is outside the supported range".into(),
                )
            })?;
        Ok(self
            .repository
            .latest_location(
                ctx.tenant_id(),
                device_id,
                DeviceLocationQuery {
                    contract_id: assigned.id,
                    stream_key: binding.stream,
                    latitude_path: binding.latitude_path,
                    longitude_path: binding.longitude_path,
                    since,
                    now,
                },
            )
            .await?)
    }
    pub async fn list_metrics(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        mut query: DeviceMetricQuery,
    ) -> Result<Vec<DeviceMetricRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadTelemetry)?;
        query.limit = query.limit.clamp(1, 10_000);
        self.repository
            .list_metrics(ctx.tenant_id(), device_id, query)
            .await
            .map_err(|error| match error {
                crate::PersistenceError::HistoryExpired => ApplicationError::InvalidOperation(
                    "Metric history is outside the retained raw-event range".into(),
                ),
                other => other.into(),
            })?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))
    }
}

use crate::devices::DeviceRepository;
use crate::events::{
    DeviceMetricSample, EventInput, MetricValue, RecordDeviceEvent, RecordDeviceEventOutcome,
};
use crate::{Clock, TenantId};
use extrittio_device_contract::{
    CompiledContractDocument, FieldValueType, PayloadEncoding, RouteDirection, SchemaFormat,
    validate_instance,
};
use serde_json::Value;

#[derive(Clone)]
pub struct EventIngressApplication {
    repository: Arc<dyn DeviceEventRepository>,
    devices: Arc<dyn DeviceRepository>,
    ingress: Arc<dyn crate::device_ingress::DeviceIngressRepository>,
    clock: Arc<dyn Clock>,
}
impl EventIngressApplication {
    pub fn new(
        repository: Arc<dyn DeviceEventRepository>,
        devices: Arc<dyn DeviceRepository>,
        ingress: Arc<dyn crate::device_ingress::DeviceIngressRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            devices,
            ingress,
            clock,
        }
    }
    pub async fn record(
        &self,
        tenant: &TenantId,
        device_id: &str,
        route_key: &str,
        envelope: EventInput,
        snapshot: crate::rule_snapshots::RuleEvaluationSnapshot,
    ) -> Result<RecordDeviceEventOutcome, ApplicationError> {
        if envelope.api_version != 1 || uuid::Uuid::parse_str(&envelope.event_id).is_err() {
            return Err(ApplicationError::InvalidInput(
                "Unsupported event envelope version or invalid event ID".into(),
            ));
        }
        let assigned = self
            .devices
            .assigned_contract(tenant, device_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("Device has no assigned contract".into()))?;
        if envelope.contract_hash != assigned.contract_hash {
            return Err(ApplicationError::InvalidInput(
                "Event was produced from an unassigned contract".into(),
            ));
        }
        let contract: CompiledContractDocument = serde_json::from_value(assigned.document)
            .map_err(|error| {
                ApplicationError::Internal(format!("Stored device contract is invalid: {error}"))
            })?;
        if contract.device_id != device_id
            || envelope.encoded_size > contract.runtime.max_message_bytes
        {
            return Err(ApplicationError::InvalidInput(
                "Event violates contract identity or size limit".into(),
            ));
        }
        let route = contract
            .routes
            .get(route_key)
            .filter(|route| {
                route.direction == RouteDirection::DeviceToCloud
                    && route.encoding == PayloadEncoding::Json
            })
            .ok_or_else(|| {
                ApplicationError::InvalidInput(
                    "Event route is not declared as device-to-cloud JSON".into(),
                )
            })?;
        let schema = contract
            .schemas
            .get(&route.message_schema)
            .filter(|schema| schema.format == SchemaFormat::JsonSchema)
            .ok_or_else(|| {
                ApplicationError::InvalidInput(
                    "Event route does not reference a JSON Schema".into(),
                )
            })?;
        validate_instance(
            extrittio_device_contract::SchemaProfile::ExtrittioV1,
            &schema.schema,
            &envelope.payload,
        )
        .map_err(|error| {
            ApplicationError::InvalidInput(format!(
                "Event payload failed contract validation: {error}"
            ))
        })?;
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
                let value = metric_value(field.value_type, value).ok_or_else(|| {
                    ApplicationError::InvalidInput(format!(
                        "Metric '{field_path}' does not match its contract type"
                    ))
                })?;
                let numeric_value = match &value {
                    MetricValue::Float64(value) => {
                        Some(crate::rule_engine::number::MetricNumber::Float(*value))
                    }
                    MetricValue::Int64(value) => {
                        Some(crate::rule_engine::number::MetricNumber::Integer(*value))
                    }
                    _ => None,
                };
                if let Some(value) = numeric_value {
                    let canonical = crate::rule_engine::metric::MetricSelector {
                        stream_key: stream_key.clone(),
                        field_path: field_path.clone(),
                    }
                    .field_key();
                    rule_metrics.insert(canonical, value);
                }
                metrics.push(DeviceMetricSample {
                    stream_key: stream_key.clone(),
                    field_path: field_path.clone(),
                    value,
                });
            }
        }

        // Both stores persist microseconds; normalize before either adapter can
        // round differently. Occurrence remains device supplied, receipt server supplied.
        use chrono::Timelike;
        let received = self.clock.now();
        let received_at = received
            .with_nanosecond(received.nanosecond() / 1_000 * 1_000)
            .expect("microsecond truncation is a valid nanosecond");
        let occurred_at = envelope
            .occurred_at
            .with_nanosecond(envelope.occurred_at.nanosecond() / 1_000 * 1_000)
            .expect("microsecond truncation is a valid nanosecond");
        let location = contract.event_location(
            route_key,
            &envelope.payload,
            received_at
                .signed_duration_since(occurred_at)
                .num_milliseconds(),
        );
        let identity = crate::DeviceIdentity::new(tenant.as_str(), device_id)
            .map_err(|error| ApplicationError::InvalidInput(error.to_string()))?;
        let context = self
            .ingress
            .ingress_context(&identity)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))?;
        let rule_evaluation = crate::rule_snapshots::DeviceRuleEvaluation {
            snapshot,
            tenant: tenant.clone(),
            device_id: device_id.to_owned(),
            fleet_id: context.fleet_id,
            blueprint_id: context.blueprint_id,
            blueprint_revision_id: Some(assigned.blueprint_revision_id.clone()),
            input: crate::rule_snapshots::RuleEvaluationInput::Telemetry {
                data: crate::rule_engine::types::TelemetryData {
                    latitude: location.map(|point| point.0),
                    longitude: location.map(|point| point.1),
                    metrics: rule_metrics,
                },
                geofence: location.is_some(),
            },
            observed_at: received_at.naive_utc(),
        };
        self.repository
            .record(
                tenant,
                RecordDeviceEvent {
                    event_id: envelope.event_id,
                    device_id: device_id.to_owned(),
                    contract_id: assigned.id,
                    route_key: route_key.to_owned(),
                    occurred_at,
                    received_at,
                    payload: envelope.payload,
                    metrics,
                    rule_evaluation,
                },
            )
            .await
            .map_err(|error| match error {
                crate::PersistenceError::HistoryExpired => ApplicationError::InvalidOperation(
                    "Event occurred before retained metric history".into(),
                ),
                other => other.into(),
            })
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

/// Decoder selection is contract policy; decoding itself belongs to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractIngressRoute {
    CommandResponse,
    JsonEvent { route_key: String },
    Unsupported { route_key: String },
}

#[derive(Clone)]
pub struct ContractIngressApplication {
    devices: Arc<dyn DeviceRepository>,
}
impl ContractIngressApplication {
    pub fn new(devices: Arc<dyn DeviceRepository>) -> Self {
        Self { devices }
    }

    pub async fn resolve(
        &self,
        identity: &crate::DeviceIdentity,
        address: &str,
    ) -> Result<Option<ContractIngressRoute>, ApplicationError> {
        let Some(assigned) = self
            .devices
            .assigned_contract(identity.tenant_id(), identity.device_id())
            .await?
        else {
            return Ok(None);
        };
        let contract: CompiledContractDocument = serde_json::from_value(assigned.document)
            .map_err(|error| {
                ApplicationError::Internal(format!("Stored device contract is invalid: {error}"))
            })?;
        let Some((route_key, route)) = contract.routes.iter().find(|(_, route)| {
            route.direction == RouteDirection::DeviceToCloud && route.address == address
        }) else {
            return Ok(None);
        };
        // Preserve command response precedence even if its route uses JSON.
        Ok(Some(
            if contract
                .commands
                .values()
                .any(|command| command.response_route == *route_key)
            {
                ContractIngressRoute::CommandResponse
            } else if route.encoding == PayloadEncoding::Json {
                ContractIngressRoute::JsonEvent {
                    route_key: route_key.clone(),
                }
            } else {
                ContractIngressRoute::Unsupported {
                    route_key: route_key.clone(),
                }
            },
        ))
    }
}
