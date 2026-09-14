//! Transaction participants for ingress still hosted in the migration bridge.
use crate::row;
use extrittio_backend_core::rule_snapshots::{DeviceRuleEvaluation, DeviceRuleRuntime};
use extrittio_backend_core::{PersistenceError, rule_engine::types::PendingAction};
use turso::{Connection, params};

/// Caller must already hold the database write transaction and enqueue returned
/// deliveries before committing it. Never opens or commits another transaction.
pub async fn evaluate_rules_in_transaction(
    connection: &Connection,
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
    let mut targets = connection.query("SELECT d.device_type_id,d.fleet_id,(SELECT r.blueprint_id FROM device_contract_assignments a JOIN device_contracts c ON c.tenant_id=a.tenant_id AND c.id=a.desired_contract_id JOIN device_blueprint_revisions r ON r.tenant_id=c.tenant_id AND r.id=c.blueprint_revision_id WHERE a.tenant_id=d.tenant_id AND a.device_id=d.id) AS blueprint_id FROM devices d WHERE d.tenant_id=?1 AND d.id=?2",params![tenant,device]).await.map_err(row::legacy_error)?;
    let target = targets
        .next()
        .await
        .map_err(row::legacy_error)?
        .ok_or(PersistenceError::NotFound)?;
    let mut plan = plan.clone();
    plan.device_type_id = row::i32(target.get(0).map_err(row::legacy_error)?, "device_type_id")?;
    plan.fleet_id = target
        .get::<Option<i64>>(1)
        .map_err(row::legacy_error)?
        .map(|v| row::i32(v, "fleet_id"))
        .transpose()?;
    plan.blueprint_id = target.get(2).map_err(row::legacy_error)?;
    drop(targets);
    let mut runtime = DeviceRuleRuntime::default();
    // The outer write transaction prevents rule deletion after this lookup.
    let candidates = serde_json::to_string(&plan.candidate_rule_ids())
        .map_err(|e| PersistenceError::Internal(e.to_string()))?;
    let mut eligible = connection.query("SELECT id FROM rules WHERE tenant_id=?1 AND enabled=1 AND id IN (SELECT value FROM json_each(?2)) ORDER BY id",params![tenant,candidates]).await.map_err(row::legacy_error)?;
    while let Some(r) = eligible.next().await.map_err(row::legacy_error)? {
        runtime
            .live_rule_ids
            .insert(r.get(0).map_err(row::legacy_error)?);
    }
    drop(eligible);

    let mut active=connection.query("SELECT rule_id,id FROM alerts WHERE tenant_id=?1 AND device_id=?2 AND status IN ('active','acknowledged') ORDER BY created_at,id",params![tenant,device]).await.map_err(row::legacy_error)?;
    while let Some(r) = active.next().await.map_err(row::legacy_error)? {
        if let Some(rule) = r.get::<Option<String>>(0).map_err(row::legacy_error)? {
            runtime.active_alerts.insert(
                (tenant.into(), rule, device.into()),
                r.get(1).map_err(row::legacy_error)?,
            );
        }
    }
    drop(active);
    let mut cooldowns = connection
        .query(
            "SELECT rule_id,last_fired_at FROM rule_cooldowns WHERE tenant_id=?1 AND device_id=?2",
            params![tenant, device],
        )
        .await
        .map_err(row::legacy_error)?;
    while let Some(r) = cooldowns.next().await.map_err(row::legacy_error)? {
        let rule = r.get(0).map_err(row::legacy_error)?;
        let time = row::datetime(r.get(1).map_err(row::legacy_error)?)?.naive_utc();
        runtime
            .cooldowns
            .insert((tenant.into(), rule, device.into()), time);
    }
    drop(cooldowns);
    let mut entries = connection
        .query(
            "SELECT rule_id,entered_at FROM rule_zone_entries WHERE tenant_id=?1 AND device_id=?2",
            params![tenant, device],
        )
        .await
        .map_err(row::legacy_error)?;
    while let Some(r) = entries.next().await.map_err(row::legacy_error)? {
        runtime.zone_entries.insert(
            (
                tenant.into(),
                r.get(0).map_err(row::legacy_error)?,
                device.into(),
            ),
            row::datetime(r.get(1).map_err(row::legacy_error)?)?.naive_utc(),
        );
    }
    drop(entries);
    let decision = plan.decide(runtime);
    for c in decision.cooldowns {
        crate::alerts::upsert_cooldown(
            connection,
            &c.tenant_id,
            &c.rule_id,
            &c.device_id,
            c.last_fired_at.and_utc().timestamp_micros(),
        )
        .await?;
    }
    for entry in decision.zone_entries {
        if let Some(time) = entry.entered_at {
            connection.execute("INSERT INTO rule_zone_entries(tenant_id,rule_id,device_id,entered_at) VALUES(?1,?2,?3,?4) ON CONFLICT(tenant_id,rule_id,device_id) DO UPDATE SET entered_at=excluded.entered_at",params![entry.tenant_id,entry.rule_id,entry.device_id,time.and_utc().timestamp_micros()]).await.map_err(row::legacy_error)?;
        } else {
            connection.execute("DELETE FROM rule_zone_entries WHERE tenant_id=?1 AND rule_id=?2 AND device_id=?3",params![entry.tenant_id,entry.rule_id,entry.device_id]).await.map_err(row::legacy_error)?;
        }
    }
    for rule in decision.zone_observations {
        connection.execute("INSERT INTO rule_zone_handoffs(tenant_id,rule_id,device_id,live_seen) VALUES(?1,?2,?3,1) ON CONFLICT(tenant_id,rule_id,device_id) DO UPDATE SET live_seen=1",params![tenant,rule,device]).await.map_err(row::legacy_error)?;
    }
    Ok(decision.deliveries)
}
