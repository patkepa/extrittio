use super::model::{
    CompiledAction, CompiledCondition, ConditionField, ConditionOperator, RuleActionKind,
    RuleTargetType, RuleTrigger, TelemetryField,
};
use super::types::{CachedAction, CachedCondition};

pub fn compile_trigger(value: &str) -> Option<RuleTrigger> {
    match value {
        "telemetry" => Some(RuleTrigger::Telemetry),
        "device_status" => Some(RuleTrigger::DeviceStatus),
        "geofence" => Some(RuleTrigger::Geofence),
        _ => None,
    }
}

pub fn compile_target_type(value: &str) -> Option<RuleTargetType> {
    match value {
        "global" => Some(RuleTargetType::Global),
        "blueprint" => Some(RuleTargetType::Blueprint),
        "device_type" => Some(RuleTargetType::DeviceType),
        "fleet" => Some(RuleTargetType::Fleet),
        "device" => Some(RuleTargetType::Device),
        _ => None,
    }
}

pub fn compile_telemetry_field(value: &str) -> Option<TelemetryField> {
    match value {
        "temperature" => Some(TelemetryField::Temperature),
        "humidity" => Some(TelemetryField::Humidity),
        "battery_level" => Some(TelemetryField::BatteryLevel),
        "latitude" => Some(TelemetryField::Latitude),
        "longitude" => Some(TelemetryField::Longitude),
        "speed" => Some(TelemetryField::Speed),
        "altitude" => Some(TelemetryField::Altitude),
        "heading" => Some(TelemetryField::Heading),
        _ => None,
    }
}

pub fn compile_operator(value: &str) -> Option<ConditionOperator> {
    match value {
        "gt" => Some(ConditionOperator::Gt),
        "gte" => Some(ConditionOperator::Gte),
        "lt" => Some(ConditionOperator::Lt),
        "lte" => Some(ConditionOperator::Lte),
        "eq" => Some(ConditionOperator::Eq),
        "neq" => Some(ConditionOperator::Neq),
        _ => None,
    }
}

pub fn compile_action_kind(value: &str) -> Option<RuleActionKind> {
    match value {
        "alert" => Some(RuleActionKind::Alert),
        "webhook" => Some(RuleActionKind::Webhook),
        "command" => Some(RuleActionKind::Command),
        _ => None,
    }
}

pub fn compile_condition(condition: &CachedCondition) -> Option<CompiledCondition> {
    let field = if let Some(field) = compile_telemetry_field(&condition.field) {
        ConditionField::Telemetry(field)
    } else if condition.field == "status" {
        ConditionField::Status
    } else if condition.zone_id.is_some() {
        ConditionField::Zone
    } else {
        return None;
    };

    Some(CompiledCondition {
        field,
        operator: compile_operator(&condition.operator)?,
        value: condition.value.clone(),
        numeric_value: condition.value.parse::<f64>().ok(),
        zone_id: condition.zone_id.clone(),
    })
}

pub fn compile_action(action: &CachedAction) -> Option<CompiledAction> {
    Some(CompiledAction {
        kind: compile_action_kind(&action.action_type)?,
        config: action.config.clone(),
    })
}
