use crate::{PostgresExecutor, PostgresPool};
use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Text};
use extrittio_backend_core::dashboard::{DashboardReadRepository, DashboardSummary};
use extrittio_backend_core::{PersistenceError, TenantId};
#[derive(Clone)]
pub struct PostgresDashboardRepository {
    executor: PostgresExecutor,
}
impl PostgresDashboardRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
#[derive(QueryableByName)]
struct Counts {
    #[diesel(sql_type = BigInt)]
    total_devices: i64,
    #[diesel(sql_type = BigInt)]
    online_devices: i64,
    #[diesel(sql_type = BigInt)]
    offline_devices: i64,
    #[diesel(sql_type = BigInt)]
    total_messages: i64,
}
#[async_trait]
impl DashboardReadRepository for PostgresDashboardRepository {
    async fn get_summary(&self, tenant: &TenantId) -> Result<DashboardSummary, PersistenceError> {
        let tenant = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let counts = diesel::sql_query(
                    "SELECT count(*) AS total_devices,
                count(*) FILTER (WHERE status = 'online') AS online_devices,
                count(*) FILTER (WHERE status = 'offline') AS offline_devices,
                (SELECT count(*) FROM device_events WHERE tenant_id = $1) AS total_messages
                FROM devices WHERE tenant_id = $1",
                )
                .bind::<Text, _>(tenant)
                .get_result::<Counts>(connection)
                .map_err(crate::error::map_diesel_error)?;
                Ok(DashboardSummary {
                    total_devices: counts.total_devices,
                    online_devices: counts.online_devices,
                    offline_devices: counts.offline_devices,
                    total_messages: counts.total_messages,
                })
            })
            .await
    }
}
