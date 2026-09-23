use async_trait::async_trait;
use turso::params;

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::dashboard::DashboardReadRepository;
use extrittio_backend_core::dashboard::DashboardSummary;

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoDashboardRepository {
    handles: TursoConnectionHandles,
}
impl TursoDashboardRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
}

#[async_trait]
impl DashboardReadRepository for TursoDashboardRepository {
    async fn get_summary(&self, tenant: &TenantId) -> Result<DashboardSummary, PersistenceError> {
        let connection = self
            .handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let mut rows = connection.query("SELECT count(*), count(*) FILTER (WHERE status = 'online'), count(*) FILTER (WHERE status = 'offline'), (SELECT count(*) FROM device_events WHERE tenant_id = ?1) FROM devices WHERE tenant_id = ?1", params![tenant.as_str()]).await.map_err(row::legacy_error)?;
        let record = rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?;
        Ok(DashboardSummary {
            total_devices: record.get(0).map_err(row::legacy_error)?,
            online_devices: record.get(1).map_err(row::legacy_error)?,
            offline_devices: record.get(2).map_err(row::legacy_error)?,
            total_messages: record.get(3).map_err(row::legacy_error)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn fresh_blueprint_database_loads_dashboard_without_legacy_telemetry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dashboard.db");
        let database =
            crate::TursoDatabase::open_and_migrate(directory.path(), &path, Duration::from_secs(1))
                .await
                .unwrap();
        let repository = TursoDashboardRepository::from_handles(database.shared_handles());
        let summary = repository
            .get_summary(&TenantId::new("default").unwrap())
            .await
            .unwrap();
        assert_eq!(summary.total_devices, 0);
        assert_eq!(summary.total_messages, 0);
    }
}
