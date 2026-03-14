use chrono::NaiveDateTime;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Cached rule structures (in-memory, populated from DB)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CachedRule {
    pub id: String,
    pub name: String,
    pub trigger_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub cooldown_seconds: i32,
    pub conditions: Vec<CachedCondition>,
    pub actions: Vec<CachedAction>,
}

#[derive(Debug, Clone)]
pub struct CachedCondition {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct CachedAction {
    pub action_type: String,
    pub config: String,
}

// ---------------------------------------------------------------------------
// Pending actions produced by rule evaluation
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum PendingAction {
    CreateAlert {
        rule_id: String,
        device_id: String,
        severity: String,
        message: String,
        triggered_value: Option<String>,
    },
    UpdateAlertValue {
        alert_id: String,
        triggered_value: String,
    },
    ResolveAlert {
        alert_id: String,
    },
    SendWebhook {
        url: String,
        payload: Value,
    },
    SendCommand {
        device_id: String,
        command: String,
        params: Value,
    },
    UpdateCooldown {
        rule_id: String,
        device_id: String,
        fired_at: NaiveDateTime,
    },
}

// ---------------------------------------------------------------------------
// Input event types for rule evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TelemetryData {
    pub temperature: f32,
    pub humidity: f32,
    pub battery_level: f32,
}

#[derive(Debug, Clone)]
pub struct StatusChange {
    pub old_status: String,
    pub new_status: String,
}
