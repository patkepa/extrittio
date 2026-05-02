use chrono::Utc;
use serde_json::{Value, json};

use super::cache::RuleCache;
use super::geo::{point_in_circle, point_in_polygon};
use super::types::{CachedCondition, PendingAction, StatusChange, TelemetryData, ZoneGeometry};
use crate::tenancy::DEFAULT_TENANT_ID;

// ---------------------------------------------------------------------------
// Field extraction
// ---------------------------------------------------------------------------

/// Maps a field name string to the corresponding value in `TelemetryData`.
/// Returns `None` for unknown field names.
pub fn get_field_value(field: &str, data: &TelemetryData) -> Option<f64> {
    match field {
        "temperature" => Some(data.temperature as f64),
        "humidity" => Some(data.humidity as f64),
        "battery_level" => Some(data.battery_level as f64),
        "latitude" => Some(data.latitude),
        "longitude" => Some(data.longitude),
        "speed" => Some(data.speed as f64),
        "altitude" => Some(data.altitude as f64),
        "heading" => Some(data.heading as f64),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Condition evaluation
// ---------------------------------------------------------------------------

/// Evaluates a single condition against telemetry data.
/// Returns `false` if the field is unknown or the threshold value cannot be
/// parsed as f64.
pub fn evaluate_condition(condition: &CachedCondition, data: &TelemetryData) -> bool {
    let field_val = match get_field_value(&condition.field, data) {
        Some(v) => v,
        None => return false,
    };

    let threshold: f64 = match condition.value.parse() {
        Ok(v) => v,
        Err(_) => return false,
    };

    match condition.operator.as_str() {
        "gt" => field_val > threshold,
        "gte" => field_val >= threshold,
        "lt" => field_val < threshold,
        "lte" => field_val <= threshold,
        "eq" => (field_val - threshold).abs() < f64::EPSILON,
        "neq" => (field_val - threshold).abs() >= f64::EPSILON,
        _ => false,
    }
}

/// Evaluates a single condition against a status string.
/// Only `eq` and `neq` operators are meaningful for status comparisons.
pub fn evaluate_status_condition(condition: &CachedCondition, new_status: &str) -> bool {
    match condition.operator.as_str() {
        "eq" => condition.value == new_status,
        "neq" => condition.value != new_status,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Cooldown check
// ---------------------------------------------------------------------------

/// Returns `true` if the given rule+device combination is currently within
/// its cooldown window.  A `cooldown_seconds` value of 0 means no cooldown
/// (always returns `false`).
pub fn is_in_cooldown(
    cache: &RuleCache,
    rule_id: &str,
    device_id: &str,
    cooldown_seconds: i32,
) -> bool {
    is_in_cooldown_for_tenant(
        cache,
        DEFAULT_TENANT_ID,
        rule_id,
        device_id,
        cooldown_seconds,
    )
}

pub fn is_in_cooldown_for_tenant(
    cache: &RuleCache,
    tenant_id: &str,
    rule_id: &str,
    device_id: &str,
    cooldown_seconds: i32,
) -> bool {
    if cooldown_seconds <= 0 {
        return false;
    }

    let key = (
        tenant_id.to_string(),
        rule_id.to_string(),
        device_id.to_string(),
    );
    if let Some(last_fired) = cache.cooldowns.get(&key) {
        let now = Utc::now().naive_utc();
        let elapsed = now.signed_duration_since(*last_fired);
        elapsed.num_seconds() < cooldown_seconds as i64
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Message formatting helpers
// ---------------------------------------------------------------------------

/// Formats an f64 value as a string, stripping unnecessary trailing zeros.
/// Uses 4 decimal places of precision to avoid f32-to-f64 cast noise.
/// e.g. 85.2 → "85.2",  95.0 → "95"
pub fn format_value(val: f64) -> String {
    if val.fract() == 0.0 {
        format!("{:.0}", val)
    } else {
        // Format with 4 decimal places to handle f32-to-f64 precision noise
        // (e.g. 85.2_f32 as f64 = 85.19999... rounds to "85.2000" → "85.2")
        let s = format!("{:.4}", val);
        // Strip trailing zeros after decimal point
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

/// Converts an operator string to a human-readable verb phrase.
fn operator_phrase(operator: &str) -> &'static str {
    match operator {
        "gt" | "gte" => "exceeded",
        "lt" | "lte" => "dropped below",
        "eq" => "equals",
        "neq" => "is not",
        _ => "triggered",
    }
}

/// Builds an alert message from a slice of conditions and the current
/// telemetry data.  Conditions are joined with " AND ".
/// Example: "temperature (85.2) exceeded 80"
pub fn build_alert_message(conditions: &[CachedCondition], data: &TelemetryData) -> String {
    let parts: Vec<String> = conditions
        .iter()
        .map(|c| {
            let phrase = operator_phrase(&c.operator);
            if let Some(val) = get_field_value(&c.field, data) {
                format!("{} ({}) {} {}", c.field, format_value(val), phrase, c.value)
            } else {
                format!("{} {} {}", c.field, phrase, c.value)
            }
        })
        .collect();
    parts.join(" AND ")
}

/// Builds a status-change alert message.
/// Example: "status changed to offline"
pub fn build_status_alert_message(new_status: &str) -> String {
    format!("status changed to {}", new_status)
}

// ---------------------------------------------------------------------------
// Main evaluation helpers
// ---------------------------------------------------------------------------

/// Returns the highest f32 value among conditions that actually matched, as a
/// formatted string — used as `triggered_value` for telemetry alerts.
fn triggered_value_for(conditions: &[CachedCondition], data: &TelemetryData) -> Option<String> {
    // Use the first matched condition's field value as the triggered value.
    conditions
        .first()
        .and_then(|c| get_field_value(&c.field, data))
        .map(format_value)
}

// ---------------------------------------------------------------------------
// evaluate_telemetry
// ---------------------------------------------------------------------------

/// Evaluates all applicable rules against telemetry data and returns the list
/// of `PendingAction`s that should be executed.
///
/// Rules are selected via `RuleCache::rules_for_device`.  For each rule:
/// - ALL conditions must be satisfied (AND logic).
/// - If conditions are met and no active alert exists → `CreateAlert` + actions.
/// - If conditions are met and an active alert exists → `UpdateAlertValue`.
/// - If conditions are NOT met but an active alert exists → `ResolveAlert`.
/// - After firing, an `UpdateCooldown` is appended.
pub fn evaluate_telemetry(
    device_id: &str,
    device_type_id: i32,
    fleet_id: Option<i32>,
    data: &TelemetryData,
    cache: &RuleCache,
) -> Vec<PendingAction> {
    evaluate_telemetry_for_tenant(
        DEFAULT_TENANT_ID,
        device_id,
        device_type_id,
        fleet_id,
        data,
        cache,
    )
}

pub fn evaluate_telemetry_for_tenant(
    tenant_id: &str,
    device_id: &str,
    device_type_id: i32,
    fleet_id: Option<i32>,
    data: &TelemetryData,
    cache: &RuleCache,
) -> Vec<PendingAction> {
    let device_type_str = device_type_id.to_string();
    let fleet_str = fleet_id.map(|f| f.to_string());
    let fleet_ref = fleet_str.as_deref();

    let rules = cache.rules_for_tenant_device(tenant_id, device_id, &device_type_str, fleet_ref);

    let mut actions: Vec<PendingAction> = Vec::new();

    for rule in rules {
        // Only handle telemetry-triggered rules here.
        if rule.trigger_type != "telemetry" {
            continue;
        }

        let conditions_met = rule.conditions.iter().all(|c| evaluate_condition(c, data));

        let alert_key = (
            rule.tenant_id.clone(),
            rule.id.clone(),
            device_id.to_string(),
        );
        let existing_alert_id = cache.active_alerts.get(&alert_key).cloned();

        if conditions_met {
            // Check cooldown — skip firing new alert/actions if in cooldown,
            // but still update an existing alert value.
            if let Some(ref alert_id) = existing_alert_id {
                // Skip empty-string sentinel (reservation in-flight).
                if !alert_id.is_empty() {
                    actions.push(PendingAction::UpdateAlertValue {
                        alert_id: alert_id.clone(),
                        triggered_value: triggered_value_for(&rule.conditions, data)
                            .unwrap_or_default(),
                    });
                }
            } else {
                // Not yet active. Respect cooldown before creating.
                if is_in_cooldown_for_tenant(
                    cache,
                    &rule.tenant_id,
                    &rule.id,
                    device_id,
                    rule.cooldown_seconds,
                ) {
                    continue;
                }

                // Produce one action per rule action entry.
                for rule_action in &rule.actions {
                    match rule_action.action_type.as_str() {
                        "alert" => {
                            let config = &rule_action.config;
                            let severity = config
                                .get("severity")
                                .and_then(|v| v.as_str())
                                .unwrap_or("warning")
                                .to_string();
                            let message = build_alert_message(&rule.conditions, data);
                            let triggered_value = triggered_value_for(&rule.conditions, data);
                            actions.push(PendingAction::CreateAlert {
                                tenant_id: rule.tenant_id.clone(),
                                rule_id: rule.id.clone(),
                                device_id: device_id.to_string(),
                                severity,
                                message,
                                triggered_value,
                            });
                        }
                        "webhook" => {
                            let config = &rule_action.config;
                            let url = config
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let payload = json!({
                                "event": "rule_triggered",
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                                "rule": {
                                    "id": rule.id,
                                    "name": rule.name,
                                },
                                "device": {
                                    "id": device_id,
                                },
                                "trigger": "telemetry",
                                "triggered_values": {
                                    "temperature": data.temperature,
                                    "humidity": data.humidity,
                                    "battery_level": data.battery_level,
                                },
                            });
                            let mut headers = std::collections::HashMap::new();
                            headers.insert(
                                "X-Extrittio-Event".to_string(),
                                "rule_triggered".to_string(),
                            );
                            actions.push(PendingAction::SendWebhook {
                                url,
                                headers,
                                payload,
                            });
                        }
                        "command" => {
                            let config = &rule_action.config;
                            let command = config
                                .get("command")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let params = config.get("params").cloned().unwrap_or(Value::Null);
                            actions.push(PendingAction::SendCommand {
                                tenant_id: rule.tenant_id.clone(),
                                device_id: device_id.to_string(),
                                command,
                                params,
                            });
                        }
                        _ => {}
                    }
                }

                // Record cooldown update.
                actions.push(PendingAction::UpdateCooldown {
                    tenant_id: rule.tenant_id.clone(),
                    rule_id: rule.id.clone(),
                    device_id: device_id.to_string(),
                    fired_at: Utc::now().naive_utc(),
                });
            }
        } else {
            // Conditions not met — auto-resolve if an active alert exists.
            // Skip empty-string sentinels (reservation in-flight).
            if let Some(alert_id) = existing_alert_id {
                if !alert_id.is_empty() {
                    actions.push(PendingAction::ResolveAlert { alert_id });
                }
            }
        }
    }

    actions
}

// ---------------------------------------------------------------------------
// evaluate_status_change
// ---------------------------------------------------------------------------

/// Evaluates all applicable rules against a device status change and returns
/// the list of `PendingAction`s that should be executed.
pub fn evaluate_status_change(
    device_id: &str,
    device_type_id: i32,
    fleet_id: Option<i32>,
    change: &StatusChange,
    cache: &RuleCache,
) -> Vec<PendingAction> {
    evaluate_status_change_for_tenant(
        DEFAULT_TENANT_ID,
        device_id,
        device_type_id,
        fleet_id,
        change,
        cache,
    )
}

pub fn evaluate_status_change_for_tenant(
    tenant_id: &str,
    device_id: &str,
    device_type_id: i32,
    fleet_id: Option<i32>,
    change: &StatusChange,
    cache: &RuleCache,
) -> Vec<PendingAction> {
    let device_type_str = device_type_id.to_string();
    let fleet_str = fleet_id.map(|f| f.to_string());
    let fleet_ref = fleet_str.as_deref();

    let rules = cache.rules_for_tenant_device(tenant_id, device_id, &device_type_str, fleet_ref);

    let mut actions: Vec<PendingAction> = Vec::new();

    for rule in rules {
        // Only handle status-triggered rules here.
        if rule.trigger_type != "device_status" {
            continue;
        }

        let conditions_met = rule
            .conditions
            .iter()
            .all(|c| evaluate_status_condition(c, &change.new_status));

        let alert_key = (
            rule.tenant_id.clone(),
            rule.id.clone(),
            device_id.to_string(),
        );
        let existing_alert_id = cache.active_alerts.get(&alert_key).cloned();

        if conditions_met {
            if let Some(ref alert_id) = existing_alert_id {
                // Skip empty-string sentinel (reservation in-flight).
                if !alert_id.is_empty() {
                    actions.push(PendingAction::UpdateAlertValue {
                        alert_id: alert_id.clone(),
                        triggered_value: change.new_status.clone(),
                    });
                }
            } else {
                // Respect cooldown.
                if is_in_cooldown_for_tenant(
                    cache,
                    &rule.tenant_id,
                    &rule.id,
                    device_id,
                    rule.cooldown_seconds,
                ) {
                    continue;
                }

                for rule_action in &rule.actions {
                    match rule_action.action_type.as_str() {
                        "alert" => {
                            let config = &rule_action.config;
                            let severity = config
                                .get("severity")
                                .and_then(|v| v.as_str())
                                .unwrap_or("warning")
                                .to_string();
                            let message = build_status_alert_message(&change.new_status);
                            actions.push(PendingAction::CreateAlert {
                                tenant_id: rule.tenant_id.clone(),
                                rule_id: rule.id.clone(),
                                device_id: device_id.to_string(),
                                severity,
                                message,
                                triggered_value: Some(change.new_status.clone()),
                            });
                        }
                        "webhook" => {
                            let config = &rule_action.config;
                            let url = config
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let payload = json!({
                                "event": "rule_triggered",
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                                "rule": {
                                    "id": rule.id,
                                    "name": rule.name,
                                },
                                "device": {
                                    "id": device_id,
                                },
                                "trigger": "device_status",
                                "triggered_values": {
                                    "status": change.new_status,
                                },
                            });
                            let mut headers = std::collections::HashMap::new();
                            headers.insert(
                                "X-Extrittio-Event".to_string(),
                                "rule_triggered".to_string(),
                            );
                            actions.push(PendingAction::SendWebhook {
                                url,
                                headers,
                                payload,
                            });
                        }
                        "command" => {
                            let config = &rule_action.config;
                            let command = config
                                .get("command")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let params = config.get("params").cloned().unwrap_or(Value::Null);
                            actions.push(PendingAction::SendCommand {
                                tenant_id: rule.tenant_id.clone(),
                                device_id: device_id.to_string(),
                                command,
                                params,
                            });
                        }
                        _ => {}
                    }
                }

                actions.push(PendingAction::UpdateCooldown {
                    tenant_id: rule.tenant_id.clone(),
                    rule_id: rule.id.clone(),
                    device_id: device_id.to_string(),
                    fired_at: Utc::now().naive_utc(),
                });
            }
        } else {
            // Auto-resolve if an active alert exists.
            // Skip empty-string sentinels (reservation in-flight).
            if let Some(alert_id) = existing_alert_id {
                if !alert_id.is_empty() {
                    actions.push(PendingAction::ResolveAlert { alert_id });
                }
            }
        }
    }

    actions
}

// ---------------------------------------------------------------------------
// evaluate_geofence
// ---------------------------------------------------------------------------

/// Checks whether `(lat, lon)` is inside a cached zone.
fn point_in_zone(lat: f64, lon: f64, geometry: &ZoneGeometry) -> bool {
    match geometry {
        ZoneGeometry::Circle {
            center_lat,
            center_lon,
            radius_meters,
        } => point_in_circle(lat, lon, *center_lat, *center_lon, *radius_meters),
        ZoneGeometry::Polygon { points } => point_in_polygon(lat, lon, points),
    }
}

/// Evaluates geofence rules against the current device location.
///
/// For each rule with `trigger_type == "geofence"`, checks each condition's
/// `zone_id` against the cached zone geometries.  Produces `PendingAction`s
/// for zone entry/exit events.  Zone dwell mutations are returned as
/// `UpdateZoneEntry` (deferred to the caller).
pub fn evaluate_geofence(
    device_id: &str,
    device_type_id: i32,
    fleet_id: Option<i32>,
    data: &TelemetryData,
    cache: &RuleCache,
) -> Vec<PendingAction> {
    evaluate_geofence_for_tenant(
        DEFAULT_TENANT_ID,
        device_id,
        device_type_id,
        fleet_id,
        data,
        cache,
    )
}

pub fn evaluate_geofence_for_tenant(
    tenant_id: &str,
    device_id: &str,
    device_type_id: i32,
    fleet_id: Option<i32>,
    data: &TelemetryData,
    cache: &RuleCache,
) -> Vec<PendingAction> {
    // Skip evaluation if no location data provided.
    if data.latitude == 0.0 && data.longitude == 0.0 {
        return Vec::new();
    }

    let device_type_str = device_type_id.to_string();
    let fleet_str = fleet_id.map(|f| f.to_string());
    let fleet_ref = fleet_str.as_deref();

    let rules = cache.rules_for_tenant_device(tenant_id, device_id, &device_type_str, fleet_ref);

    let mut actions: Vec<PendingAction> = Vec::new();

    for rule in rules {
        if rule.trigger_type != "geofence" {
            continue;
        }

        // Determine if the device is currently inside ALL required zones
        // (conditions are AND-ed; each condition refers to a zone via zone_id).
        let conditions_met = rule.conditions.iter().all(|c| {
            if let Some(ref zid) = c.zone_id {
                if let Some(zone) = cache.zones.get(zid) {
                    return point_in_zone(data.latitude, data.longitude, &zone.geometry);
                }
            }
            false
        });

        let alert_key = (
            rule.tenant_id.clone(),
            rule.id.clone(),
            device_id.to_string(),
        );
        let existing_alert_id = cache.active_alerts.get(&alert_key).cloned();
        let zone_key = (
            rule.tenant_id.clone(),
            rule.id.clone(),
            device_id.to_string(),
        );
        let was_inside = cache.zone_entry_times.contains_key(&zone_key);

        if conditions_met {
            // Device is inside the zone(s).
            if !was_inside {
                // Just entered — record entry time.
                actions.push(PendingAction::UpdateZoneEntry {
                    tenant_id: rule.tenant_id.clone(),
                    rule_id: rule.id.clone(),
                    device_id: device_id.to_string(),
                    entered_at: Some(Utc::now().naive_utc()),
                });
            }

            if let Some(ref alert_id) = existing_alert_id {
                // Skip empty-string sentinel (reservation in-flight).
                if !alert_id.is_empty() {
                    actions.push(PendingAction::UpdateAlertValue {
                        alert_id: alert_id.clone(),
                        triggered_value: format!("{},{}", data.latitude, data.longitude),
                    });
                }
            } else {
                if is_in_cooldown_for_tenant(
                    cache,
                    &rule.tenant_id,
                    &rule.id,
                    device_id,
                    rule.cooldown_seconds,
                ) {
                    continue;
                }

                for rule_action in &rule.actions {
                    match rule_action.action_type.as_str() {
                        "alert" => {
                            let config = &rule_action.config;
                            let severity = config
                                .get("severity")
                                .and_then(|v| v.as_str())
                                .unwrap_or("warning")
                                .to_string();
                            let zone_name = rule
                                .conditions
                                .first()
                                .and_then(|c| c.zone_id.as_ref())
                                .and_then(|zid| cache.zones.get(zid))
                                .map(|z| z.name.as_str())
                                .unwrap_or("unknown zone");
                            let message = format!(
                                "device entered zone '{}' at {:.6},{:.6}",
                                zone_name, data.latitude, data.longitude
                            );
                            actions.push(PendingAction::CreateAlert {
                                tenant_id: rule.tenant_id.clone(),
                                rule_id: rule.id.clone(),
                                device_id: device_id.to_string(),
                                severity,
                                message,
                                triggered_value: Some(format!(
                                    "{},{}",
                                    data.latitude, data.longitude
                                )),
                            });
                        }
                        "webhook" => {
                            let config = &rule_action.config;
                            let url = config
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let payload = json!({
                                "event": "geofence_entered",
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                                "rule": {
                                    "id": rule.id,
                                    "name": rule.name,
                                },
                                "device": {
                                    "id": device_id,
                                },
                                "location": {
                                    "latitude": data.latitude,
                                    "longitude": data.longitude,
                                },
                            });
                            let mut headers = std::collections::HashMap::new();
                            headers.insert(
                                "X-Extrittio-Event".to_string(),
                                "geofence_entered".to_string(),
                            );
                            actions.push(PendingAction::SendWebhook {
                                url,
                                headers,
                                payload,
                            });
                        }
                        "command" => {
                            let config = &rule_action.config;
                            let command = config
                                .get("command")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let params = config.get("params").cloned().unwrap_or(Value::Null);
                            actions.push(PendingAction::SendCommand {
                                tenant_id: rule.tenant_id.clone(),
                                device_id: device_id.to_string(),
                                command,
                                params,
                            });
                        }
                        _ => {}
                    }
                }

                actions.push(PendingAction::UpdateCooldown {
                    tenant_id: rule.tenant_id.clone(),
                    rule_id: rule.id.clone(),
                    device_id: device_id.to_string(),
                    fired_at: Utc::now().naive_utc(),
                });
            }
        } else {
            // Device is outside the zone(s).
            if was_inside {
                // Just exited — clear entry time.
                actions.push(PendingAction::UpdateZoneEntry {
                    tenant_id: rule.tenant_id.clone(),
                    rule_id: rule.id.clone(),
                    device_id: device_id.to_string(),
                    entered_at: None,
                });
            }

            // Auto-resolve any active alert.
            // Skip empty-string sentinels (reservation in-flight).
            if let Some(alert_id) = existing_alert_id {
                if !alert_id.is_empty() {
                    actions.push(PendingAction::ResolveAlert { alert_id });
                }
            }
        }
    }

    actions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::*;
    use crate::rule_engine::cache::RuleCache;
    use crate::rule_engine::types::{
        CachedAction, CachedCondition, CachedRule, StatusChange, TelemetryData,
    };

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_telemetry(temperature: f32, humidity: f32, battery_level: f32) -> TelemetryData {
        TelemetryData {
            temperature,
            humidity,
            battery_level,
            latitude: 0.0,
            longitude: 0.0,
            speed: 0.0,
            altitude: 0.0,
            heading: 0.0,
        }
    }

    fn make_condition(field: &str, operator: &str, value: &str) -> CachedCondition {
        CachedCondition {
            field: field.to_string(),
            operator: operator.to_string(),
            value: value.to_string(),
            zone_id: None,
        }
    }

    fn make_alert_action(severity: &str) -> CachedAction {
        CachedAction {
            action_type: "alert".to_string(),
            config: json!({"severity": severity}),
        }
    }

    fn make_webhook_action(url: &str) -> CachedAction {
        CachedAction {
            action_type: "webhook".to_string(),
            config: json!({"url": url}),
        }
    }

    fn make_command_action(command: &str) -> CachedAction {
        CachedAction {
            action_type: "command".to_string(),
            config: json!({"command": command, "params": {"key": "val"}}),
        }
    }

    fn make_rule(
        id: &str,
        trigger_type: &str,
        target_type: &str,
        target_id: Option<&str>,
        cooldown_seconds: i32,
        conditions: Vec<CachedCondition>,
        actions: Vec<CachedAction>,
    ) -> CachedRule {
        CachedRule {
            tenant_id: DEFAULT_TENANT_ID.to_string(),
            id: id.to_string(),
            name: format!("Rule {}", id),
            trigger_type: trigger_type.to_string(),
            target_type: target_type.to_string(),
            target_id: target_id.map(|s| s.to_string()),
            cooldown_seconds,
            conditions,
            actions,
        }
    }

    fn empty_cache() -> RuleCache {
        RuleCache::default()
    }

    // -----------------------------------------------------------------------
    // get_field_value
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_field_value_temperature() {
        let data = make_telemetry(22.5, 60.0, 80.0);
        assert_eq!(get_field_value("temperature", &data), Some(22.5));
    }

    #[test]
    fn test_get_field_value_humidity() {
        let data = make_telemetry(22.5, 60.0, 80.0);
        assert_eq!(get_field_value("humidity", &data), Some(60.0));
    }

    #[test]
    fn test_get_field_value_battery_level() {
        let data = make_telemetry(22.5, 60.0, 80.0);
        assert_eq!(get_field_value("battery_level", &data), Some(80.0));
    }

    #[test]
    fn test_get_field_value_unknown_field() {
        let data = make_telemetry(22.5, 60.0, 80.0);
        assert_eq!(get_field_value("pressure", &data), None);
    }

    // -----------------------------------------------------------------------
    // evaluate_condition
    // -----------------------------------------------------------------------

    #[test]
    fn test_evaluate_condition_gt_true() {
        let data = make_telemetry(85.0, 50.0, 90.0);
        let cond = make_condition("temperature", "gt", "80");
        assert!(evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_gt_false() {
        let data = make_telemetry(75.0, 50.0, 90.0);
        let cond = make_condition("temperature", "gt", "80");
        assert!(!evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_gte_edge_true() {
        let data = make_telemetry(80.0, 50.0, 90.0);
        let cond = make_condition("temperature", "gte", "80");
        assert!(evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_gte_above_true() {
        let data = make_telemetry(85.0, 50.0, 90.0);
        let cond = make_condition("temperature", "gte", "80");
        assert!(evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_gte_below_false() {
        let data = make_telemetry(79.0, 50.0, 90.0);
        let cond = make_condition("temperature", "gte", "80");
        assert!(!evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_lt_true() {
        let data = make_telemetry(22.5, 50.0, 10.0);
        let cond = make_condition("battery_level", "lt", "20");
        assert!(evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_lt_false() {
        let data = make_telemetry(22.5, 50.0, 90.0);
        let cond = make_condition("battery_level", "lt", "20");
        assert!(!evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_lte_edge_true() {
        let data = make_telemetry(22.5, 50.0, 20.0);
        let cond = make_condition("battery_level", "lte", "20");
        assert!(evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_eq_true() {
        let data = make_telemetry(25.0, 50.0, 80.0);
        let cond = make_condition("temperature", "eq", "25");
        assert!(evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_eq_false() {
        let data = make_telemetry(26.0, 50.0, 80.0);
        let cond = make_condition("temperature", "eq", "25");
        assert!(!evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_neq_true() {
        let data = make_telemetry(30.0, 50.0, 80.0);
        let cond = make_condition("temperature", "neq", "25");
        assert!(evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_neq_false() {
        let data = make_telemetry(25.0, 50.0, 80.0);
        let cond = make_condition("temperature", "neq", "25");
        assert!(!evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_unknown_field_returns_false() {
        let data = make_telemetry(25.0, 50.0, 80.0);
        let cond = make_condition("pressure", "gt", "1000");
        assert!(!evaluate_condition(&cond, &data));
    }

    #[test]
    fn test_evaluate_condition_unparseable_value_returns_false() {
        let data = make_telemetry(25.0, 50.0, 80.0);
        let cond = make_condition("temperature", "gt", "not_a_number");
        assert!(!evaluate_condition(&cond, &data));
    }

    // -----------------------------------------------------------------------
    // evaluate_status_condition
    // -----------------------------------------------------------------------

    #[test]
    fn test_evaluate_status_condition_eq_match() {
        let cond = make_condition("status", "eq", "offline");
        assert!(evaluate_status_condition(&cond, "offline"));
    }

    #[test]
    fn test_evaluate_status_condition_eq_mismatch() {
        let cond = make_condition("status", "eq", "offline");
        assert!(!evaluate_status_condition(&cond, "online"));
    }

    #[test]
    fn test_evaluate_status_condition_neq_true() {
        let cond = make_condition("status", "neq", "offline");
        assert!(evaluate_status_condition(&cond, "online"));
    }

    #[test]
    fn test_evaluate_status_condition_neq_false() {
        let cond = make_condition("status", "neq", "offline");
        assert!(!evaluate_status_condition(&cond, "offline"));
    }

    // -----------------------------------------------------------------------
    // is_in_cooldown
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_in_cooldown_no_entry() {
        let cache = empty_cache();
        assert!(!is_in_cooldown(&cache, "rule1", "dev1", 60));
    }

    #[test]
    fn test_is_in_cooldown_recent_entry_true() {
        let mut cache = empty_cache();
        let recent = Utc::now().naive_utc() - Duration::seconds(10);
        cache.cooldowns.insert(
            (
                DEFAULT_TENANT_ID.to_string(),
                "rule1".to_string(),
                "dev1".to_string(),
            ),
            recent,
        );
        // 60-second cooldown, only 10s elapsed → still in cooldown
        assert!(is_in_cooldown(&cache, "rule1", "dev1", 60));
    }

    #[test]
    fn test_is_in_cooldown_expired_entry_false() {
        let mut cache = empty_cache();
        let old = Utc::now().naive_utc() - Duration::seconds(120);
        cache.cooldowns.insert(
            (
                DEFAULT_TENANT_ID.to_string(),
                "rule1".to_string(),
                "dev1".to_string(),
            ),
            old,
        );
        // 60-second cooldown, 120s elapsed → no longer in cooldown
        assert!(!is_in_cooldown(&cache, "rule1", "dev1", 60));
    }

    #[test]
    fn test_is_in_cooldown_zero_cooldown_always_false() {
        let mut cache = empty_cache();
        let recent = Utc::now().naive_utc() - Duration::seconds(1);
        cache.cooldowns.insert(
            (
                DEFAULT_TENANT_ID.to_string(),
                "rule1".to_string(),
                "dev1".to_string(),
            ),
            recent,
        );
        // cooldown_seconds = 0 → never in cooldown
        assert!(!is_in_cooldown(&cache, "rule1", "dev1", 0));
    }

    // -----------------------------------------------------------------------
    // format_value
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_value_with_fraction() {
        assert_eq!(format_value(85.2), "85.2");
    }

    #[test]
    fn test_format_value_whole_number() {
        assert_eq!(format_value(95.0), "95");
    }

    #[test]
    fn test_format_value_zero() {
        assert_eq!(format_value(0.0), "0");
    }

    // -----------------------------------------------------------------------
    // build_alert_message
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_alert_message_single_condition() {
        let data = make_telemetry(85.2, 50.0, 80.0);
        let conditions = vec![make_condition("temperature", "gt", "80")];
        let msg = build_alert_message(&conditions, &data);
        assert_eq!(msg, "temperature (85.2) exceeded 80");
    }

    #[test]
    fn test_build_alert_message_single_condition_whole_value() {
        let data = make_telemetry(95.0, 50.0, 80.0);
        let conditions = vec![make_condition("temperature", "gt", "80")];
        let msg = build_alert_message(&conditions, &data);
        assert_eq!(msg, "temperature (95) exceeded 80");
    }

    #[test]
    fn test_build_alert_message_lt_operator() {
        let data = make_telemetry(22.0, 50.0, 10.0);
        let conditions = vec![make_condition("battery_level", "lt", "20")];
        let msg = build_alert_message(&conditions, &data);
        assert_eq!(msg, "battery_level (10) dropped below 20");
    }

    #[test]
    fn test_build_alert_message_multiple_conditions() {
        let data = make_telemetry(85.0, 90.0, 50.0);
        let conditions = vec![
            make_condition("temperature", "gt", "80"),
            make_condition("humidity", "gt", "85"),
        ];
        let msg = build_alert_message(&conditions, &data);
        assert_eq!(
            msg,
            "temperature (85) exceeded 80 AND humidity (90) exceeded 85"
        );
    }

    #[test]
    fn test_build_status_alert_message() {
        let msg = build_status_alert_message("offline");
        assert_eq!(msg, "status changed to offline");
    }

    // -----------------------------------------------------------------------
    // evaluate_telemetry
    // -----------------------------------------------------------------------

    #[test]
    fn test_evaluate_telemetry_fires_when_threshold_exceeded() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r1",
            "telemetry",
            "global",
            None,
            0,
            vec![make_condition("temperature", "gt", "80")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        let data = make_telemetry(85.0, 50.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data, &cache);

        // Should produce CreateAlert + UpdateCooldown
        assert_eq!(actions.len(), 2);
        let has_create_alert = actions.iter().any(|a| {
            matches!(a, PendingAction::CreateAlert { rule_id, device_id, severity, .. }
                if rule_id == "r1" && device_id == "dev1" && severity == "critical")
        });
        let has_update_cooldown = actions.iter().any(|a| {
            matches!(a, PendingAction::UpdateCooldown { rule_id, device_id, .. }
                if rule_id == "r1" && device_id == "dev1")
        });
        assert!(has_create_alert, "Expected CreateAlert");
        assert!(has_update_cooldown, "Expected UpdateCooldown");
    }

    #[test]
    fn test_evaluate_telemetry_does_not_fire_below_threshold() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r1",
            "telemetry",
            "global",
            None,
            0,
            vec![make_condition("temperature", "gt", "80")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        let data = make_telemetry(75.0, 50.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data, &cache);

        assert!(actions.is_empty());
    }

    #[test]
    fn test_evaluate_telemetry_and_conditions_all_must_match() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r1",
            "telemetry",
            "global",
            None,
            0,
            vec![
                make_condition("temperature", "gt", "80"),
                make_condition("humidity", "gt", "85"),
            ],
            vec![make_alert_action("warning")],
        );
        cache.insert_rule(rule);

        // temperature OK, humidity NOT → no fire
        let data_no_fire = make_telemetry(85.0, 70.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data_no_fire, &cache);
        assert!(
            actions.is_empty(),
            "Should not fire when only one condition matches"
        );

        // Both conditions met → fire
        let data_fire = make_telemetry(85.0, 90.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data_fire, &cache);
        assert!(!actions.is_empty(), "Should fire when all conditions match");
    }

    #[test]
    fn test_evaluate_telemetry_respects_cooldown() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r1",
            "telemetry",
            "global",
            None,
            300, // 5-minute cooldown
            vec![make_condition("temperature", "gt", "80")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        // Plant a recent cooldown entry so the rule is blocked.
        let recent = Utc::now().naive_utc() - Duration::seconds(10);
        cache.cooldowns.insert(
            (
                DEFAULT_TENANT_ID.to_string(),
                "r1".to_string(),
                "dev1".to_string(),
            ),
            recent,
        );

        let data = make_telemetry(85.0, 50.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data, &cache);
        assert!(
            actions.is_empty(),
            "Should not fire while in cooldown window"
        );
    }

    #[test]
    fn test_evaluate_telemetry_auto_resolves_when_conditions_no_longer_met() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r1",
            "telemetry",
            "global",
            None,
            0,
            vec![make_condition("temperature", "gt", "80")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        // Simulate an existing active alert.
        cache.active_alerts.insert(
            (
                DEFAULT_TENANT_ID.to_string(),
                "r1".to_string(),
                "dev1".to_string(),
            ),
            "alert-42".to_string(),
        );

        // Temperature now below threshold.
        let data = make_telemetry(70.0, 50.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data, &cache);

        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0],
            PendingAction::ResolveAlert { alert_id } if alert_id == "alert-42"
        ));
    }

    #[test]
    fn test_evaluate_telemetry_updates_existing_active_alert() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r1",
            "telemetry",
            "global",
            None,
            0,
            vec![make_condition("temperature", "gt", "80")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        // Active alert already exists.
        cache.active_alerts.insert(
            (
                DEFAULT_TENANT_ID.to_string(),
                "r1".to_string(),
                "dev1".to_string(),
            ),
            "alert-99".to_string(),
        );

        // Conditions still met (temperature still high).
        let data = make_telemetry(90.0, 50.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data, &cache);

        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0],
            PendingAction::UpdateAlertValue { alert_id, triggered_value }
                if alert_id == "alert-99" && triggered_value == "90"
        ));
    }

    #[test]
    fn test_evaluate_telemetry_device_type_targeting() {
        let mut cache = empty_cache();
        // Rule targets device_type_id = "2".
        let rule = make_rule(
            "r1",
            "telemetry",
            "device_type",
            Some("2"),
            0,
            vec![make_condition("temperature", "gt", "80")],
            vec![make_alert_action("warning")],
        );
        cache.insert_rule(rule);

        let data = make_telemetry(85.0, 50.0, 90.0);

        // Device of type 1 → rule should NOT apply.
        let actions_type1 = evaluate_telemetry("dev1", 1, None, &data, &cache);
        assert!(
            actions_type1.is_empty(),
            "Rule should not apply to device_type 1"
        );

        // Device of type 2 → rule SHOULD apply.
        let actions_type2 = evaluate_telemetry("dev1", 2, None, &data, &cache);
        assert!(
            !actions_type2.is_empty(),
            "Rule should apply to device_type 2"
        );
    }

    #[test]
    fn test_evaluate_telemetry_webhook_action() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r1",
            "telemetry",
            "global",
            None,
            0,
            vec![make_condition("temperature", "gt", "80")],
            vec![make_webhook_action("https://example.com/hook")],
        );
        cache.insert_rule(rule);

        let data = make_telemetry(85.0, 50.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data, &cache);

        let has_webhook = actions.iter().any(|a| {
            matches!(a, PendingAction::SendWebhook { url, .. } if url == "https://example.com/hook")
        });
        assert!(has_webhook, "Expected SendWebhook action");
    }

    #[test]
    fn test_evaluate_telemetry_command_action() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r1",
            "telemetry",
            "global",
            None,
            0,
            vec![make_condition("temperature", "gt", "80")],
            vec![make_command_action("restart")],
        );
        cache.insert_rule(rule);

        let data = make_telemetry(85.0, 50.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data, &cache);

        let has_command = actions.iter().any(|a| {
            matches!(a, PendingAction::SendCommand { device_id, command, .. }
                if device_id == "dev1" && command == "restart")
        });
        assert!(has_command, "Expected SendCommand action");
    }

    // -----------------------------------------------------------------------
    // evaluate_status_change
    // -----------------------------------------------------------------------

    #[test]
    fn test_evaluate_status_change_fires_on_match() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r2",
            "device_status",
            "global",
            None,
            0,
            vec![make_condition("status", "eq", "offline")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        let change = StatusChange {
            old_status: "online".to_string(),
            new_status: "offline".to_string(),
        };
        let actions = evaluate_status_change("dev1", 1, None, &change, &cache);

        assert!(!actions.is_empty());
        let has_create = actions.iter().any(|a| {
            matches!(a, PendingAction::CreateAlert { rule_id, device_id, .. }
                if rule_id == "r2" && device_id == "dev1")
        });
        assert!(has_create, "Expected CreateAlert for status change");
    }

    #[test]
    fn test_evaluate_status_change_does_not_fire_on_mismatch() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r2",
            "device_status",
            "global",
            None,
            0,
            vec![make_condition("status", "eq", "offline")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        let change = StatusChange {
            old_status: "offline".to_string(),
            new_status: "online".to_string(), // not "offline"
        };
        let actions = evaluate_status_change("dev1", 1, None, &change, &cache);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_evaluate_status_change_auto_resolves_on_recovery() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r2",
            "device_status",
            "global",
            None,
            0,
            vec![make_condition("status", "eq", "offline")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        // Active alert for this rule+device.
        cache.active_alerts.insert(
            (
                DEFAULT_TENANT_ID.to_string(),
                "r2".to_string(),
                "dev1".to_string(),
            ),
            "alert-55".to_string(),
        );

        // Device comes back online — condition "eq offline" no longer met.
        let change = StatusChange {
            old_status: "offline".to_string(),
            new_status: "online".to_string(),
        };
        let actions = evaluate_status_change("dev1", 1, None, &change, &cache);

        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0],
            PendingAction::ResolveAlert { alert_id } if alert_id == "alert-55"
        ));
    }

    #[test]
    fn test_evaluate_status_change_webhook_action() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r2",
            "device_status",
            "global",
            None,
            0,
            vec![make_condition("status", "eq", "offline")],
            vec![make_webhook_action("https://hooks.example.com/status")],
        );
        cache.insert_rule(rule);

        let change = StatusChange {
            old_status: "online".to_string(),
            new_status: "offline".to_string(),
        };
        let actions = evaluate_status_change("dev1", 1, None, &change, &cache);

        let has_webhook = actions.iter().any(|a| {
            matches!(a, PendingAction::SendWebhook { url, .. }
                if url == "https://hooks.example.com/status")
        });
        assert!(has_webhook, "Expected SendWebhook for status change");
    }

    #[test]
    fn test_evaluate_status_change_command_action() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r2",
            "device_status",
            "global",
            None,
            0,
            vec![make_condition("status", "eq", "offline")],
            vec![make_command_action("reboot")],
        );
        cache.insert_rule(rule);

        let change = StatusChange {
            old_status: "online".to_string(),
            new_status: "offline".to_string(),
        };
        let actions = evaluate_status_change("dev1", 1, None, &change, &cache);

        let has_command = actions.iter().any(
            |a| matches!(a, PendingAction::SendCommand { command, .. } if command == "reboot"),
        );
        assert!(has_command, "Expected SendCommand for status change");
    }

    #[test]
    fn test_evaluate_status_change_alert_message_format() {
        let mut cache = empty_cache();
        let rule = make_rule(
            "r2",
            "device_status",
            "global",
            None,
            0,
            vec![make_condition("status", "eq", "offline")],
            vec![make_alert_action("critical")],
        );
        cache.insert_rule(rule);

        let change = StatusChange {
            old_status: "online".to_string(),
            new_status: "offline".to_string(),
        };
        let actions = evaluate_status_change("dev1", 1, None, &change, &cache);

        let alert_msg = actions.iter().find_map(|a| {
            if let PendingAction::CreateAlert { message, .. } = a {
                Some(message.clone())
            } else {
                None
            }
        });
        assert_eq!(alert_msg.as_deref(), Some("status changed to offline"));
    }

    #[test]
    fn test_evaluate_telemetry_ignores_status_rules() {
        let mut cache = empty_cache();
        // A status rule should be ignored by evaluate_telemetry.
        let rule = make_rule(
            "r3",
            "device_status",
            "global",
            None,
            0,
            vec![make_condition("status", "eq", "offline")],
            vec![make_alert_action("warning")],
        );
        cache.insert_rule(rule);

        let data = make_telemetry(85.0, 50.0, 90.0);
        let actions = evaluate_telemetry("dev1", 1, None, &data, &cache);
        assert!(
            actions.is_empty(),
            "evaluate_telemetry should ignore status rules"
        );
    }

    #[test]
    fn test_evaluate_status_change_ignores_telemetry_rules() {
        let mut cache = empty_cache();
        // A telemetry rule should be ignored by evaluate_status_change.
        let rule = make_rule(
            "r4",
            "telemetry",
            "global",
            None,
            0,
            vec![make_condition("temperature", "gt", "80")],
            vec![make_alert_action("warning")],
        );
        cache.insert_rule(rule);

        let change = StatusChange {
            old_status: "online".to_string(),
            new_status: "offline".to_string(),
        };
        let actions = evaluate_status_change("dev1", 1, None, &change, &cache);
        assert!(
            actions.is_empty(),
            "evaluate_status_change should ignore telemetry rules"
        );
    }
}
