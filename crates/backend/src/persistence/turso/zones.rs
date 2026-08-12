use async_trait::async_trait;
use turso::{Connection, Row, params};

use crate::domains::zones::port::ZoneRepository;
use crate::domains::zones::types::{
    DeleteZoneOutcome, NewZoneRecord, UpdateZoneRecord, ZoneRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

fn decode(r: &Row) -> Result<ZoneRecord, PersistenceError> {
    let geometry: String = r.get(5).map_err(row::error)?;
    Ok(ZoneRecord {
        id: r.get(0).map_err(row::error)?,
        tenant_id: r.get(1).map_err(row::error)?,
        name: r.get(2).map_err(row::error)?,
        description: r.get(3).map_err(row::error)?,
        geometry_type: r.get(4).map_err(row::error)?,
        geometry_json: serde_json::from_str(&geometry)
            .map_err(|e| PersistenceError::CorruptData(e.to_string()))?,
        color: r.get(6).map_err(row::error)?,
        created_at: row::datetime(r.get(7).map_err(row::error)?)?.naive_utc(),
        updated_at: row::datetime(r.get(8).map_err(row::error)?)?.naive_utc(),
    })
}
async fn get_from(
    c: &Connection,
    t: &TenantId,
    id: &str,
) -> Result<Option<ZoneRecord>, PersistenceError> {
    let mut rs=c.query("SELECT id,tenant_id,name,description,geometry_type,geometry_json,color,created_at,updated_at FROM zones WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id]).await.map_err(row::error)?;
    rs.next()
        .await
        .map_err(row::error)?
        .map(|r| decode(&r))
        .transpose()
}
async fn list_sql(
    c: &Connection,
    sql: &str,
    p: impl turso::IntoParams,
) -> Result<Vec<ZoneRecord>, PersistenceError> {
    let mut rs = c.query(sql, p).await.map_err(row::error)?;
    let mut out = Vec::new();
    while let Some(r) = rs.next().await.map_err(row::error)? {
        out.push(decode(&r)?)
    }
    Ok(out)
}

#[async_trait]
impl ZoneRepository for TursoAdapter {
    async fn list(&self, t: &TenantId) -> Result<Vec<ZoneRecord>, PersistenceError> {
        list_sql(&self.database.connect()?,"SELECT id,tenant_id,name,description,geometry_type,geometry_json,color,created_at,updated_at FROM zones WHERE tenant_id=?1 ORDER BY name,id",params![t.as_str()]).await
    }
    async fn list_all(&self) -> Result<Vec<ZoneRecord>, PersistenceError> {
        list_sql(&self.database.connect()?,"SELECT id,tenant_id,name,description,geometry_type,geometry_json,color,created_at,updated_at FROM zones ORDER BY tenant_id,name,id",()).await
    }
    async fn get(&self, t: &TenantId, id: &str) -> Result<Option<ZoneRecord>, PersistenceError> {
        get_from(&self.database.connect()?, t, id).await
    }
    async fn create(&self, t: &TenantId, r: NewZoneRecord) -> Result<ZoneRecord, PersistenceError> {
        let w = self.database.writer().await;
        let now = chrono::Utc::now().timestamp_micros();
        w.execute("INSERT INTO zones(id,tenant_id,name,description,geometry_type,geometry_json,color,created_at,updated_at)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",params![r.id.clone(),t.as_str(),r.name,r.description,r.geometry_type,serde_json::to_string(&r.geometry_json).map_err(|e|PersistenceError::Internal(e.to_string()))?,r.color,now]).await.map_err(row::error)?;
        get_from(&w, t, &r.id)
            .await?
            .ok_or(PersistenceError::NotFound)
    }
    async fn update(
        &self,
        t: &TenantId,
        id: &str,
        r: UpdateZoneRecord,
    ) -> Result<Option<ZoneRecord>, PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        if tx
            .execute(
                "UPDATE zones SET updated_at=?3 WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id, r.updated_at.and_utc().timestamp_micros()],
            )
            .await
            .map_err(row::error)?
            == 0
        {
            tx.rollback().await.map_err(row::error)?;
            return Ok(None);
        }
        if let Some(v) = r.name {
            tx.execute(
                "UPDATE zones SET name=?3 WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id, v],
            )
            .await
            .map_err(row::error)?;
        }
        if let Some(v) = r.description {
            tx.execute(
                "UPDATE zones SET description=?3 WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id, v],
            )
            .await
            .map_err(row::error)?;
        }
        if let Some(v) = r.geometry_type {
            tx.execute(
                "UPDATE zones SET geometry_type=?3 WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id, v],
            )
            .await
            .map_err(row::error)?;
        }
        if let Some(v) = r.geometry_json {
            tx.execute(
                "UPDATE zones SET geometry_json=?3 WHERE tenant_id=?1 AND id=?2",
                params![
                    t.as_str(),
                    id,
                    serde_json::to_string(&v)
                        .map_err(|e| PersistenceError::Internal(e.to_string()))?
                ],
            )
            .await
            .map_err(row::error)?;
        }
        if let Some(v) = r.color {
            tx.execute(
                "UPDATE zones SET color=?3 WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id, v],
            )
            .await
            .map_err(row::error)?;
        }
        let out = get_from(&tx, t, id).await?;
        tx.commit().await.map_err(row::error)?;
        Ok(out)
    }
    async fn delete(&self, t: &TenantId, id: &str) -> Result<DeleteZoneOutcome, PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        if get_from(&tx, t, id).await?.is_none() {
            tx.rollback().await.map_err(row::error)?;
            return Ok(DeleteZoneOutcome::NotFound);
        }
        let mut rs = tx
            .query(
                "SELECT count(*) FROM rule_conditions WHERE tenant_id=?1 AND zone_id=?2",
                params![t.as_str(), id],
            )
            .await
            .map_err(row::error)?;
        let count = rs
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(row::error)?;
        drop(rs);
        if count > 0 {
            tx.rollback().await.map_err(row::error)?;
            return Ok(DeleteZoneOutcome::InUse);
        }
        tx.execute(
            "DELETE FROM zones WHERE tenant_id=?1 AND id=?2",
            params![t.as_str(), id],
        )
        .await
        .map_err(row::error)?;
        tx.commit().await.map_err(row::error)?;
        Ok(DeleteZoneOutcome::Deleted)
    }
}
