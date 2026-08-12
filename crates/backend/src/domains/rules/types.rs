use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct RuleRecord {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub trigger_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub cooldown_seconds: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RuleConditionRecord {
    pub id: String,
    pub field: String,
    pub operator: String,
    pub value: String,
    pub condition_group: i32,
    pub zone_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuleActionRecord {
    pub id: String,
    pub action_type: String,
    pub config: Value,
}

#[derive(Debug, Clone)]
pub struct RuleDetails {
    pub rule: RuleRecord,
    pub conditions: Vec<RuleConditionRecord>,
    pub actions: Vec<RuleActionRecord>,
}

#[derive(Debug, Clone)]
pub struct RuleFilter {
    pub enabled: Option<bool>,
    pub trigger_type: Option<String>,
    pub target_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewRuleRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub cooldown_seconds: i32,
    pub conditions: Vec<RuleConditionRecord>,
    pub actions: Vec<RuleActionRecord>,
}

#[derive(Debug, Clone)]
pub struct UpdateRuleRecord {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub trigger_type: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<Option<String>>,
    pub cooldown_seconds: Option<i32>,
    pub conditions: Option<Vec<RuleConditionRecord>>,
    pub actions: Option<Vec<RuleActionRecord>>,
    pub updated_at: NaiveDateTime,
}
