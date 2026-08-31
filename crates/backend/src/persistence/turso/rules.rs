use async_trait::async_trait;
use chrono::NaiveDateTime;
use turso::{Connection, Row, params};

use crate::domains::rules::port::RuleRepository;
use crate::domains::rules::types::{
    NewRuleRecord, RuleActionRecord, RuleConditionRecord, RuleDetails, RuleFilter, RuleRecord,
    UpdateRuleRecord,
};
use crate::persistence::PersistenceError;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::types::{CachedAction, CachedCondition, CachedRule};
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

const RULE_COLUMNS: &str = "id,tenant_id,name,description,enabled,trigger_type,target_type,target_id,cooldown_seconds,created_at,updated_at";

fn decode_rule(r: &Row) -> Result<RuleRecord, PersistenceError> {
    Ok(RuleRecord {
        id: r.get(0).map_err(row::error)?,
        tenant_id: r.get(1).map_err(row::error)?,
        name: r.get(2).map_err(row::error)?,
        description: r.get(3).map_err(row::error)?,
        enabled: r.get::<i64>(4).map_err(row::error)? != 0,
        trigger_type: r.get(5).map_err(row::error)?,
        target_type: r.get(6).map_err(row::error)?,
        target_id: r.get(7).map_err(row::error)?,
        cooldown_seconds: row::i32(r.get(8).map_err(row::error)?, "cooldown_seconds")?,
        created_at: row::datetime(r.get(9).map_err(row::error)?)?.naive_utc(),
        updated_at: row::datetime(r.get(10).map_err(row::error)?)?.naive_utc(),
    })
}

async fn details_from(
    c: &Connection,
    tenant: &str,
    id: &str,
) -> Result<Option<RuleDetails>, PersistenceError> {
    let mut rules = c
        .query(
            &format!("SELECT {RULE_COLUMNS} FROM rules WHERE tenant_id=?1 AND id=?2"),
            params![tenant, id],
        )
        .await
        .map_err(row::error)?;
    let Some(rule_row) = rules.next().await.map_err(row::error)? else {
        return Ok(None);
    };
    let rule = decode_rule(&rule_row)?;
    drop(rules);
    let mut rows = c.query("SELECT id,field,operator,value,condition_group,zone_id FROM rule_conditions WHERE tenant_id=?1 AND rule_id=?2 ORDER BY condition_group,id",params![tenant,id]).await.map_err(row::error)?;
    let mut conditions = Vec::new();
    while let Some(r) = rows.next().await.map_err(row::error)? {
        conditions.push(RuleConditionRecord {
            id: r.get(0).map_err(row::error)?,
            field: r.get(1).map_err(row::error)?,
            operator: r.get(2).map_err(row::error)?,
            value: r.get(3).map_err(row::error)?,
            condition_group: row::i32(r.get(4).map_err(row::error)?, "condition_group")?,
            zone_id: r.get(5).map_err(row::error)?,
        });
    }
    drop(rows);
    let mut rows=c.query("SELECT id,action_type,config FROM rule_actions WHERE tenant_id=?1 AND rule_id=?2 ORDER BY id",params![tenant,id]).await.map_err(row::error)?;
    let mut actions = Vec::new();
    while let Some(r) = rows.next().await.map_err(row::error)? {
        let config: String = r.get(2).map_err(row::error)?;
        actions.push(RuleActionRecord {
            id: r.get(0).map_err(row::error)?,
            action_type: r.get(1).map_err(row::error)?,
            config: serde_json::from_str(&config)
                .map_err(|e| PersistenceError::CorruptData(e.to_string()))?,
        });
    }
    Ok(Some(RuleDetails {
        rule,
        conditions,
        actions,
    }))
}

