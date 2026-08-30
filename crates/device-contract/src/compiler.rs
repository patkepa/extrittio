use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::CONTRACT_API_VERSION;
use crate::canonical::{canonical_json, sha256};
use crate::error::ContractError;
use crate::model::{
    AggregateKind, CommandDanger, CommandIdempotency, ConfigurationMode, DeliverySemantics,
    FieldPresentation, FieldValueType, FirmwareDefinition, HealthDefinition, OrderingSemantics,
    PayloadEncoding, PresentationDefinition, RelationshipDefinition, ReportedStateDefinition,
    RouteDirection, SchemaFormat, TransportProtocol,
};
use crate::schema::{SchemaProfile, validate_instance};
use crate::validation::{ValidatedBlueprint, parse_duration_ms};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractHash([u8; 32]);

impl ContractHash {
    pub(crate) fn digest(value: &[u8]) -> Self {
        Self(sha256(value))
    }

    #[must_use]
    pub fn as_hex(&self) -> String {
        let mut value = String::with_capacity(64);
        for byte in self.0 {
            use fmt::Write as _;
            write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
        }
        value
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContractHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.as_hex())
    }
}

impl FromStr for ContractHash {
    type Err = ContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ContractError::InvalidContext(
                "contract hash must contain 64 hexadecimal characters".to_string(),
            ));
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let pair = std::str::from_utf8(pair).map_err(|_| {
                ContractError::InvalidContext("contract hash is not UTF-8".to_string())
            })?;
            bytes[index] = u8::from_str_radix(pair, 16).map_err(|_| {
                ContractError::InvalidContext("contract hash is not hexadecimal".to_string())
            })?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for ContractHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_hex())
    }
}

