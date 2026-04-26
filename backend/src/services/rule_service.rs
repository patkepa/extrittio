// Rule service — business logic for rule management and cache building

use chrono::Utc;
use diesel::PgConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::db::models::{
    NewRule, NewRuleAction, NewRuleCondition, Rule, RuleAction, RuleCondition, UpdateRule,
};
use crate::error::AppError;
use crate::repositories::{alert_repo, rule_repo, zone_repo};
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::types::{CachedAction, CachedCondition, CachedRule, CachedZone, ZoneGeometry};

// ---------------------------------------------------------------------------
// Public composite type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RuleWithDetails {
    pub rule: Rule,
    pub conditions: Vec<RuleCondition>,
    pub actions: Vec<RuleAction>,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

const TELEMETRY_FIELDS: &[&str] = &["temperature", "humidity", "battery_level"];
const TELEMETRY_NUMERIC_OPS: &[&str] = &["gt", "gte", "lt", "lte"];
const TELEMETRY_EQ_OPS: &[&str] = &["eq", "neq"];
const STATUS_FIELD: &str = "status";
const STATUS_VALUES: &[&str] = &["online", "offline", "warning"];
const TARGET_TYPES: &[&str] = &["global", "device_type", "fleet", "device"];
const TRIGGER_TYPES: &[&str] = &["telemetry", "device_status"];

fn validate_rule(
    name: &str,
    trigger_type: &str,
    target_type: &str,
    target_id: &Option<String>,
    cooldown_seconds: i32,
    conditions: &[(String, String, String)],
    actions: &[(String, Value)],
) -> Result<(), AppError> {
    // --- name ---
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if name.len() > 255 {
        return Err(AppError::BadRequest(
            "name must be at most 255 characters".into(),
        ));
    }

    // --- trigger_type ---
    if !TRIGGER_TYPES.contains(&trigger_type) {
        return Err(AppError::BadRequest(format!(
            "trigger_type must be one of: {}",
            TRIGGER_TYPES.join(", ")
        )));
    }

    // --- target_type ---
    if !TARGET_TYPES.contains(&target_type) {
        return Err(AppError::BadRequest(format!(
            "target_type must be one of: {}",
            TARGET_TYPES.join(", ")
        )));
    }

    // --- target_id ---
    match target_type {
        "global" => {
            if target_id.is_some() {
                return Err(AppError::BadRequest(
                    "target_id must be null for global rules".into(),
                ));
            }
        }
        _ => {
            if target_id.is_none() || target_id.as_deref().unwrap_or("").is_empty() {
                return Err(AppError::BadRequest(format!(
                    "target_id is required for target_type '{target_type}'"
                )));
            }
        }
    }

    // --- cooldown_seconds ---
    if cooldown_seconds < 0 || cooldown_seconds > 86400 {
        return Err(AppError::BadRequest(
            "cooldown_seconds must be between 0 and 86400".into(),
        ));
    }

    // --- conditions ---
    if conditions.is_empty() {
        return Err(AppError::BadRequest(
            "at least one condition is required".into(),
        ));
    }

    for (field, operator, value) in conditions {
        match trigger_type {
            "telemetry" => {
                if !TELEMETRY_FIELDS.contains(&field.as_str()) {
                    return Err(AppError::BadRequest(format!(
                        "telemetry condition field must be one of: {}",
                        TELEMETRY_FIELDS.join(", ")
                    )));
                }
                let all_ops: Vec<&str> = TELEMETRY_NUMERIC_OPS
                    .iter()
                    .chain(TELEMETRY_EQ_OPS.iter())
                    .copied()
                    .collect();
                if !all_ops.contains(&operator.as_str()) {
                    return Err(AppError::BadRequest(format!(
                        "telemetry operator must be one of: {}",
                        all_ops.join(", ")
                    )));
                }
                // gt/gte/lt/lte only for numeric — all telemetry fields are numeric here
                // value must parse as f32
                value.parse::<f32>().map_err(|_| {
                    AppError::BadRequest(format!(
                        "telemetry condition value '{}' is not a valid number",
                        value
                    ))
                })?;
            }
            "device_status" => {
                if field != STATUS_FIELD {
                    return Err(AppError::BadRequest(
                        "device_status condition field must be 'status'".into(),
                    ));
                }
                if !["eq", "neq"].contains(&operator.as_str()) {
                    return Err(AppError::BadRequest(
                        "device_status condition operator must be 'eq' or 'neq'".into(),
                    ));
                }
                if !STATUS_VALUES.contains(&value.as_str()) {
                    return Err(AppError::BadRequest(format!(
                        "device_status condition value must be one of: {}",
                        STATUS_VALUES.join(", ")
                    )));
                }
            }
            _ => unreachable!("trigger_type already validated"),
        }
    }

    // --- actions ---
    if actions.is_empty() {
        return Err(AppError::BadRequest(
            "at least one action is required".into(),
        ));
    }

    for (action_type, config_val) in actions {
        match action_type.as_str() {
            "webhook" => {
                let url = config_val.get("url").and_then(|v| v.as_str()).unwrap_or("");
                if url.is_empty() {
                    return Err(AppError::BadRequest(
                        "webhook action config must have a non-empty 'url'".into(),
                    ));
                }
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(AppError::BadRequest(
                        "webhook url must start with http:// or https://".into(),
                    ));
                }
            }
            "command" => {
                let cmd = config_val.get("command").and_then(|v| v.as_str()).unwrap_or("");
                if cmd.is_empty() {
                    return Err(AppError::BadRequest(
                        "command action config must have a non-empty 'command'".into(),
                    ));
                }
            }
            "alert" => {
                // Alert actions are valid without extra config constraints here;
                // the action's config JSON is stored as-is.
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "unknown action_type '{other}'"
                )));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public service functions
