//! Immutable evaluation entry points. Host scheduling does not own rule decisions.
use crate::TenantId;
use crate::rule_engine::{
    cache::RuleCache,
    evaluate,
    types::{PendingAction, StatusChange, TelemetryData},
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RuleEvaluationSnapshot {
    cache: Arc<RuleCache>,
}
impl RuleEvaluationSnapshot {
    pub fn new(cache: Arc<RuleCache>) -> Self {
        Self { cache }
    }
}

/// Unindexed records loaded in one consistent database snapshot. Fingerprint
/// definitions before compilation so unchanged observations reuse the live index.
#[derive(Default)]
pub struct RuleSnapshotRecords {
    pub rules: Vec<crate::rule_engine::types::CachedRule>,
    pub zones: std::collections::HashMap<String, crate::rule_engine::types::CachedZone>,
}
impl RuleSnapshotRecords {
    pub fn canonical_definitions(&self) -> Result<Vec<u8>, serde_json::Error> {
        let zones: std::collections::BTreeMap<_, _> = self.zones.iter().collect();
        serde_json::to_vec(&(&self.rules, zones))
    }

    pub fn compile(self) -> RuleCache {
        let mut cache = RuleCache::default();
        for rule in self.rules {
            cache.insert_rule(rule);
        }
        cache.zones = self.zones;
        cache
    }
}

/// Device-scoped runtime rows read by an adapter while holding the device lock.
#[derive(Default)]
pub struct DeviceRuleRuntime {
    pub live_rule_ids: std::collections::HashSet<String>,
    pub active_alerts: std::collections::HashMap<crate::rule_engine::cache::RuleDeviceKey, String>,
    pub cooldowns:
        std::collections::HashMap<crate::rule_engine::cache::RuleDeviceKey, chrono::NaiveDateTime>,
    pub zone_entries:
        std::collections::HashMap<crate::rule_engine::cache::RuleDeviceKey, chrono::NaiveDateTime>,
}

/// Prepared immutable definitions/input; mutable runtime data is supplied only
/// after the ingress transaction has acquired its serialization lock.
#[derive(Debug, Clone)]
pub struct DeviceRuleEvaluation {
    pub snapshot: RuleEvaluationSnapshot,
    pub tenant: TenantId,
    pub device_id: String,
    pub fleet_id: Option<i32>,
    pub blueprint_id: Option<String>,
    pub input: RuleEvaluationInput,
    pub observed_at: chrono::NaiveDateTime,
}
#[derive(Debug, Clone)]
pub enum RuleEvaluationInput {
    Status(StatusChange),
    Telemetry { data: TelemetryData, geofence: bool },
}
pub struct ZoneEntryMutation {
    pub tenant_id: String,
    pub rule_id: String,
    pub device_id: String,
    pub entered_at: Option<chrono::NaiveDateTime>,
}
pub struct DeviceRuleDecision {
    pub deliveries: Vec<PendingAction>,
    pub cooldowns: Vec<crate::alerts::CooldownRecord>,
    pub zone_entries: Vec<ZoneEntryMutation>,
}
impl DeviceRuleEvaluation {
    /// Candidate definitions only; adapters confirm identities/eligibility under
    /// their transaction before asking core for any runtime mutations.
    pub fn candidate_rule_ids(&self) -> Vec<String> {
        let fleet = self.fleet_id.map(|id| id.to_string());
        self.snapshot
            .cache
            .rules_for_tenant_device(
                self.tenant.as_str(),
                &self.device_id,
                fleet.as_deref(),
                self.blueprint_id.as_deref(),
            )
            .into_iter()
            .map(|rule| rule.id.clone())
            .collect()
    }

    pub fn decide(&self, runtime: DeviceRuleRuntime) -> DeviceRuleDecision {
        let mut cache = self.snapshot.cache.definitions_only();
        cache.restrict_to_rule_ids(runtime.live_rule_ids);
        cache.active_alerts = runtime.active_alerts;
        cache.cooldowns = runtime.cooldowns;
        cache.zone_entry_times = runtime.zone_entries;
        let actions = match &self.input {
            RuleEvaluationInput::Status(change) => evaluate::evaluate_status_change_for_tenant_at(
                self.tenant.as_str(),
                &self.device_id,
                self.fleet_id,
                self.blueprint_id.as_deref(),
                change,
                &cache,
                self.observed_at,
            ),
            RuleEvaluationInput::Telemetry { data, geofence } => {
                let mut actions = evaluate::evaluate_telemetry_for_tenant_at(
                    self.tenant.as_str(),
                    &self.device_id,
                    self.fleet_id,
                    self.blueprint_id.as_deref(),
                    data,
                    &cache,
                    self.observed_at,
                );
                if *geofence {
                    actions.extend(evaluate::evaluate_geofence_for_tenant_at(
                        self.tenant.as_str(),
                        &self.device_id,
                        self.fleet_id,
                        self.blueprint_id.as_deref(),
                        data,
                        &cache,
                        self.observed_at,
                    ));
                }
                actions
            }
        };
        let mut decision = DeviceRuleDecision {
            deliveries: Vec::new(),
            cooldowns: Vec::new(),
            zone_entries: Vec::new(),
        };
        for action in actions {
            match action {
                PendingAction::UpdateCooldown {
                    tenant_id,
                    rule_id,
                    device_id,
                    fired_at,
                } => {
                    decision.cooldowns.push(crate::alerts::CooldownRecord {
                        tenant_id,
                        rule_id,
                        device_id,
                        last_fired_at: fired_at,
                    });
                }
                PendingAction::UpdateZoneEntry {
                    tenant_id,
                    rule_id,
                    device_id,
                    entered_at,
                } => {
                    decision.zone_entries.push(ZoneEntryMutation {
                        tenant_id,
                        rule_id,
                        device_id,
                        entered_at,
                    });
                }
                delivery => decision.deliveries.push(delivery),
            }
        }
        decision
    }
}

/// Host-owned definition acquisition. Core controls when decisions need a snapshot.
pub trait RuleSnapshotProvider: Send + Sync {
    fn snapshot(&self) -> Result<RuleEvaluationSnapshot, crate::ApplicationError>;
}