impl<'de> Deserialize<'de> for ContractHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileContext {
    pub contract_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub blueprint_revision_id: String,
    pub blueprint_revision: u32,
    pub transport_bindings: BTreeMap<String, ResolvedTransport>,
    /// Ordered from least to most specific, for example tenant, fleet, device.
    pub configuration_layers: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTransport {
    pub protocol: TransportProtocol,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ca_pem: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledContract {
    pub hash: ContractHash,
    pub document: CompiledContractDocument,
}

impl CompiledContract {
    pub fn canonical_document(&self) -> Result<Vec<u8>, ContractError> {
        canonical_json(&self.document)
    }

    pub fn verify_hash(&self) -> Result<bool, ContractError> {
        Ok(self.hash == ContractHash::digest(&self.canonical_document()?))
    }
}

impl CompiledContractDocument {
    /// Canonical serialized representation used for provisioning integrity.
    pub fn canonical_document(&self) -> Result<Vec<u8>, ContractError> {
        canonical_json(self)
    }

    /// Recompute the deterministic hash of this materialized document.
    pub fn contract_hash(&self) -> Result<ContractHash, ContractError> {
        Ok(ContractHash::digest(&self.canonical_document()?))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledContractDocument {
    pub contract_api: u32,
    pub contract_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub blueprint_revision_id: String,
    pub blueprint_revision: u32,
    pub blueprint_key: String,
    pub blueprint_name: String,
    pub runtime: CompiledRuntime,
    pub transports: BTreeMap<String, CompiledTransport>,
    pub routes: BTreeMap<String, CompiledRoute>,
    pub schemas: BTreeMap<String, CompiledSchema>,
    pub streams: BTreeMap<String, CompiledStream>,
    pub commands: BTreeMap<String, CompiledCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<CompiledConfiguration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_state: Option<ReportedStateDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<FirmwareDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<RelationshipDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledRuntime {
    pub heartbeat_interval_ms: u64,
    pub offline_after_ms: u64,
    pub max_message_bytes: u64,
    pub max_messages_per_minute: u32,
    pub max_metric_cardinality: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledTransport {
    pub protocol: TransportProtocol,
    pub endpoint: String,
    pub delivery: DeliverySemantics,
    pub ordering: OrderingSemantics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ca_pem: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledRoute {
    pub transport: String,
    pub direction: RouteDirection,
    pub address: String,
    pub message_schema: String,
    pub encoding: PayloadEncoding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledSchema {
    pub format: SchemaFormat,
    pub schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledStream {
    pub route: String,
    pub timestamp: String,
    pub fields: BTreeMap<String, CompiledField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledField {
    pub value_type: FieldValueType,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic: Option<String>,
    pub index: bool,
    pub aggregates: Vec<AggregateKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<FieldPresentation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledCommand {
    pub request_route: String,
    pub response_route: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub danger: CommandDanger,
    pub timeout_ms: u64,
    pub input_schema: Value,
    pub result_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability: Option<String>,
    pub idempotency: CommandIdempotency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledConfiguration {
    pub schema: Value,
    pub desired: Value,
    pub mode: ConfigurationMode,
    pub acknowledgement_timeout_ms: u64,
    pub atomic: bool,
}

#[derive(Debug, Default)]
pub struct BlueprintCompiler;

impl BlueprintCompiler {
    pub fn compile(
        blueprint: &ValidatedBlueprint,
        context: &CompileContext,
    ) -> Result<CompiledContract, ContractError> {
        validate_context(context)?;
        let blueprint = blueprint.blueprint();
        let runtime = &blueprint.spec.runtime;

        let mut transports = BTreeMap::new();
        for transport in &blueprint.spec.transports {
            let resolved = context
                .transport_bindings
                .get(&transport.binding)
                .ok_or_else(|| ContractError::MissingTransportBinding(transport.binding.clone()))?;
            if resolved.protocol != transport.protocol {
                return Err(ContractError::InvalidContext(format!(
                    "binding '{}' uses {:?}, expected {:?}",
                    transport.binding, resolved.protocol, transport.protocol
                )));
            }
            if resolved.endpoint.trim().is_empty() {
                return Err(ContractError::InvalidContext(format!(
                    "binding '{}' has an empty endpoint",
                    transport.binding
                )));
            }
            transports.insert(
                transport.key.clone(),
                CompiledTransport {
                    protocol: resolved.protocol,
                    endpoint: resolved.endpoint.clone(),
                    delivery: transport.qos.delivery,
                    ordering: transport.qos.ordering,
                    server_ca_pem: resolved.server_ca_pem.clone(),
                    credential_ref: resolved.credential_ref.clone(),
                },
            );
        }

        let routes = blueprint
            .spec
            .routes
            .iter()
            .map(|route| {
                (
                    route.key.clone(),
                    CompiledRoute {
                        transport: route.transport.clone(),
                        direction: route.direction,
                        address: route.address.replace("{device.id}", &context.device_id),
                        message_schema: route.message_schema.clone(),
                        encoding: route.encoding,
                    },
                )
            })
            .collect();

        let schemas = blueprint
            .spec
            .schemas
            .iter()
            .map(|schema| {
                (
                    schema.key.clone(),
                    CompiledSchema {
                        format: schema.format,
                        schema: schema.schema.clone(),
                    },
                )
            })
            .collect();

        let streams = blueprint
            .spec
            .streams
            .iter()
            .map(|stream| {
                let fields = stream
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.path.clone(),
                            CompiledField {
                                value_type: field.value_type,
                                label: field.label.clone(),
                                unit: field.unit.clone(),
                                semantic: field.semantic.clone(),
                                index: field.index,
                                aggregates: field.aggregates.clone(),
                                presentation: field.presentation.clone(),
                            },
                        )
                    })
                    .collect();
                (
                    stream.key.clone(),
                    CompiledStream {
                        route: stream.route.clone(),
                        timestamp: stream.timestamp.clone(),
                        fields,
                    },
                )
            })
            .collect();

        let commands = blueprint
            .spec
            .commands
            .iter()
            .map(|command| {
                (
                    command.key.clone(),
                    CompiledCommand {
                        request_route: command.request_route.clone(),
                        response_route: command.response_route.clone(),
                        label: command.label.clone(),
                        description: command.description.clone(),
                        danger: command.danger,
                        timeout_ms: parse_duration_ms(&command.timeout)
                            .expect("validated command duration"),
                        input_schema: command.input_schema.clone(),
                        result_schema: command.result_schema.clone(),
                        availability: command.availability.clone(),
                        idempotency: command.idempotency,
                    },
                )
            })
            .collect();

        let configuration = blueprint
            .spec
            .configuration
            .as_ref()
            .map(|configuration| compile_configuration(configuration, context))
            .transpose()?;
        if configuration.is_none() && !context.configuration_layers.is_empty() {
            return Err(ContractError::InvalidContext(
                "configuration layers were provided for a blueprint without configuration"
                    .to_string(),
            ));
        }

        let document = CompiledContractDocument {
            contract_api: CONTRACT_API_VERSION,
            contract_id: context.contract_id.clone(),
            tenant_id: context.tenant_id.clone(),
            device_id: context.device_id.clone(),
            blueprint_revision_id: context.blueprint_revision_id.clone(),
            blueprint_revision: context.blueprint_revision,
            blueprint_key: blueprint.metadata.key.clone(),
            blueprint_name: blueprint.metadata.name.clone(),
            runtime: CompiledRuntime {
                heartbeat_interval_ms: parse_duration_ms(&runtime.heartbeat.interval)
                    .expect("validated heartbeat interval"),
                offline_after_ms: parse_duration_ms(&runtime.heartbeat.offline_after)
                    .expect("validated offline timeout"),
                max_message_bytes: runtime.limits.max_message_bytes,
                max_messages_per_minute: runtime.limits.max_messages_per_minute,
                max_metric_cardinality: runtime.limits.max_metric_cardinality,
            },
            transports,
            routes,
            schemas,
            streams,
            commands,
            configuration,
            reported_state: blueprint.spec.reported_state.clone(),
            health: blueprint.spec.health.clone(),
            firmware: blueprint.spec.firmware.clone(),
            relationships: blueprint.spec.relationships.clone(),
            presentation: blueprint.spec.presentation.clone(),
        };
        let hash = ContractHash::digest(&canonical_json(&document)?);
        Ok(CompiledContract { hash, document })
    }
}

fn compile_configuration(
    configuration: &crate::model::ConfigurationDefinition,
    context: &CompileContext,
) -> Result<CompiledConfiguration, ContractError> {
    let mut desired = defaults_from_schema(&configuration.schema);
    if !configuration.defaults.is_null() {
        merge_value(&mut desired, &configuration.defaults);
    }
    for layer in &context.configuration_layers {
        merge_value(&mut desired, layer);
    }
    validate_instance(SchemaProfile::ExtrittioV1, &configuration.schema, &desired)?;
    Ok(CompiledConfiguration {
        schema: configuration.schema.clone(),
        desired,
        mode: configuration.apply.mode,
        acknowledgement_timeout_ms: parse_duration_ms(&configuration.apply.acknowledgement_timeout)
            .expect("validated acknowledgement timeout"),
        atomic: configuration.apply.atomic,
    })
}

fn defaults_from_schema(schema: &Value) -> Value {
    let Some(schema) = schema.as_object() else {
        return Value::Null;
    };
    if let Some(default) = schema.get("default") {
        return default.clone();
    }
    if schema.get("type").and_then(Value::as_str) == Some("object") {
        let mut defaults = Map::new();
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (key, child_schema) in properties {
                let child = defaults_from_schema(child_schema);
                if !child.is_null() {
                    defaults.insert(key.clone(), child);
                }
            }
        }
        return Value::Object(defaults);
    }
    Value::Null
}

fn merge_value(target: &mut Value, overlay: &Value) {
    match (target, overlay) {
        (Value::Object(target), Value::Object(overlay)) => {
            for (key, value) in overlay {
                merge_value(target.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (target, overlay) => *target = overlay.clone(),
    }
}

fn validate_context(context: &CompileContext) -> Result<(), ContractError> {
    for (name, value) in [
        ("contract_id", context.contract_id.as_str()),
        ("tenant_id", context.tenant_id.as_str()),
        ("device_id", context.device_id.as_str()),
        (
            "blueprint_revision_id",
            context.blueprint_revision_id.as_str(),
        ),
    ] {
        if value.trim().is_empty() || value.len() > 128 {
            return Err(ContractError::InvalidContext(format!(
                "{name} must contain 1-128 bytes"
            )));
        }
    }
    if context.blueprint_revision == 0 {
        return Err(ContractError::InvalidContext(
            "blueprint_revision must be positive".to_string(),
        ));
    }
    if !context
        .device_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ContractError::InvalidContext(
            "device_id contains characters unsafe for route expansion".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::DeviceBlueprint;
    use crate::validation::validate_blueprint;

    fn fixture() -> ValidatedBlueprint {
        let blueprint: DeviceBlueprint =
            serde_yaml::from_str(include_str!("../tests/fixtures/cold-room.yaml")).unwrap();
        validate_blueprint(blueprint).unwrap()
    }

    fn context() -> CompileContext {
        CompileContext {
            contract_id: "dc_1".to_string(),
            tenant_id: "tenant-a".to_string(),
            device_id: "device-1".to_string(),
            blueprint_revision_id: "dbr_1".to_string(),
            blueprint_revision: 1,
            transport_bindings: BTreeMap::from([(
                "primary_zenoh".to_string(),
                ResolvedTransport {
                    protocol: TransportProtocol::Zenoh,
                    endpoint: "tcp/example.internal:7447".to_string(),
                    server_ca_pem: None,
                    credential_ref: Some("device-certificate".to_string()),
                },
            )]),
            configuration_layers: vec![json!({"sample_interval_seconds": 30})],
        }
    }

    #[test]
    fn compiles_and_hashes_deterministically() {
        let first = BlueprintCompiler::compile(&fixture(), &context()).unwrap();
        let second = BlueprintCompiler::compile(&fixture(), &context()).unwrap();
        assert_eq!(first, second);
        assert!(first.verify_hash().unwrap());
        assert_eq!(first.hash.as_hex().len(), 64);
        assert_eq!(
            first.document.routes["environment"].address,
            "extrittio/devices/device-1/events/environment"
        );
        assert_eq!(
            first.document.configuration.unwrap().desired,
            json!({"sample_interval_seconds": 30, "temperature_offset": 0})
        );
    }

    #[test]
    fn rejects_invalid_configuration_layer() {
        let mut context = context();
        context.configuration_layers = vec![json!({"sample_interval_seconds": 2})];
        let error = BlueprintCompiler::compile(&fixture(), &context).unwrap_err();
        assert!(matches!(error, ContractError::InvalidInstance(_, _)));
    }
}
