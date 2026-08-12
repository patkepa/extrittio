use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use turso::{Connection, Row, params};

use crate::domains::alerts::port::AlertRepository;
use crate::domains::alerts::types::{
    AlertListFilter, AlertRecord, AlertTransition, AlertTransitionOutcome, CooldownRecord,
    NewAlertRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

fn decode(r: &Row) -> Result<AlertRecord, PersistenceError> {
    Ok(AlertRecord {
        id: r.get(0).map_err(row::error)?,
        tenant_id: r.get(1).map_err(row::error)?,
        rule_id: r.get(2).map_err(row::error)?,
        device_id: r.get(3).map_err(row::error)?,
        severity: r.get(4).map_err(row::error)?,
        status: r.get(5).map_err(row::error)?,
        message: r.get(6).map_err(row::error)?,
        triggered_value: r.get(7).map_err(row::error)?,
        resolved_at: r
            .get::<Option<i64>>(8)
            .map_err(row::error)?
            .map(row::datetime)
            .transpose()?
            .map(|v| v.naive_utc()),
        acknowledged_at: r
            .get::<Option<i64>>(9)
            .map_err(row::error)?
            .map(row::datetime)
            .transpose()?
            .map(|v| v.naive_utc()),
        created_at: row::datetime(r.get(10).map_err(row::error)?)?.naive_utc(),
    })
}
async fn get_from(
    c: &Connection,
    t: &TenantId,
    id: &str,
) -> Result<Option<AlertRecord>, PersistenceError> {
    let mut rows=c.query("SELECT id,tenant_id,rule_id,device_id,severity,status,message,triggered_value,resolved_at,acknowledged_at,created_at FROM alerts WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id]).await.map_err(row::error)?;
    rows.next()
        .await
        .map_err(row::error)?
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
    let valid = match tr {
        AlertTransition::Acknowledge => current.status == "active",
        AlertTransition::Resolve => current.status != "resolved",
        AlertTransition::Reactivate => current.status != "active",
    };
    if !valid {
        return Ok(AlertTransitionOutcome::InvalidStatus(current.status));
    }
    let now = Utc::now().timestamp_micros();
    match tr{AlertTransition::Acknowledge=>c.execute("UPDATE alerts SET status='acknowledged',acknowledged_at=?3 WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id,now]).await,AlertTransition::Resolve=>c.execute("UPDATE alerts SET status='resolved',resolved_at=?3 WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id,now]).await,AlertTransition::Reactivate=>c.execute("UPDATE alerts SET status='active',acknowledged_at=NULL,resolved_at=NULL WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id]).await}.map_err(row::error)?;
    Ok(AlertTransitionOutcome::Updated(Box::new(
        get_from(c, t, id)
            .await?
            .ok_or(PersistenceError::NotFound)?,
    )))
}

#[async_trait]
impl AlertRepository for TursoAdapter {
    async fn create(
        &self,
        t: &TenantId,
        r: NewAlertRecord,
    ) -> Result<AlertRecord, PersistenceError> {
        let w = self.database.writer().await;
        let now = Utc::now().timestamp_micros();
        w.execute("INSERT INTO alerts(id,tenant_id,rule_id,device_id,severity,status,message,triggered_value,created_at)VALUES(?1,?2,?3,?4,?5,'active',?6,?7,?8)",params![r.id.clone(),t.as_str(),r.rule_id,r.device_id,r.severity,r.message,r.triggered_value,now]).await.map_err(row::error)?;
        get_from(&w, t, &r.id)
            .await?
            .ok_or(PersistenceError::NotFound)
    }
    async fn update_triggered_value(
        &self,
        t: &TenantId,
        id: &str,
        value: String,
    ) -> Result<bool, PersistenceError> {
        let w = self.database.writer().await;
        w.execute(
            "UPDATE alerts SET triggered_value=?3 WHERE tenant_id=?1 AND id=?2",
            params![t.as_str(), id, value],
        )
        .await
        .map(|n| n == 1)
        .map_err(row::error)
    }
    async fn list(
        &self,
        t: &TenantId,
        f: AlertListFilter,
    ) -> Result<(Vec<AlertRecord>, i64), PersistenceError> {
        let c = self.database.connect()?;
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
            .map_err(row::error)?;
        let total = cr
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::error)?;
        let mut rows=c.query(&format!("SELECT id,tenant_id,rule_id,device_id,severity,status,message,triggered_value,resolved_at,acknowledged_at,created_at {base} ORDER BY created_at DESC,id DESC LIMIT ?8 OFFSET ?9"),params![t.as_str(),f.status,f.severity,f.device_id,f.rule_id,f.since.map(|v|v.and_utc().timestamp_micros()),f.before.map(|v|v.and_utc().timestamp_micros()),f.limit,f.offset]).await.map_err(row::error)?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await.map_err(row::error)? {
            out.push(decode(&r)?)
        }
        Ok((out, total))
    }
    async fn get(&self, t: &TenantId, id: &str) -> Result<Option<AlertRecord>, PersistenceError> {
        get_from(&self.database.connect()?, t, id).await
    }
    async fn transition(
        &self,
        t: &TenantId,
        id: &str,
        tr: AlertTransition,
    ) -> Result<AlertTransitionOutcome, PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        let out = transition_from(&tx, t, id, tr).await?;
        tx.commit().await.map_err(row::error)?;
        Ok(out)
    }
    async fn transition_many(
        &self,
        t: &TenantId,
        ids: Vec<String>,
        tr: AlertTransition,
    ) -> Result<Vec<AlertRecord>, PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        let mut out = Vec::new();
        for id in ids {
            if let AlertTransitionOutcome::Updated(r) = transition_from(&tx, t, &id, tr).await? {
                out.push(*r)
            }
        }
        tx.commit().await.map_err(row::error)?;
        Ok(out)
    }
    async fn summary(&self, t: &TenantId) -> Result<Vec<(String, String, i64)>, PersistenceError> {
        let c = self.database.connect()?;
        let mut rows=c.query("SELECT status,severity,count(*) FROM alerts WHERE tenant_id=?1 GROUP BY status,severity ORDER BY status,severity",params![t.as_str()]).await.map_err(row::error)?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await.map_err(row::error)? {
            out.push((
                r.get(0).map_err(row::error)?,
                r.get(1).map_err(row::error)?,
                r.get(2).map_err(row::error)?,
            ))
        }
        Ok(out)
    }
    async fn persist_cooldowns(&self, cs: Vec<CooldownRecord>) -> Result<(), PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        for c in cs {
            tx.execute("INSERT INTO rule_cooldowns(tenant_id,rule_id,device_id,last_fired_at)VALUES(?1,?2,?3,?4)ON CONFLICT(tenant_id,rule_id,device_id)DO UPDATE SET last_fired_at=excluded.last_fired_at",params![c.tenant_id,c.rule_id,c.device_id,c.last_fired_at.and_utc().timestamp_micros()]).await.map_err(row::error)?;
        }
        tx.commit().await.map_err(row::error)
    }
    async fn delete_resolved_before(
        &self,
        t: &TenantId,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        let w = self.database.writer().await;
        w.execute(
            "DELETE FROM alerts WHERE tenant_id=?1 AND status='resolved' AND resolved_at<?2",
            params![t.as_str(), cutoff.and_utc().timestamp_micros()],
        )
        .await
        .map(|n| n as usize)
        .map_err(row::error)
    }
}
