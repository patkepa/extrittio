use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceBlueprint {
    pub api_version: String,
    pub kind: String,
    pub metadata: BlueprintMetadata,
    pub spec: BlueprintSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintMetadata {
    pub key: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintSpec {
    pub runtime: RuntimeDefinition,
    #[serde(default)]
    pub transports: Vec<TransportDefinition>,
    #[serde(default)]
    pub routes: Vec<RouteDefinition>,
    #[serde(default)]
    pub schemas: Vec<SchemaDefinition>,
    #[serde(default)]
    pub streams: Vec<StreamDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationDefinition>,
    #[serde(default)]
    pub commands: Vec<CommandDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<ConfigurationDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_state: Option<ReportedStateDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<FirmwareDefinition>,
    #[serde(default)]
    pub relationships: Vec<RelationshipDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDefinition {
    pub minimum_contract_api: u32,
    pub heartbeat: HeartbeatDefinition,
    pub limits: RuntimeLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatDefinition {
    pub interval: String,
    pub offline_after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLimits {
    pub max_message_bytes: u64,
    pub max_messages_per_minute: u32,
    #[serde(default = "default_max_metric_cardinality")]
    pub max_metric_cardinality: u32,
}

const fn default_max_metric_cardinality() -> u32 {
    128
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportDefinition {
    pub key: String,
    pub binding: String,
    pub protocol: TransportProtocol,
    #[serde(default)]
    pub qos: TransportQos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportProtocol {
    Zenoh,
    Mqtt,
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportQos {
    #[serde(default)]
    pub delivery: DeliverySemantics,
    #[serde(default)]
    pub ordering: OrderingSemantics,
}

impl Default for TransportQos {
    fn default() -> Self {
        Self {
            delivery: DeliverySemantics::AtLeastOnce,
            ordering: OrderingSemantics::PerDeviceStream,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliverySemantics {
    AtMostOnce,
    #[default]
    AtLeastOnce,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderingSemantics {
    None,
    #[default]
    PerDeviceStream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteDefinition {
    pub key: String,
    pub transport: String,
    pub direction: RouteDirection,
    pub address: String,
    pub message_schema: String,
    pub encoding: PayloadEncoding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteDirection {
    DeviceToCloud,
    CloudToDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadEncoding {
    Json,
    Cbor,
    Protobuf,
    ProtobufDynamic,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDefinition {
    pub key: String,
    pub format: SchemaFormat,
    pub schema: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaFormat {
    JsonSchema,
    ProtobufDescriptor,
    Opaque,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamDefinition {
    pub key: String,
    pub route: String,
    #[serde(default = "default_timestamp_source")]
    pub timestamp: String,
    #[serde(default)]
    pub fields: Vec<FieldDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention: Option<RetentionDefinition>,
}

fn default_timestamp_source() -> String {
    "envelope.occurred_at".to_string()
}

/// Coordinates are taken from one event in the declared stream, in WGS84 degrees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocationDefinition {
    pub stream: String,
    pub latitude_path: String,
    pub longitude_path: String,
    pub coordinate_system: CoordinateSystem,
    pub unit: CoordinateUnit,
    pub max_age: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSystem {
    Wgs84,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateUnit {
    Degrees,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDefinition {
    pub path: String,
    #[serde(rename = "type")]
    pub value_type: FieldValueType,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic: Option<String>,
    #[serde(default)]
    pub index: bool,
    #[serde(default)]
    pub aggregates: Vec<AggregateKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<FieldPresentation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldValueType {
    Float64,
    Int64,
    String,
    Boolean,
    Json,
}

impl FieldValueType {
    #[must_use]
    pub const fn is_numeric(self) -> bool {
        matches!(self, Self::Float64 | Self::Int64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateKind {
    Min,
    Max,
    Avg,
    Sum,
    Count,
    Last,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldPresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<ChartKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Line,
    Step,
    Bar,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollup_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandDefinition {
    pub key: String,
    pub request_route: String,
    pub response_route: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub danger: CommandDanger,
    pub timeout: String,
    pub input_schema: Value,
    pub result_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability: Option<String>,
    #[serde(default)]
    pub idempotency: CommandIdempotency,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandDanger {
    #[default]
    Normal,
    Confirm,
    Critical,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandIdempotency {
    #[default]
    NonIdempotent,
    Idempotent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationDefinition {
    pub schema: Value,
    #[serde(default)]
    pub defaults: Value,
    pub apply: ConfigurationApply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationApply {
    #[serde(default)]
    pub mode: ConfigurationMode,
    pub acknowledgement_timeout: String,
    #[serde(default = "default_true")]
    pub atomic: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationMode {
    #[default]
    DesiredReported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportedStateDefinition {
    pub schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDefinition {
    #[serde(default)]
    pub signals: Vec<HealthSignalDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSignalDefinition {
    pub key: String,
    pub source: HealthSignalSource,
    pub path: String,
    pub operator: ComparisonOperator,
    pub value: Value,
    pub severity: HealthSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthSignalSource {
    Metric,
    ReportedState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareDefinition {
    pub strategy: FirmwareStrategy,
    #[serde(default)]
    pub compatibility: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirmwareStrategy {
    BinaryReplacement,
    PartitionSwap,
    Package,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipDefinition {
    pub key: String,
    pub stream: String,
    pub target_key_path: String,
    pub target_kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationDefinition {
    #[serde(default)]
    pub summary: Vec<MetricReference>,
    #[serde(default)]
    pub tabs: Vec<PresentationTab>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricReference {
    pub metric: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationTab {
    Overview,
    Telemetry,
    Commands,
    Configuration,
    Logs,
    Firmware,
    Relationships,
}