// ---------------------------------------------------------------------------

pub fn list_rules(
    conn: &mut PgConnection,
    enabled: Option<bool>,
    trigger_type: Option<&str>,
    target_type: Option<&str>,
) -> Result<Vec<Rule>, AppError> {
    Ok(rule_repo::list_rules(conn, enabled, trigger_type, target_type)?)
}

/// Load filtered rules together with their conditions and actions in batch
/// (3 queries total, regardless of rule count). This avoids the N+1 query
/// problem that occurs when fetching details for each rule individually.
pub fn list_rules_with_details(
    conn: &mut PgConnection,
    enabled: Option<bool>,
    trigger_type: Option<&str>,
    target_type: Option<&str>,
) -> Result<Vec<RuleWithDetails>, AppError> {
    let rows = rule_repo::load_rules_with_details(conn, enabled, trigger_type, target_type)?;
    Ok(rows
        .into_iter()
        .map(|(rule, conditions, actions)| RuleWithDetails {
            rule,
            conditions,
            actions,
        })
        .collect())
}

pub fn get_rule(conn: &mut PgConnection, id: &str) -> Result<RuleWithDetails, AppError> {
    let rule = rule_repo::find_rule(conn, id).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Rule '{id}' not found"))
        }
        other => AppError::Database(other),
    })?;
    let conditions = rule_repo::list_conditions(conn, id)?;
    let actions = rule_repo::list_actions(conn, id)?;
    Ok(RuleWithDetails {
        rule,
        conditions,
        actions,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn create_rule(
    conn: &mut PgConnection,
    name: &str,
    description: Option<String>,
    trigger_type: &str,
    target_type: &str,
    target_id: Option<String>,
    cooldown_seconds: i32,
    conditions: Vec<(String, String, String)>,
    actions: Vec<(String, Value)>,
) -> Result<RuleWithDetails, AppError> {
    validate_rule(
        name,
        trigger_type,
        target_type,
        &target_id,
        cooldown_seconds,
        &conditions,
        &actions,
    )?;

    let rule_id = Uuid::new_v4().to_string();

    let new_rule = NewRule {
        id: rule_id.clone(),
        name: name.trim().to_string(),
        description,
        enabled: true,
        trigger_type: trigger_type.to_string(),
        target_type: target_type.to_string(),
        target_id,
        cooldown_seconds,
    };

    let new_conditions: Vec<NewRuleCondition> = conditions
        .into_iter()
        .map(|(field, operator, value)| NewRuleCondition {
            id: Uuid::new_v4().to_string(),
            rule_id: rule_id.clone(),
            field,
            operator,
            value,
            condition_group: 0,
            zone_id: None,
        })
        .collect();

    let new_actions: Vec<NewRuleAction> = actions
        .into_iter()
        .map(|(action_type, config)| NewRuleAction {
            id: Uuid::new_v4().to_string(),
            rule_id: rule_id.clone(),
            action_type,
            config,
        })
        .collect();

    use diesel::Connection;
    conn.transaction(|conn| {
        rule_repo::insert_rule(conn, &new_rule)?;
        rule_repo::insert_conditions(conn, &new_conditions)?;
        rule_repo::insert_actions(conn, &new_actions)?;
        Ok::<(), diesel::result::Error>(())
    })?;

    get_rule(conn, &rule_id)
}

#[allow(clippy::too_many_arguments)]
pub fn update_rule(
    conn: &mut PgConnection,
    id: &str,
    name: Option<String>,
    description: Option<Option<String>>,
    trigger_type: Option<String>,
    target_type: Option<String>,
    target_id: Option<Option<String>>,
    cooldown_seconds: Option<i32>,
    conditions: Option<Vec<(String, String, String)>>,
    actions: Option<Vec<(String, Value)>>,
) -> Result<RuleWithDetails, AppError> {
    // Fetch current rule to fill in defaults for validation
    let current = rule_repo::find_rule(conn, id).map_err(|e| match e {
        diesel::result::Error::NotFound => AppError::NotFound(format!("Rule '{id}' not found")),
        other => AppError::Database(other),
    })?;

    // Resolve final values for validation
    let resolved_name = name.as_deref().unwrap_or(&current.name).to_string();
    let resolved_trigger = trigger_type
        .as_deref()
        .unwrap_or(&current.trigger_type)
        .to_string();
    let resolved_target_type = target_type
        .as_deref()
        .unwrap_or(&current.target_type)
        .to_string();
    let resolved_target_id = match &target_id {
        Some(inner) => inner.clone(),
        None => current.target_id.clone(),
    };
    let resolved_cooldown = cooldown_seconds.unwrap_or(current.cooldown_seconds);

    // For conditions/actions validation, load current ones if not replacing
    let current_conditions = rule_repo::list_conditions(conn, id)?;
    let current_actions = rule_repo::list_actions(conn, id)?;

    let validated_conditions: Vec<(String, String, String)> = match &conditions {
        Some(c) => c.clone(),
        None => current_conditions
            .iter()
            .map(|c| (c.field.clone(), c.operator.clone(), c.value.clone()))
            .collect(),
    };
    let validated_actions: Vec<(String, Value)> = match &actions {
        Some(a) => a.clone(),
        None => current_actions
            .iter()
            .map(|a| (a.action_type.clone(), a.config.clone()))
            .collect(),
    };

    validate_rule(
        &resolved_name,
        &resolved_trigger,
        &resolved_target_type,
        &resolved_target_id,
        resolved_cooldown,
        &validated_conditions,
        &validated_actions,
    )?;

    let now = Utc::now().naive_utc();
    let changeset = UpdateRule {
        name: name.map(|n| n.trim().to_string()),
        description,
        enabled: None,
        trigger_type,
        target_type,
        target_id,
        cooldown_seconds,
        updated_at: Some(now),
    };

    use diesel::Connection;
    conn.transaction(|conn| {
        rule_repo::update_rule(conn, id, &changeset)?;

        if let Some(new_conds) = conditions {
            rule_repo::delete_conditions_for_rule(conn, id)?;
            let new_conditions: Vec<NewRuleCondition> = new_conds
                .into_iter()
                .map(|(field, operator, value)| NewRuleCondition {
                    id: Uuid::new_v4().to_string(),
                    rule_id: id.to_string(),
                    field,
                    operator,
                    value,
                    condition_group: 0,
                    zone_id: None,
                })
                .collect();
            if !new_conditions.is_empty() {
                rule_repo::insert_conditions(conn, &new_conditions)?;
            }
        }

        if let Some(new_acts) = actions {
            rule_repo::delete_actions_for_rule(conn, id)?;
            let new_actions: Vec<NewRuleAction> = new_acts
                .into_iter()
                .map(|(action_type, config)| NewRuleAction {
                    id: Uuid::new_v4().to_string(),
                    rule_id: id.to_string(),
                    action_type,
                    config,
                })
                .collect();
            if !new_actions.is_empty() {
                rule_repo::insert_actions(conn, &new_actions)?;
            }
        }

        Ok::<(), diesel::result::Error>(())
    })?;

    get_rule(conn, id)
}

pub fn delete_rule(conn: &mut PgConnection, id: &str) -> Result<(), AppError> {
    let rows = rule_repo::delete_rule(conn, id)?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("Rule '{id}' not found")));
    }
    Ok(())
}

