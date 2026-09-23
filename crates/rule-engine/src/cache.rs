use std::collections::HashMap;

use chrono::NaiveDateTime;

use super::compiler::compile_target_type;
use super::model::RuleTargetType;
use super::types::{CachedRule, CachedZone};

pub type RuleDeviceKey = (String, String, String);

// ---------------------------------------------------------------------------
// RuleCache — in-memory rule index for fast evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct RuleCache {
    definitions: std::sync::Arc<RuleDefinitions>,
    rule_allowlist: Option<std::collections::HashSet<String>>,
    /// Active alerts keyed by (tenant_id, rule_id, device_id) → alert_id
    pub active_alerts: HashMap<RuleDeviceKey, String>,
    /// Cooldown timestamps keyed by (tenant_id, rule_id, device_id) → last_fired_at
    pub cooldowns: HashMap<RuleDeviceKey, NaiveDateTime>,
    /// Zone entry timestamps keyed by (tenant_id, rule_id, device_id) → entered_at
    pub zone_entry_times: HashMap<RuleDeviceKey, chrono::NaiveDateTime>,
    /// Monotonically increasing version bumped on every cache mutation
    pub version: u64,
}

/// Shared immutable definition index. Runtime hints are kept in `RuleCache`.
#[derive(Debug, Default, Clone)]
pub struct RuleDefinitions {
    /// Rules that apply to all devices (target_type = 'global')
    pub global_rules: Vec<CachedRule>,
    /// Rules scoped to a stable device blueprint across its revisions.
    pub by_blueprint: Vec<CachedRule>,
    /// Rules scoped to a specific fleet (target_type = 'fleet')
    pub by_fleet: Vec<CachedRule>,
    /// Rules scoped to a specific device (target_type = 'device')
    pub by_device: Vec<CachedRule>,
    /// Zone definitions keyed by zone_id
    pub zones: HashMap<String, CachedZone>,
}

impl std::ops::Deref for RuleCache {
    type Target = RuleDefinitions;
    fn deref(&self) -> &Self::Target {
        &self.definitions
    }
}

impl std::ops::DerefMut for RuleCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        std::sync::Arc::make_mut(&mut self.definitions)
    }
}

impl RuleCache {
    /// Restrict a transaction runtime view to rule identities still present in
    /// its database snapshot. Pure standalone callers retain unrestricted defaults.
    pub fn restrict_to_rule_ids(&mut self, ids: std::collections::HashSet<String>) {
        self.rule_allowlist = Some(ids);
    }

    /// Reuse the immutable index without copying or inheriting runtime hints.
    pub fn definitions_only(&self) -> Self {
        Self {
            definitions: self.definitions.clone(),
            ..Self::default()
        }
    }

    /// Returns all rules that could apply to the given tenant/device.
    pub fn rules_for_tenant_device(
        &self,
        tenant_id: &str,
        device_id: &str,
        fleet_id: Option<&str>,
        blueprint_id: Option<&str>,
    ) -> Vec<&CachedRule> {
        let mut applicable: Vec<&CachedRule> = Vec::new();

        for rule in &self.global_rules {
            if rule.tenant_id == tenant_id {
                applicable.push(rule);
            }
        }

        if let Some(blueprint_id) = blueprint_id {
            for rule in &self.by_blueprint {
                if rule.tenant_id == tenant_id && rule.target_id.as_deref() == Some(blueprint_id) {
                    applicable.push(rule);
                }
            }
        }

        if let Some(fid) = fleet_id {
            for rule in &self.by_fleet {
                if rule.tenant_id == tenant_id && rule.target_id.as_deref() == Some(fid) {
                    applicable.push(rule);
                }
            }
        }

        for rule in &self.by_device {
            if rule.tenant_id == tenant_id && rule.target_id.as_deref() == Some(device_id) {
                applicable.push(rule);
            }
        }

        if let Some(ids) = &self.rule_allowlist {
            applicable.retain(|rule| ids.contains(&rule.id));
        }
        applicable
    }

    /// Inserts a rule into the appropriate bucket based on its `target_type`.
    /// Replaces an existing rule with the same id if present; otherwise appends.
    pub fn insert_rule(&mut self, rule: CachedRule) {
        let bucket: &mut Vec<CachedRule> = match compile_target_type(&rule.target_type) {
            Some(RuleTargetType::Global) => &mut self.global_rules,
            Some(RuleTargetType::Blueprint) => &mut self.by_blueprint,
            Some(RuleTargetType::Fleet) => &mut self.by_fleet,
            Some(RuleTargetType::Device) => &mut self.by_device,
            None => return,
        };

        if let Some(existing) = bucket
            .iter_mut()
            .find(|r| r.tenant_id == rule.tenant_id && r.id == rule.id)
        {
            *existing = rule;
        } else {
            bucket.push(rule);
        }

        self.version = self.version.wrapping_add(1);
    }
}
