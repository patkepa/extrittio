use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Cached rule structures (in-memory, populated from DB)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CachedRule {
    pub tenant_id: String,
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
    pub zone_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CachedAction {
    pub action_type: String,
    pub config: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Pending actions produced by rule evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PendingAction {
    CreateAlert {
        tenant_id: String,
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
        headers: std::collections::HashMap<String, String>,
        payload: Value,
    },
    SendCommand {
        tenant_id: String,
        device_id: String,
        command: String,
        params: Value,
    },
    UpdateCooldown {
        tenant_id: String,
        rule_id: String,
        device_id: String,
        fired_at: NaiveDateTime,
    },
    UpdateZoneEntry {
        tenant_id: String,
        rule_id: String,
        device_id: String,
        entered_at: Option<chrono::NaiveDateTime>,
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
    pub latitude: f64,
    pub longitude: f64,
    pub speed: f32,
    pub altitude: f32,
    pub heading: f32,
}

#[derive(Debug, Clone)]
pub struct StatusChange {
    pub old_status: String,
    pub new_status: String,
}

// ---------------------------------------------------------------------------
// Zone geometry types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CachedZone {
    pub id: String,
    pub name: String,
    pub geometry: ZoneGeometry,
}

#[derive(Debug, Clone)]
pub enum ZoneGeometry {
    Circle {
        center_lat: f64,
        center_lon: f64,
        radius_meters: f64,
    },
    Polygon {
        points: Vec<(f64, f64)>,
    },
}
