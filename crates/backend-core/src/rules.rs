use crate::rule_engine::types::{CachedZone, ZoneGeometry};
use crate::rule_snapshots::RuleSnapshotRecords;
use crate::{PersistenceError, TenantId, Zone};
use async_trait::async_trait;
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

#[async_trait]
pub trait RuleRepository: Send + Sync {
    /// Apply ordered legacy state only before a live location observation takes
    /// ownership of this rule/device pair. Replays and superseded updates no-op.
    async fn apply_legacy_zone_entry(
        &self,
        tenant: &TenantId,
        entry: crate::rule_snapshots::LegacyZoneEntry,
    ) -> Result<(), PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        filter: RuleFilter,
    ) -> Result<Vec<RuleDetails>, PersistenceError>;
    async fn get(
        &self,
        tenant: &TenantId,
        id: &str,
    ) -> Result<Option<RuleDetails>, PersistenceError>;
    async fn create(
        &self,
        tenant: &TenantId,
        record: NewRuleRecord,
    ) -> Result<RuleDetails, PersistenceError>;
    async fn update(
        &self,
        tenant: &TenantId,
        id: &str,
        record: UpdateRuleRecord,
    ) -> Result<Option<RuleDetails>, PersistenceError>;
    async fn delete(&self, tenant: &TenantId, id: &str) -> Result<bool, PersistenceError>;
    async fn toggle(
        &self,
        tenant: &TenantId,
        id: &str,
        enabled: bool,
        updated_at: NaiveDateTime,
    ) -> Result<Option<RuleDetails>, PersistenceError>;
    /// Loads enabled definitions and zones in one consistent system snapshot.
    async fn load_snapshot(&self) -> Result<RuleSnapshotRecords, PersistenceError>;
    async fn delete_stale_cooldowns(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError>;
}

pub fn merge_zone_snapshots(cache: &mut RuleSnapshotRecords, zones: Vec<Zone>) {
    for zone in zones {
        if let Some(geometry) = parse_zone_geometry(&zone.geometry_type, &zone.geometry_json) {
            cache.zones.insert(
                zone.id.clone(),
                CachedZone {
                    id: zone.id,
                    name: zone.name,
                    geometry,
                },
            );
        }
    }
}

fn parse_zone_geometry(geometry_type: &str, geometry_json: &Value) -> Option<ZoneGeometry> {
    match geometry_type {
        "circle" => {
            let center = geometry_json.get("center")?.as_array()?;
            if center.len() != 2 {
                return None;
            }
            Some(ZoneGeometry::Circle {
                center_lat: center[0].as_f64()?,
                center_lon: center[1].as_f64()?,
                radius_meters: geometry_json.get("radius_meters")?.as_f64()?,
            })
        }
        "polygon" => {
            let points = geometry_json.get("points")?.as_array()?;
            let points = points
                .iter()
                .map(|point| {
                    let coordinates = point.as_array()?;
                    (coordinates.len() == 2)
                        .then(|| Some((coordinates[0].as_f64()?, coordinates[1].as_f64()?)))?
                })
                .collect::<Option<Vec<_>>>()?;
            Some(ZoneGeometry::Polygon { points })
        }
        _ => None,
    }
}

/// Host-owned URL parsing and literal-address policy; DNS is validated again at dispatch.
pub trait WebhookUrlPolicy: Send + Sync {
    fn validate(&self, url: &str) -> Result<(), crate::ApplicationError>;
}

/// Non-blocking notification after a committed rule mutation. Reload failure
/// cannot turn a successful database operation into an API failure.
pub trait RuleChangeNotifier: Send + Sync {
    fn committed(&self);
}
