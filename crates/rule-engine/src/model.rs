#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleTrigger {
    Telemetry,
    DeviceStatus,
    Geofence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleTargetType {
    Global,
    Blueprint,
    Fleet,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionField {
    Metric,
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
