// Repository functions for rules, conditions, actions, and cooldowns

use chrono::NaiveDateTime;
use diesel::SqliteConnection;
use diesel::prelude::*;

use crate::db::models::{
    NewRule, NewRuleAction, NewRuleCondition, Rule, RuleAction, RuleCondition, RuleCooldown,
    UpdateRule,
};
use crate::db::schema::{rule_actions, rule_conditions, rule_cooldowns, rules};

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

pub fn list_rules(
    conn: &mut SqliteConnection,
    enabled: Option<bool>,
    trigger_type: Option<&str>,
    target_type: Option<&str>,
) -> Result<Vec<Rule>, diesel::result::Error> {
    let mut query = rules::table.into_boxed();

    if let Some(en) = enabled {
        query = query.filter(rules::enabled.eq(en));
    }
    if let Some(tt) = trigger_type {
        query = query.filter(rules::trigger_type.eq(tt));
    }
    if let Some(tgt) = target_type {
        query = query.filter(rules::target_type.eq(tgt));
    }

    query
        .order(rules::created_at.desc())
        .select(Rule::as_select())
        .load(conn)
}

pub fn find_rule(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Rule, diesel::result::Error> {
    rules::table
        .find(id)
        .select(Rule::as_select())
        .first(conn)
}

pub fn insert_rule(
    conn: &mut SqliteConnection,
    new_rule: &NewRule,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(rules::table)
        .values(new_rule)
        .execute(conn)?;
    Ok(())
}

pub fn update_rule(
    conn: &mut SqliteConnection,
    id: &str,
    changeset: &UpdateRule,
) -> Result<usize, diesel::result::Error> {
    diesel::update(rules::table.find(id))
        .set(changeset)
        .execute(conn)
}

pub fn delete_rule(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(rules::table.find(id)).execute(conn)
}

// ---------------------------------------------------------------------------
// Rule Conditions
// ---------------------------------------------------------------------------

pub fn list_conditions(
    conn: &mut SqliteConnection,
    rule_id: &str,
) -> Result<Vec<RuleCondition>, diesel::result::Error> {
    rule_conditions::table
        .filter(rule_conditions::rule_id.eq(rule_id))
        .select(RuleCondition::as_select())
        .load(conn)
}

pub fn insert_conditions(
    conn: &mut SqliteConnection,
    conditions: &[NewRuleCondition],
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(rule_conditions::table)
        .values(conditions)
        .execute(conn)?;
    Ok(())
}

pub fn delete_conditions_for_rule(
    conn: &mut SqliteConnection,
    rule_id: &str,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(rule_conditions::table.filter(rule_conditions::rule_id.eq(rule_id)))
        .execute(conn)
}

// ---------------------------------------------------------------------------
// Rule Actions
// ---------------------------------------------------------------------------

pub fn list_actions(
    conn: &mut SqliteConnection,
    rule_id: &str,
) -> Result<Vec<RuleAction>, diesel::result::Error> {
    rule_actions::table
        .filter(rule_actions::rule_id.eq(rule_id))
        .select(RuleAction::as_select())
        .load(conn)
}

pub fn insert_actions(
    conn: &mut SqliteConnection,
    actions: &[NewRuleAction],
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(rule_actions::table)
        .values(actions)
        .execute(conn)?;
    Ok(())
}

pub fn delete_actions_for_rule(
    conn: &mut SqliteConnection,
    rule_id: &str,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(rule_actions::table.filter(rule_actions::rule_id.eq(rule_id))).execute(conn)
}

// ---------------------------------------------------------------------------
// Bulk load for cache building
// ---------------------------------------------------------------------------

/// Attach conditions and actions to a pre-loaded set of rules using only two
/// additional queries (one for conditions, one for actions) regardless of rule
/// count. Returns `Vec<(Rule, Vec<RuleCondition>, Vec<RuleAction>)>`.
fn attach_conditions_and_actions(
    conn: &mut SqliteConnection,
    rules_vec: Vec<Rule>,
) -> Result<Vec<(Rule, Vec<RuleCondition>, Vec<RuleAction>)>, diesel::result::Error> {
    if rules_vec.is_empty() {
        return Ok(vec![]);
    }

    let ids: Vec<&str> = rules_vec.iter().map(|r| r.id.as_str()).collect();

    let all_conditions: Vec<RuleCondition> = rule_conditions::table
        .filter(rule_conditions::rule_id.eq_any(&ids))
        .select(RuleCondition::as_select())
        .load(conn)?;

    let all_actions: Vec<RuleAction> = rule_actions::table
        .filter(rule_actions::rule_id.eq_any(&ids))
        .select(RuleAction::as_select())
        .load(conn)?;

    // Group by rule_id using HashMaps for O(n) assembly instead of O(n*m).
    use std::collections::HashMap;

    let mut cond_map: HashMap<String, Vec<RuleCondition>> = HashMap::new();
    for c in all_conditions {
        cond_map.entry(c.rule_id.clone()).or_default().push(c);
    }

    let mut action_map: HashMap<String, Vec<RuleAction>> = HashMap::new();
    for a in all_actions {
        action_map.entry(a.rule_id.clone()).or_default().push(a);
    }

    let result = rules_vec
        .into_iter()
        .map(|rule| {
            let conditions = cond_map.remove(&rule.id).unwrap_or_default();
            let actions = action_map.remove(&rule.id).unwrap_or_default();
            (rule, conditions, actions)
        })
        .collect();

    Ok(result)
}

/// Load all enabled rules together with their conditions and actions in one
/// logical operation. Returns a vec of `(Rule, Vec<Condition>, Vec<Action>)`.
pub fn load_all_enabled_rules(
    conn: &mut SqliteConnection,
) -> Result<Vec<(Rule, Vec<RuleCondition>, Vec<RuleAction>)>, diesel::result::Error> {
    let enabled_rules: Vec<Rule> = rules::table
        .filter(rules::enabled.eq(true))
        .select(Rule::as_select())
        .load(conn)?;

    attach_conditions_and_actions(conn, enabled_rules)
}

/// Load filtered rules with their conditions and actions in batch (3 queries
/// total regardless of rule count).
pub fn load_rules_with_details(
    conn: &mut SqliteConnection,
    enabled: Option<bool>,
    trigger_type: Option<&str>,
    target_type: Option<&str>,
) -> Result<Vec<(Rule, Vec<RuleCondition>, Vec<RuleAction>)>, diesel::result::Error> {
    let filtered_rules = list_rules(conn, enabled, trigger_type, target_type)?;
    attach_conditions_and_actions(conn, filtered_rules)
}

// ---------------------------------------------------------------------------
// Cooldowns
// ---------------------------------------------------------------------------

pub fn load_all_cooldowns(
    conn: &mut SqliteConnection,
) -> Result<Vec<RuleCooldown>, diesel::result::Error> {
    rule_cooldowns::table
        .select(RuleCooldown::as_select())
        .load(conn)
}

/// Delete cooldown records whose `last_fired_at` is older than `cutoff`.
/// Returns the number of rows deleted.
pub fn delete_cooldowns_older_than(
    conn: &mut SqliteConnection,
    cutoff: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(rule_cooldowns::table.filter(rule_cooldowns::last_fired_at.lt(cutoff)))
        .execute(conn)
}

/// Insert or replace a cooldown record. Uses SQLite's `REPLACE INTO` semantics
/// (the composite PK is `(rule_id, device_id)`).
pub fn upsert_cooldown(
    conn: &mut SqliteConnection,
    cooldown: &RuleCooldown,
) -> Result<(), diesel::result::Error> {
    diesel::replace_into(rule_cooldowns::table)
        .values((
            rule_cooldowns::rule_id.eq(&cooldown.rule_id),
            rule_cooldowns::device_id.eq(&cooldown.device_id),
            rule_cooldowns::last_fired_at.eq(cooldown.last_fired_at),
        ))
        .execute(conn)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convenience wrapper used by the service when it needs a specific timestamp
/// formatted for update operations.
pub fn touch_rule_updated_at(
    conn: &mut SqliteConnection,
    id: &str,
    now: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::update(rules::table.find(id))
        .set(rules::updated_at.eq(now))
        .execute(conn)
}
