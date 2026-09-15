use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Cached rule structures (in-memory, populated from DB)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct CachedCondition {
    pub field: String,
    pub operator: String,
    pub value: String,
    pub zone_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
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
        tenant_id: String,
        alert_id: String,
        triggered_value: String,
    },
    ResolveAlert {
        tenant_id: String,
        alert_id: String,
    },
    SendWebhook {
        tenant_id: String,
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
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Contract-defined numeric metrics keyed by canonical stream field
    /// (for example `environment./temperature`). Semantic labels are not keys.
    pub metrics: std::collections::BTreeMap<String, crate::number::MetricNumber>,
}

#[derive(Debug, Clone)]
pub struct StatusChange {
    pub old_status: String,
    pub new_status: String,
}

// ---------------------------------------------------------------------------
// Zone geometry types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CachedZone {
    pub id: String,
    pub name: String,
    pub geometry: ZoneGeometry,
}

#[derive(Debug, Clone, Serialize)]
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
