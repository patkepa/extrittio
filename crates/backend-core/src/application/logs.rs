use super::require_permission;
use crate::logs::{LogQuery, LogRecord, LogRepository};
use crate::{ApplicationError, Clock, Permission, TenantContext, TenantId};
use std::sync::Arc;
#[derive(Clone)]
pub struct LogApplication {
    repository: Arc<dyn LogRepository>,
}
impl LogApplication {
    pub fn new(repository: Arc<dyn LogRepository>) -> Self {
        Self { repository }
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        mut query: LogQuery,
    ) -> Result<Vec<LogRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadLogs)?;
        query.limit = query.limit.clamp(1, 1000);
        query.level = query.level.map(|level| level.to_uppercase());
        self.repository
            .list(ctx.tenant_id(), device_id, query)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))
    }
}
#[derive(Clone)]
pub struct LogIngressApplication {
    repository: Arc<dyn LogRepository>,
    clock: Arc<dyn Clock>,
}
impl LogIngressApplication {
    pub fn new(repository: Arc<dyn LogRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn record(
        &self,
        tenant: &TenantId,
        device_id: &str,
        level: &str,
        message: &str,
    ) -> Result<bool, ApplicationError> {
        let level = level.to_uppercase();
        let level = if matches!(level.as_str(), "DEBUG" | "INFO" | "WARN" | "ERROR") {
            level
        } else {
            "INFO".into()
        };
        Ok(self
            .repository
            .record(
                tenant,
                device_id,
                level,
                message.to_owned(),
                self.clock.now().naive_utc(),
            )
            .await?)
    }
    /// System-wide retention; cutoff is exclusive and arithmetic cannot wrap.
    pub async fn prune(&self, retention_days: u64) -> Result<usize, ApplicationError> {
        let now = self.clock.now().naive_utc();
        let cutoff = i64::try_from(retention_days)
            .ok()
            .and_then(chrono::Duration::try_days)
            .and_then(|days| now.checked_sub_signed(days))
            .ok_or_else(|| {
                ApplicationError::InvalidInput(
                    "Log retention is outside the supported range".into(),
                )
            })?;
        Ok(self.repository.delete_older_than(cutoff).await?)
    }
}
