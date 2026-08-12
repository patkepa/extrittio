use async_trait::async_trait;
use turso::params;

use crate::domains::dashboard::repository::DashboardReadRepository;
use crate::domains::dashboard::types::DashboardSummary;
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

#[async_trait]
impl DashboardReadRepository for TursoAdapter {
    async fn get_summary(&self, tenant: &TenantId) -> Result<DashboardSummary, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection.query("SELECT count(*), count(*) FILTER (WHERE status = 'online'), count(*) FILTER (WHERE status = 'offline'), (SELECT count(*) FROM telemetry WHERE tenant_id = ?1) FROM devices WHERE tenant_id = ?1", params![tenant.as_str()]).await.map_err(row::error)?;
        let record = rows
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?;
        Ok(DashboardSummary {
            total_devices: record.get(0).map_err(row::error)?,
            online_devices: record.get(1).map_err(row::error)?,
            offline_devices: record.get(2).map_err(row::error)?,
            total_messages: record.get(3).map_err(row::error)?,
        })
    }
}