pub fn toggle_rule(
    conn: &mut PgConnection,
    id: &str,
    enabled: bool,
) -> Result<(), AppError> {
    let now = Utc::now().naive_utc();
    let changeset = UpdateRule {
        enabled: Some(enabled),
        updated_at: Some(now),
        ..Default::default()
    };
    let rows = rule_repo::update_rule(conn, id, &changeset)?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("Rule '{id}' not found")));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Retention / cleanup
// ---------------------------------------------------------------------------

/// Delete stale cooldown records older than `cutoff`.
pub fn delete_stale_cooldowns(
    conn: &mut PgConnection,
    cutoff: chrono::NaiveDateTime,
) -> Result<usize, AppError> {
    Ok(rule_repo::delete_cooldowns_older_than(conn, cutoff)?)
}

// ---------------------------------------------------------------------------
// Cache building
// ---------------------------------------------------------------------------

/// Build the in-memory RuleCache from the current database state.
pub fn build_cache(conn: &mut PgConnection) -> Result<RuleCache, AppError> {
    let enabled_rules = rule_repo::load_all_enabled_rules(conn)?;
    let cooldowns = rule_repo::load_all_cooldowns(conn)?;
    let active_alerts = alert_repo::load_active_alerts(conn)?;

    let mut cache = RuleCache::default();

    for (rule, conditions, actions) in enabled_rules {
        let cached = CachedRule {
            id: rule.id,
            name: rule.name,
            trigger_type: rule.trigger_type,
            target_type: rule.target_type,
            target_id: rule.target_id,
            cooldown_seconds: rule.cooldown_seconds,
            conditions: conditions
                .into_iter()
                .map(|c| CachedCondition {
                    field: c.field,
                    operator: c.operator,
                    value: c.value,
                    zone_id: c.zone_id.clone(),
                })
                .collect(),
            actions: actions
                .into_iter()
                .map(|a| CachedAction {
                    action_type: a.action_type,
                    config: a.config,
                })
                .collect(),
        };
        cache.insert_rule(cached);
    }

    for cooldown in cooldowns {
        cache.cooldowns.insert(
            (cooldown.rule_id, cooldown.device_id),
            cooldown.last_fired_at,
        );
    }

    for alert in active_alerts {
        if let Some(rule_id) = alert.rule_id {
            cache
                .active_alerts
                .insert((rule_id, alert.device_id), alert.id);
        }
    }

    let zones = zone_repo::list_zones(conn)?;
    for zone in zones {
        if let Ok(geometry) = parse_zone_geometry(&zone.geometry_type, &zone.geometry_json) {
            cache.zones.insert(zone.id.clone(), CachedZone {
                id: zone.id,
                name: zone.name,
                geometry,
            });
        }
    }

    Ok(cache)
}

fn parse_zone_geometry(geometry_type: &str, geometry_json: &Value) -> Result<ZoneGeometry, String> {
    match geometry_type {
        "circle" => {
            let center = geometry_json["center"].as_array().ok_or("Missing center")?;
            Ok(ZoneGeometry::Circle {
                center_lat: center[0].as_f64().unwrap_or(0.0),
                center_lon: center[1].as_f64().unwrap_or(0.0),
                radius_meters: geometry_json["radius_meters"].as_f64().unwrap_or(0.0),
            })
        }
        "polygon" => {
            let points = geometry_json["points"].as_array().ok_or("Missing points")?;
            Ok(ZoneGeometry::Polygon {
                points: points.iter().map(|p| {
                    let a = p.as_array().unwrap();
                    (a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0))
                }).collect(),
            })
        }
        _ => Err(format!("Unknown geometry type: {}", geometry_type)),
    }
}
