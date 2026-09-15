//! Transaction participants for ingress still hosted in the migration bridge.
use crate::{
    error::map_diesel_error,
    schema::{alerts, rule_cooldowns},
};
use diesel::{PgConnection, prelude::*};
use extrittio_backend_core::rule_snapshots::{DeviceRuleEvaluation, DeviceRuleRuntime};
use extrittio_backend_core::{PersistenceError, rule_engine::types::PendingAction};

#[derive(QueryableByName)]
struct DeviceTargets {
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
    fleet_id: Option<i32>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    blueprint_id: Option<String>,
}

/// Caller must hold the device row lock and commit the returned deliveries in
/// the SAME transaction. This function never opens a connection or commits.
pub fn evaluate_rules_in_transaction(
    connection: &mut PgConnection,
    tenant: &str,
    device: &str,
    plan: Option<&DeviceRuleEvaluation>,
) -> Result<Vec<PendingAction>, PersistenceError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };
    if plan.tenant.as_str() != tenant || plan.device_id != device {
        return Err(PersistenceError::CorruptData(
            "rule evaluation scope does not match ingress".into(),
        ));
    }
    let targets = diesel::sql_query("SELECT d.fleet_id,(SELECT r.blueprint_id FROM device_contract_assignments a JOIN device_contracts c ON c.tenant_id=a.tenant_id AND c.id=a.desired_contract_id JOIN device_blueprint_revisions r ON r.tenant_id=c.tenant_id AND r.id=c.blueprint_revision_id WHERE a.tenant_id=d.tenant_id AND a.device_id=d.id) AS blueprint_id FROM devices d WHERE d.tenant_id=$1 AND d.id=$2")
        .bind::<diesel::sql_types::Text,_>(tenant).bind::<diesel::sql_types::Text,_>(device)
        .get_result::<DeviceTargets>(connection).map_err(map_diesel_error)?;
    let mut plan = plan.clone();
    plan.fleet_id = targets.fleet_id;
    plan.blueprint_id = targets.blueprint_id;
    let mut runtime = DeviceRuleRuntime::default();
    // Do not let a stale definition create runtime rows for a deleted rule.
    // Key-share locks preserve referenced identities through the outer commit.
    use crate::schema::rules;
    runtime.live_rule_ids = rules::table
        .filter(rules::tenant_id.eq(tenant))
        .filter(rules::id.eq_any(plan.candidate_rule_ids()))
        .filter(rules::enabled.eq(true))
        .order(rules::id.asc())
        .for_key_share()
        .select(rules::id)
        .load::<String>(connection)
        .map_err(map_diesel_error)?
        .into_iter()
        .collect();

    let active = alerts::table
        .filter(alerts::tenant_id.eq(tenant))
        .filter(alerts::device_id.eq(device))
        .filter(alerts::status.eq_any(["active", "acknowledged"]))
        .order((alerts::created_at.asc(), alerts::id.asc()))
        .select((alerts::rule_id, alerts::id))
        .load::<(Option<String>, String)>(connection)
        .map_err(map_diesel_error)?;
    for (rule, alert) in active {
        if let Some(rule) = rule {
            runtime
                .active_alerts
                .insert((tenant.into(), rule, device.into()), alert);
        }
    }
    let cooldowns = rule_cooldowns::table
        .filter(rule_cooldowns::tenant_id.eq(tenant))
        .filter(rule_cooldowns::device_id.eq(device))
        .select((rule_cooldowns::rule_id, rule_cooldowns::last_fired_at))
        .load::<(String, chrono::NaiveDateTime)>(connection)
        .map_err(map_diesel_error)?;
    for (rule, time) in cooldowns {
        runtime
            .cooldowns
            .insert((tenant.into(), rule, device.into()), time);
    }
    use crate::schema::rule_zone_entries;
    let entries = rule_zone_entries::table
        .filter(rule_zone_entries::tenant_id.eq(tenant))
        .filter(rule_zone_entries::device_id.eq(device))
        .select((rule_zone_entries::rule_id, rule_zone_entries::entered_at))
        .load::<(String, chrono::NaiveDateTime)>(connection)
        .map_err(map_diesel_error)?;
    for (rule, time) in entries {
        runtime
            .zone_entries
            .insert((tenant.into(), rule, device.into()), time);
    }
    let decision = plan.decide(runtime);
    for cooldown in decision.cooldowns {
        crate::alerts_sql::upsert_cooldown(
            connection,
            &crate::models::RuleCooldown {
                tenant_id: cooldown.tenant_id,
                rule_id: cooldown.rule_id,
                device_id: cooldown.device_id,
                last_fired_at: cooldown.last_fired_at,
            },
        )
        .map_err(map_diesel_error)?;
    }
    for entry in decision.zone_entries {
        if let Some(time) = entry.entered_at {
            diesel::insert_into(rule_zone_entries::table)
                .values((
                    rule_zone_entries::tenant_id.eq(&entry.tenant_id),
                    rule_zone_entries::rule_id.eq(&entry.rule_id),
                    rule_zone_entries::device_id.eq(&entry.device_id),
                    rule_zone_entries::entered_at.eq(time),
                ))
                .on_conflict((
                    rule_zone_entries::tenant_id,
                    rule_zone_entries::rule_id,
                    rule_zone_entries::device_id,
                ))
                .do_update()
                .set(rule_zone_entries::entered_at.eq(time))
                .execute(connection)
                .map_err(map_diesel_error)?;
        } else {
            diesel::delete(
                rule_zone_entries::table
                    .filter(rule_zone_entries::tenant_id.eq(&entry.tenant_id))
                    .filter(rule_zone_entries::rule_id.eq(&entry.rule_id))
                    .filter(rule_zone_entries::device_id.eq(&entry.device_id)),
            )
            .execute(connection)
            .map_err(map_diesel_error)?;
        }
    }

    Ok(decision.deliveries)
}
