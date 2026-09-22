use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use turso::{Connection, Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::alerts::AlertRepository;
use extrittio_backend_core::alerts::{
    AlertListFilter, AlertRecord, AlertTransition, AlertTransitionOutcome, NewRuleAlertRecord,
};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoAlertRepository {
    handles: TursoConnectionHandles,
}
impl TursoAlertRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|e| PersistenceError::Unavailable(e.to_string()))
    }
}

fn decode(r: &Row) -> Result<AlertRecord, PersistenceError> {
    Ok(AlertRecord {
        id: r.get(0).map_err(row::legacy_error)?,
        tenant_id: r.get(1).map_err(row::legacy_error)?,
        rule_id: r.get(2).map_err(row::legacy_error)?,
        device_id: r.get(3).map_err(row::legacy_error)?,
        severity: r.get(4).map_err(row::legacy_error)?,
        status: r.get(5).map_err(row::legacy_error)?,
        message: r.get(6).map_err(row::legacy_error)?,
        triggered_value: r.get(7).map_err(row::legacy_error)?,
        resolved_at: r
            .get::<Option<i64>>(8)
            .map_err(row::legacy_error)?
            .map(row::datetime)
            .transpose()?
            .map(|v| v.naive_utc()),
        acknowledged_at: r
            .get::<Option<i64>>(9)
            .map_err(row::legacy_error)?
            .map(row::datetime)
            .transpose()?
            .map(|v| v.naive_utc()),
        created_at: row::datetime(r.get(10).map_err(row::legacy_error)?)?.naive_utc(),
    })
}
async fn get_from(
    c: &Connection,
    t: &TenantId,
    id: &str,
) -> Result<Option<AlertRecord>, PersistenceError> {
    let mut rows=c.query("SELECT id,tenant_id,rule_id,device_id,severity,status,message,triggered_value,resolved_at,acknowledged_at,created_at FROM alerts WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id]).await.map_err(row::legacy_error)?;
    rows.next()
        .await
        .map_err(row::legacy_error)?
        .map(|r| decode(&r))
        .transpose()
}
async fn transition_from(
    c: &Connection,
    t: &TenantId,
    id: &str,
    tr: AlertTransition,
) -> Result<AlertTransitionOutcome, PersistenceError> {
    let Some(current) = get_from(c, t, id).await? else {
        return Ok(AlertTransitionOutcome::NotFound);
    };
    let found = c
        .execute(
            "UPDATE devices SET id=id WHERE tenant_id=?1 AND id=?2",
            params![t.as_str(), current.device_id.as_str()],
        )
        .await
        .map_err(row::legacy_error)?;
    if found == 0 {
        return Ok(AlertTransitionOutcome::NotFound);
    }
    let Some(current) = get_from(c, t, id).await? else {
        return Ok(AlertTransitionOutcome::NotFound);
    };
    let valid = tr.accepts(&current.status);
    if !valid {
        return Ok(AlertTransitionOutcome::InvalidStatus(current.status));
    }
    if tr.requires_active_slot()
        && let Some(rule_id) = &current.rule_id
    {
        let mut rows=c.query("SELECT id FROM alerts WHERE tenant_id=?1 AND rule_id=?2 AND device_id=?3 AND id<>?4 AND status IN ('active','acknowledged') ORDER BY created_at DESC,id DESC LIMIT 1",params![t.as_str(),rule_id.as_str(),current.device_id.as_str(),id]).await.map_err(row::legacy_error)?;
        if let Some(row) = rows.next().await.map_err(row::legacy_error)? {
            return Ok(AlertTransitionOutcome::ActiveConflict(
                row.get(0).map_err(row::legacy_error)?,
            ));
        }
    }
    let now = Utc::now().timestamp_micros();
    match tr{AlertTransition::Acknowledge=>c.execute("UPDATE alerts SET status='acknowledged',acknowledged_at=?3 WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id,now]).await,AlertTransition::Resolve=>c.execute("UPDATE alerts SET status='resolved',resolved_at=?3 WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id,now]).await,AlertTransition::Reactivate=>c.execute("UPDATE alerts SET status='active',acknowledged_at=NULL,resolved_at=NULL WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id]).await}.map_err(row::legacy_error)?;

    if let Some(rule_id) = &current.rule_id {
        match tr {
            AlertTransition::Resolve => {
                upsert_cooldown(c, t.as_str(), rule_id, &current.device_id, now).await?;
            }
            AlertTransition::Reactivate => {
                c.execute(
                    "DELETE FROM rule_cooldowns WHERE tenant_id=?1 AND rule_id=?2 AND device_id=?3",
                    params![t.as_str(), rule_id.as_str(), current.device_id.as_str()],
                )
                .await
                .map_err(row::legacy_error)?;
            }
            AlertTransition::Acknowledge => {}
        }
    }
    Ok(AlertTransitionOutcome::Updated(Box::new(
        get_from(c, t, id)
            .await?
            .ok_or(PersistenceError::NotFound)?,
    )))
}

#[async_trait]
impl AlertRepository for TursoAlertRepository {
    async fn create_or_get_active(
        &self,
        t: &TenantId,
        r: NewRuleAlertRecord,
    ) -> Result<Option<AlertRecord>, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let tx = writer.transaction().await.map_err(row::legacy_error)?;
        if let Some(applied) = find_delivery(&tx, t, &r.id).await? {
            tx.commit().await.map_err(row::legacy_error)?;
            return Ok(applied);
        }
        // Acquire the database writer before checking state. The local writer
        // mutex alone is not the duplicate-prevention boundary.
        let found = tx
            .execute(
                "UPDATE devices SET id=id WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), r.device_id.as_str()],
            )
            .await
            .map_err(row::legacy_error)?;
        if found == 0 {
            return Err(PersistenceError::NotFound);
        }
        if let Some(applied) = find_delivery(&tx, t, &r.id).await? {
            tx.commit().await.map_err(row::legacy_error)?;
            return Ok(applied);
        }
        if let Some(existing) = get_from(&tx, t, &r.id).await? {
            record_delivery(&tx, t, &r.id, &existing.id).await?;
            tx.commit().await.map_err(row::legacy_error)?;
            return Ok(Some(existing));
        }
        let mut rows=tx.query("SELECT id,tenant_id,rule_id,device_id,severity,status,message,triggered_value,resolved_at,acknowledged_at,created_at FROM alerts WHERE tenant_id=?1 AND rule_id=?2 AND device_id=?3 AND status IN ('active','acknowledged') ORDER BY created_at DESC,id DESC LIMIT 1",params![t.as_str(),r.rule_id.as_str(),r.device_id.as_str()]).await.map_err(row::legacy_error)?;
        let existing = rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .map(|row| decode(&row))
            .transpose()?;
        drop(rows);
        if let Some(existing) = existing {
            record_delivery(&tx, t, &r.id, &existing.id).await?;
            tx.commit().await.map_err(row::legacy_error)?;
            return Ok(Some(existing));
        }
        let now = Utc::now().timestamp_micros();
        tx.execute("INSERT INTO alerts(id,tenant_id,rule_id,device_id,severity,status,message,triggered_value,created_at)VALUES(?1,?2,?3,?4,?5,'active',?6,?7,?8)",params![r.id.clone(),t.as_str(),r.rule_id,r.device_id,r.severity,r.message,r.triggered_value,now]).await.map_err(row::legacy_error)?;
        let alert = get_from(&tx, t, &r.id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        record_delivery(&tx, t, &r.id, &alert.id).await?;
        tx.commit().await.map_err(row::legacy_error)?;
        Ok(Some(alert))
    }

    async fn update_triggered_value(
        &self,
        t: &TenantId,
        id: &str,
        value: String,
    ) -> Result<bool, PersistenceError> {
        let w = self.handles.lock_writer().await;
        w.execute(
            "UPDATE alerts SET triggered_value=?3 WHERE tenant_id=?1 AND id=?2",
            params![t.as_str(), id, value],
        )
        .await
        .map(|n| n == 1)
        .map_err(row::legacy_error)
    }
    async fn list(
        &self,
        t: &TenantId,
        f: AlertListFilter,
    ) -> Result<(Vec<AlertRecord>, i64), PersistenceError> {
        let c = self.connect()?;
        let base = "FROM alerts WHERE tenant_id=?1 AND (?2 IS NULL OR status=?2) AND (?3 IS NULL OR severity=?3) AND (?4 IS NULL OR device_id=?4) AND (?5 IS NULL OR rule_id=?5) AND (?6 IS NULL OR created_at>=?6) AND (?7 IS NULL OR created_at<?7)";
        let mut cr = c
            .query(
                &format!("SELECT count(*) {base}"),
                params![
                    t.as_str(),
                    f.status.clone(),
                    f.severity.clone(),
                    f.device_id.clone(),
                    f.rule_id.clone(),
                    f.since.map(|v| v.and_utc().timestamp_micros()),
                    f.before.map(|v| v.and_utc().timestamp_micros())
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let total = cr
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::legacy_error)?;
        let mut rows=c.query(&format!("SELECT id,tenant_id,rule_id,device_id,severity,status,message,triggered_value,resolved_at,acknowledged_at,created_at {base} ORDER BY created_at DESC,id DESC LIMIT ?8 OFFSET ?9"),params![t.as_str(),f.status,f.severity,f.device_id,f.rule_id,f.since.map(|v|v.and_utc().timestamp_micros()),f.before.map(|v|v.and_utc().timestamp_micros()),f.limit,f.offset]).await.map_err(row::legacy_error)?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await.map_err(row::legacy_error)? {
            out.push(decode(&r)?)
        }
        Ok((out, total))
    }
    async fn get(&self, t: &TenantId, id: &str) -> Result<Option<AlertRecord>, PersistenceError> {
        get_from(&self.connect()?, t, id).await
    }
    async fn transition(
        &self,
        t: &TenantId,
        id: &str,
        tr: AlertTransition,
    ) -> Result<AlertTransitionOutcome, PersistenceError> {
        let mut w = self.handles.lock_writer().await;
        let tx = w.transaction().await.map_err(row::legacy_error)?;
        let out = transition_from(&tx, t, id, tr).await?;
        tx.commit().await.map_err(row::legacy_error)?;
        Ok(out)
    }
    async fn transition_many(
        &self,
        t: &TenantId,
        ids: Vec<String>,
        tr: AlertTransition,
    ) -> Result<Vec<AlertRecord>, PersistenceError> {
        let mut w = self.handles.lock_writer().await;
        let tx = w.transaction().await.map_err(row::legacy_error)?;
        let mut out = Vec::new();
        for id in ids.into_iter().collect::<std::collections::BTreeSet<_>>() {
            if let AlertTransitionOutcome::Updated(r) = transition_from(&tx, t, &id, tr).await? {
                out.push(*r)
            }
        }
        tx.commit().await.map_err(row::legacy_error)?;
        Ok(out)
    }
    async fn summary(&self, t: &TenantId) -> Result<Vec<(String, String, i64)>, PersistenceError> {
        let c = self.connect()?;
        let mut rows=c.query("SELECT status,severity,count(*) FROM alerts WHERE tenant_id=?1 GROUP BY status,severity ORDER BY status,severity",params![t.as_str()]).await.map_err(row::legacy_error)?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await.map_err(row::legacy_error)? {
            out.push((
                r.get(0).map_err(row::legacy_error)?,
                r.get(1).map_err(row::legacy_error)?,
                r.get(2).map_err(row::legacy_error)?,
            ))
        }
        Ok(out)
    }
    async fn delete_all_resolved_before(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        let w = self.handles.lock_writer().await;
        w.execute(
            "DELETE FROM alerts WHERE status='resolved' AND resolved_at<?1",
            params![cutoff.and_utc().timestamp_micros()],
        )
        .await
        .map(|n| n as usize)
        .map_err(row::legacy_error)
    }
}

async fn find_delivery(
    c: &Connection,
    tenant: &TenantId,
    delivery: &str,
) -> Result<Option<Option<AlertRecord>>, PersistenceError> {
    let mut rows = c
        .query(
            "SELECT alert_id FROM rule_alert_deliveries WHERE tenant_id=?1 AND delivery_id=?2",
            params![tenant.as_str(), delivery],
        )
        .await
        .map_err(row::legacy_error)?;
    let alert_id = rows
        .next()
        .await
        .map_err(row::legacy_error)?
        .map(|r| r.get::<String>(0))
        .transpose()
        .map_err(row::legacy_error)?;
    drop(rows);
    match alert_id {
        Some(id) => Ok(Some(get_from(c, tenant, &id).await?)),
        None => Ok(None),
    }
}
async fn record_delivery(
    c: &Connection,
    tenant: &TenantId,
    delivery: &str,
    alert: &str,
) -> Result<(), PersistenceError> {
    c.execute(
        "INSERT INTO rule_alert_deliveries(delivery_id,tenant_id,alert_id) VALUES(?1,?2,?3)",
        params![delivery, tenant.as_str(), alert],
    )
    .await
    .map_err(row::legacy_error)?;
    Ok(())
}

/// Must participate in the caller's write transaction.
pub(crate) async fn upsert_cooldown(
    c: &Connection,
    tenant: &str,
    rule: &str,
    device: &str,
    fired_at: i64,
) -> Result<(), PersistenceError> {
    c.execute("INSERT INTO rule_cooldowns(tenant_id,rule_id,device_id,last_fired_at)
        VALUES (?1,?2,?3,?4) ON CONFLICT(tenant_id,rule_id,device_id) DO UPDATE SET last_fired_at=max(rule_cooldowns.last_fired_at,excluded.last_fired_at)",params![tenant,rule,device,fired_at]).await.map_err(row::legacy_error)?;
    Ok(())
}

#[cfg(test)]
mod cooldown_tests {
    use super::*;

    #[tokio::test]
    async fn fresh_baseline_cooldowns_are_monotonic_and_reactivation_clears_them() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("cooldowns.db"),
            std::time::Duration::from_secs(1),
        )
        .await
        .unwrap();
        database.migrate().await.unwrap();
        let handles = database.shared_handles();
        {
            let mut writer = handles.lock_writer().await;
            writer.execute_batch("INSERT INTO devices(id,tenant_id,name,status,firmware,created_at,updated_at)
                VALUES('device','default','Device','online','1',0,0);
                INSERT INTO rules(id,tenant_id,name,enabled,trigger_type,target_type,cooldown_seconds,created_at,updated_at)
                VALUES('rule','default','Rule',1,'telemetry','global',60,0,0);
                INSERT INTO alerts(id,tenant_id,rule_id,device_id,severity,status,message,created_at)
                VALUES('alert','default','rule','device','warning','resolved','Test',0);").await.unwrap();
            let tx = writer.transaction().await.unwrap();
            upsert_cooldown(&tx, "default", "rule", "device", 20)
                .await
                .unwrap();
            upsert_cooldown(&tx, "default", "rule", "device", 10)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        let connection = handles.connect().unwrap();
        let mut rows = connection
            .query("SELECT last_fired_at FROM rule_cooldowns", ())
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            20
        );
        drop(rows);
        let repository = TursoAlertRepository::from_handles(handles.clone());
        let outcome = repository
            .transition(
                &TenantId::new("default").unwrap(),
                "alert",
                AlertTransition::Reactivate,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, AlertTransitionOutcome::Updated(_)));
        let mut rows = connection
            .query("SELECT count(*) FROM rule_cooldowns", ())
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            0
        );
        drop(rows);
        {
            let mut writer = handles.lock_writer().await;
            let tx = writer.transaction().await.unwrap();
            upsert_cooldown(&tx, "default", "rule", "device", 30)
                .await
                .unwrap();
            tx.rollback().await.unwrap();
        }
        let mut rows = connection
            .query("SELECT count(*) FROM rule_cooldowns", ())
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            0
        );
    }
}