async fn insert_children(
    c: &Connection,
    tenant: &str,
    rule_id: &str,
    conditions: Vec<RuleConditionRecord>,
    actions: Vec<RuleActionRecord>,
) -> Result<(), PersistenceError> {
    for v in conditions {
        c.execute("INSERT INTO rule_conditions(id,tenant_id,rule_id,field,operator,value,condition_group,zone_id)VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![v.id,tenant,rule_id,v.field,v.operator,v.value,v.condition_group,v.zone_id]).await.map_err(row::error)?;
    }
    for v in actions {
        c.execute("INSERT INTO rule_actions(id,tenant_id,rule_id,action_type,config)VALUES(?1,?2,?3,?4,?5)",params![v.id,tenant,rule_id,v.action_type,serde_json::to_string(&v.config).map_err(|e|PersistenceError::Internal(e.to_string()))?]).await.map_err(row::error)?;
    }
    Ok(())
}

#[async_trait]
impl RuleRepository for TursoAdapter {
    async fn list(
        &self,
        t: &TenantId,
        f: RuleFilter,
    ) -> Result<Vec<RuleDetails>, PersistenceError> {
        let c = self.database.connect()?;
        let mut rows=c.query(&format!("SELECT {RULE_COLUMNS} FROM rules WHERE tenant_id=?1 AND (?2 IS NULL OR enabled=?2) AND (?3 IS NULL OR trigger_type=?3) AND (?4 IS NULL OR target_type=?4) ORDER BY name,id"),params![t.as_str(),f.enabled.map(i64::from),f.trigger_type,f.target_type]).await.map_err(row::error)?;
        let mut ids = Vec::new();
        while let Some(r) = rows.next().await.map_err(row::error)? {
            ids.push(r.get::<String>(0).map_err(row::error)?);
        }
        drop(rows);
        let mut out = Vec::new();
        for id in ids {
            if let Some(v) = details_from(&c, t.as_str(), &id).await? {
                out.push(v)
            }
        }
        Ok(out)
    }
    async fn get(&self, t: &TenantId, id: &str) -> Result<Option<RuleDetails>, PersistenceError> {
        details_from(&self.database.connect()?, t.as_str(), id).await
    }
    async fn create(
        &self,
        t: &TenantId,
        r: NewRuleRecord,
    ) -> Result<RuleDetails, PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        let now = chrono::Utc::now().timestamp_micros();
        tx.execute("INSERT INTO rules(id,tenant_id,name,description,enabled,trigger_type,target_type,target_id,cooldown_seconds,created_at,updated_at)VALUES(?1,?2,?3,?4,1,?5,?6,?7,?8,?9,?9)",params![r.id.clone(),t.as_str(),r.name,r.description,r.trigger_type,r.target_type,r.target_id,r.cooldown_seconds,now]).await.map_err(row::error)?;
        insert_children(&tx, t.as_str(), &r.id, r.conditions, r.actions).await?;
        let out = details_from(&tx, t.as_str(), &r.id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        tx.commit().await.map_err(row::error)?;
        Ok(out)
    }
    async fn update(
        &self,
        t: &TenantId,
        id: &str,
        r: UpdateRuleRecord,
    ) -> Result<Option<RuleDetails>, PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        if tx
            .execute(
                "UPDATE rules SET updated_at=?3 WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id, r.updated_at.and_utc().timestamp_micros()],
            )
            .await
            .map_err(row::error)?
            == 0
        {
            tx.rollback().await.map_err(row::error)?;
            return Ok(None);
        }
        macro_rules! update {
            ($field:literal,$value:expr) => {
                if let Some(v) = $value {
                    tx.execute(
                        concat!(
                            "UPDATE rules SET ",
                            $field,
                            "=?3 WHERE tenant_id=?1 AND id=?2"
                        ),
                        params![t.as_str(), id, v],
                    )
                    .await
                    .map_err(row::error)?;
                }
            };
        }
        update!("name", r.name);
        update!("description", r.description);
        update!("trigger_type", r.trigger_type);
        update!("target_type", r.target_type);
        update!("target_id", r.target_id);
        update!("cooldown_seconds", r.cooldown_seconds);
        if let Some(v) = r.conditions {
            tx.execute(
                "DELETE FROM rule_conditions WHERE tenant_id=?1 AND rule_id=?2",
                params![t.as_str(), id],
            )
            .await
            .map_err(row::error)?;
            insert_children(&tx, t.as_str(), id, v, Vec::new()).await?
        }
        if let Some(v) = r.actions {
            tx.execute(
                "DELETE FROM rule_actions WHERE tenant_id=?1 AND rule_id=?2",
                params![t.as_str(), id],
            )
            .await
            .map_err(row::error)?;
            insert_children(&tx, t.as_str(), id, Vec::new(), v).await?
        }
        let out = details_from(&tx, t.as_str(), id).await?;
        tx.commit().await.map_err(row::error)?;
        Ok(out)
    }
    async fn delete(&self, t: &TenantId, id: &str) -> Result<bool, PersistenceError> {
        self.database
            .writer()
            .await
            .execute(
                "DELETE FROM rules WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id],
            )
            .await
            .map(|n| n > 0)
            .map_err(row::error)
    }
    async fn toggle(
        &self,
        t: &TenantId,
        id: &str,
        enabled: bool,
        updated_at: NaiveDateTime,
    ) -> Result<Option<RuleDetails>, PersistenceError> {
        let w = self.database.writer().await;
        if w.execute(
            "UPDATE rules SET enabled=?3,updated_at=?4 WHERE tenant_id=?1 AND id=?2",
            params![
                t.as_str(),
                id,
                i64::from(enabled),
                updated_at.and_utc().timestamp_micros()
            ],
        )
        .await
        .map_err(row::error)?
            == 0
        {
            return Ok(None);
        }
        details_from(&w, t.as_str(), id).await
    }
    async fn build_cache(&self) -> Result<RuleCache, PersistenceError> {
        let c = self.database.connect()?;
        let mut cache = RuleCache::default();
        let mut rows = c
            .query(
                "SELECT tenant_id,id FROM rules WHERE enabled=1 ORDER BY tenant_id,id",
                (),
            )
            .await
            .map_err(row::error)?;
        let mut keys = Vec::new();
        while let Some(r) = rows.next().await.map_err(row::error)? {
            keys.push((
                r.get::<String>(0).map_err(row::error)?,
                r.get::<String>(1).map_err(row::error)?,
            ))
        }
        drop(rows);
        for (tenant, id) in keys {
            let d = details_from(&c, &tenant, &id)
                .await?
                .ok_or(PersistenceError::NotFound)?;
            cache.insert_rule(CachedRule {
                tenant_id: tenant,
                id: d.rule.id,
                name: d.rule.name,
                trigger_type: d.rule.trigger_type,
                target_type: d.rule.target_type,
                target_id: d.rule.target_id,
                cooldown_seconds: d.rule.cooldown_seconds,
                conditions: d
                    .conditions
                    .into_iter()
                    .map(|v| CachedCondition {
                        field: v.field,
                        operator: v.operator,
                        value: v.value,
                        zone_id: v.zone_id,
                    })
                    .collect(),
                actions: d
                    .actions
                    .into_iter()
                    .map(|v| CachedAction {
                        action_type: v.action_type,
                        config: v.config,
                    })
                    .collect(),
            })
        }
        let mut rows = c
            .query(
                "SELECT tenant_id,rule_id,device_id,last_fired_at FROM rule_cooldowns",
                (),
            )
            .await
            .map_err(row::error)?;
        while let Some(r) = rows.next().await.map_err(row::error)? {
            cache.cooldowns.insert(
                (
                    r.get(0).map_err(row::error)?,
                    r.get(1).map_err(row::error)?,
                    r.get(2).map_err(row::error)?,
                ),
                row::datetime(r.get(3).map_err(row::error)?)?.naive_utc(),
            );
        }
        drop(rows);
        let mut rows=c.query("SELECT tenant_id,rule_id,device_id,id FROM alerts WHERE rule_id IS NOT NULL AND status IN ('active','acknowledged')",()).await.map_err(row::error)?;
        while let Some(r) = rows.next().await.map_err(row::error)? {
            cache.active_alerts.insert(
                (
                    r.get(0).map_err(row::error)?,
                    r.get(1).map_err(row::error)?,
                    r.get(2).map_err(row::error)?,
                ),
                r.get(3).map_err(row::error)?,
            );
        }
        drop(rows);
        Ok(cache)
    }
    async fn delete_stale_cooldowns(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        self.database
            .writer()
            .await
            .execute(
                "DELETE FROM rule_cooldowns WHERE last_fired_at<?1",
                params![cutoff.and_utc().timestamp_micros()],
            )
            .await
            .map(|n| n as usize)
            .map_err(row::error)
    }
}
