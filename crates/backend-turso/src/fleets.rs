use async_trait::async_trait;
use chrono::Utc;
use turso::{Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::fleets::FleetRepository;
use extrittio_backend_core::fleets::{CreateFleetRecord, FleetList, FleetRecord, FleetSummary};

use crate::row::legacy_error as map_error;
use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoFleetRepository {
    handles: TursoConnectionHandles,
}
impl TursoFleetRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}

fn decode(record: &Row) -> Result<FleetRecord, PersistenceError> {
    Ok(FleetRecord {
        id: row::i32(record.get::<i64>(0).map_err(map_error)?, "fleets.id")?,
        name: record.get(1).map_err(map_error)?,
    })
}

#[async_trait]
impl FleetRepository for TursoFleetRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<FleetList, PersistenceError> {
        let connection = self.connect()?;
        let mut count_rows = connection
            .query(
                "SELECT count(*) FROM fleets WHERE tenant_id = ?1",
                params![tenant.as_str()],
            )
            .await
            .map_err(map_error)?;
        let total = count_rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(map_error)?;
        let mut rows = connection
            .query(
                "SELECT f.id, f.name, count(d.id) FROM fleets f
                 LEFT JOIN devices d ON d.tenant_id = f.tenant_id AND d.fleet_id = f.id
                 WHERE f.tenant_id = ?1 GROUP BY f.id, f.name
                 ORDER BY f.name COLLATE BINARY, f.id LIMIT ?2 OFFSET ?3",
                params![tenant.as_str(), limit, offset],
            )
            .await
            .map_err(map_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(map_error)? {
            records.push(FleetSummary {
                fleet: decode(&record)?,
                device_count: record.get(2).map_err(map_error)?,
            });
        }
        Ok(FleetList { records, total })
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateFleetRecord,
    ) -> Result<FleetRecord, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        let mut rows = writer
            .query(
                "INSERT INTO fleets (tenant_id, name, created_at) VALUES (?1, ?2, ?3)
                 RETURNING id, name",
                params![tenant.as_str(), record.name, Utc::now().timestamp_micros()],
            )
            .await
            .map_err(map_error)?;
        decode(
            &rows
                .next()
                .await
                .map_err(map_error)?
                .ok_or(PersistenceError::NotFound)?,
        )
    }

    async fn rename(
        &self,
        tenant: &TenantId,
        id: i32,
        name: String,
    ) -> Result<Option<FleetRecord>, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        let mut rows = writer
            .query(
                "UPDATE fleets SET name = ?3 WHERE tenant_id = ?1 AND id = ?2 RETURNING id, name",
                params![tenant.as_str(), i64::from(id), name],
            )
            .await
            .map_err(map_error)?;
        rows.next()
            .await
            .map_err(map_error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer
            .execute(
                "DELETE FROM fleets WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(id)],
            )
            .await
            .map(|count| count > 0)
            .map_err(map_error)
    }
}
