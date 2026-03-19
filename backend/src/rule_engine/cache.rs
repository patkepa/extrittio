use std::collections::HashMap;

use chrono::NaiveDateTime;

use super::types::{CachedRule, CachedZone};

// ---------------------------------------------------------------------------
// RuleCache — in-memory rule index for fast evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct RuleCache {
    /// Rules that apply to all devices (target_type = 'global')
    pub global_rules: Vec<CachedRule>,
    /// Rules scoped to a specific device type (target_type = 'device_type')
    pub by_device_type: Vec<CachedRule>,
    /// Rules scoped to a specific fleet (target_type = 'fleet')
    pub by_fleet: Vec<CachedRule>,
    /// Rules scoped to a specific device (target_type = 'device')
    pub by_device: Vec<CachedRule>,
    /// Active alerts keyed by (rule_id, device_id) → alert_id
    pub active_alerts: HashMap<(String, String), String>,
    /// Cooldown timestamps keyed by (rule_id, device_id) → last_fired_at
    pub cooldowns: HashMap<(String, String), NaiveDateTime>,
    /// Zone definitions keyed by zone_id
    pub zones: HashMap<String, CachedZone>,
    /// Zone entry timestamps keyed by (rule_id, device_id) → entered_at
    pub zone_entry_times: HashMap<(String, String), chrono::NaiveDateTime>,
    /// Monotonically increasing version bumped on every cache mutation
    pub version: u64,
}

impl RuleCache {
    /// Returns all rules that could apply to the given device, combining
    /// global rules with any that match by `device_type_id`, `fleet_id`, or
    /// `device_id`.
    pub fn rules_for_device(
        &self,
        device_id: &str,
        device_type_id: &str,
        fleet_id: Option<&str>,
    ) -> Vec<&CachedRule> {
        let mut applicable: Vec<&CachedRule> = Vec::new();

        for rule in &self.global_rules {
            applicable.push(rule);
        }

        for rule in &self.by_device_type {
            if rule.target_id.as_deref() == Some(device_type_id) {
                applicable.push(rule);
            }
        }

        if let Some(fid) = fleet_id {
            for rule in &self.by_fleet {
                if rule.target_id.as_deref() == Some(fid) {
                    applicable.push(rule);
                }
            }
        }

        for rule in &self.by_device {
            if rule.target_id.as_deref() == Some(device_id) {
                applicable.push(rule);
            }
        }

        applicable
    }

    /// Inserts a rule into the appropriate bucket based on its `target_type`.
    /// Replaces an existing rule with the same id if present; otherwise appends.
    pub fn insert_rule(&mut self, rule: CachedRule) {
        let bucket: &mut Vec<CachedRule> = match rule.target_type.as_str() {
            "global" => &mut self.global_rules,
            "device_type" => &mut self.by_device_type,
            "fleet" => &mut self.by_fleet,
            "device" => &mut self.by_device,
            _ => return,
        };

        if let Some(existing) = bucket.iter_mut().find(|r| r.id == rule.id) {
            *existing = rule;
        } else {
            bucket.push(rule);
        }

        self.version = self.version.wrapping_add(1);
    }
}
