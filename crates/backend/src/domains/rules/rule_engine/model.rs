#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleTrigger {
    Telemetry,
    DeviceStatus,
    Geofence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleTargetType {
    Global,
    DeviceType,
    Fleet,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryField {
    Temperature,
    Humidity,
    BatteryLevel,
    Latitude,
    Longitude,
    Speed,
    Altitude,
    Heading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionField {
    Telemetry(TelemetryField),
    Status,
    Zone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionOperator {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleActionKind {
    Alert,
    Webhook,
    Command,
}

#[derive(Debug, Clone)]
pub struct CompiledCondition {
    pub field: ConditionField,
    pub operator: ConditionOperator,
    pub value: String,
    pub numeric_value: Option<f64>,
    pub zone_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompiledAction {
    pub kind: RuleActionKind,
    pub config: serde_json::Value,
}
