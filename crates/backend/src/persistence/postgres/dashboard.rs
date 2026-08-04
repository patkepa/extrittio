use async_trait::async_trait;
use diesel::prelude::*;

use crate::db::schema::{devices, telemetry};
use crate::domains::dashboard::repository::DashboardReadRepository;
use crate::domains::dashboard::types::DashboardSummary;
use crate::persistence::error::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

#[async_trait]
impl DashboardReadRepository for PostgresAdapter {
    async fn get_summary(&self, tenant: &TenantId) -> Result<DashboardSummary, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let total_devices = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .count()
                    .get_result(connection)
                    .map_err(map_diesel_error)?;
                let online_devices = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .filter(devices::status.eq("online"))
                    .count()
                    .get_result(connection)
                    .map_err(map_diesel_error)?;
                let offline_devices = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .filter(devices::status.eq("offline"))
                    .count()
                    .get_result(connection)
                    .map_err(map_diesel_error)?;
                let total_messages = telemetry::table
                    .filter(telemetry::tenant_id.eq(&tenant_id))
                    .count()
                    .get_result(connection)
                    .map_err(map_diesel_error)?;

                Ok(DashboardSummary {
                    total_devices,
                    online_devices,
                    offline_devices,
                    total_messages,
                })
            })
            .await
    }
}
