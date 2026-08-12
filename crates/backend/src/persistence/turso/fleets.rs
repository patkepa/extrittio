use async_trait::async_trait;
use chrono::Utc;
use turso::{Row, params};

use crate::domains::fleets::repository::FleetRepository;
use crate::domains::fleets::types::{CreateFleetRecord, FleetList, FleetRecord, FleetSummary};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

fn decode(record: &Row) -> Result<FleetRecord, PersistenceError> {
    Ok(FleetRecord {
        id: row::i32(record.get::<i64>(0).map_err(row::error)?, "fleets.id")?,
        name: record.get(1).map_err(row::error)?,
    })
}

#[async_trait]
impl FleetRepository for TursoAdapter {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<FleetList, PersistenceError> {
        let connection = self.database.connect()?;
        let mut count_rows = connection
            .query(
                "SELECT count(*) FROM fleets WHERE tenant_id = ?1",
                params![tenant.as_str()],
            )
            .await
            .map_err(row::error)?;
        let total = count_rows
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::error)?;
        let mut rows = connection
            .query(
                "SELECT f.id, f.name, count(d.id) FROM fleets f
                 LEFT JOIN devices d ON d.tenant_id = f.tenant_id AND d.fleet_id = f.id
                 WHERE f.tenant_id = ?1 GROUP BY f.id, f.name
                 ORDER BY f.name, f.id LIMIT ?2 OFFSET ?3",
                params![tenant.as_str(), limit, offset],
            )
            .await
            .map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            records.push(FleetSummary {
                fleet: decode(&record)?,
                device_count: record.get(2).map_err(row::error)?,
            });
        }
        Ok(FleetList { records, total })
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateFleetRecord,
    ) -> Result<FleetRecord, PersistenceError> {
        let writer = self.database.writer().await;
        let mut rows = writer
            .query(
                "INSERT INTO fleets (tenant_id, name, created_at) VALUES (?1, ?2, ?3)
                 RETURNING id, name",
                params![tenant.as_str(), record.name, Utc::now().timestamp_micros()],
            )
            .await
            .map_err(row::error)?;
        decode(
            &rows
                .next()
                .await
                .map_err(row::error)?
                .ok_or(PersistenceError::NotFound)?,
        )
    }

    async fn rename(
        &self,
        tenant: &TenantId,
        id: i32,
        name: String,
    ) -> Result<Option<FleetRecord>, PersistenceError> {
        let writer = self.database.writer().await;
        let mut rows = writer
            .query(
                "UPDATE fleets SET name = ?3 WHERE tenant_id = ?1 AND id = ?2 RETURNING id, name",
                params![tenant.as_str(), i64::from(id), name],
            )
            .await
            .map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError> {
        let writer = self.database.writer().await;
        writer
            .execute(
                "DELETE FROM fleets WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(id)],
            )
            .await
            .map(|count| count > 0)
            .map_err(row::error)
    }
}
