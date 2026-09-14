use super::require_permission;
use crate::metrics::{MetricsHistory, MetricsRepository, MetricsSnapshot};
use crate::{ApplicationError, Permission, TenantContext};
use chrono::{NaiveDateTime, Timelike};
use std::sync::Arc;

/// Server-wide operational metrics retain the existing administrative permission.
#[derive(Clone)]
pub struct MetricsApplication {
    repository: Arc<dyn MetricsRepository>,
}
impl MetricsApplication {
    pub fn new(repository: Arc<dyn MetricsRepository>) -> Self {
        Self { repository }
    }
    pub async fn current(&self, ctx: &TenantContext) -> Result<MetricsSnapshot, ApplicationError> {
        require_permission(ctx, Permission::ReadServerMetrics)?;
        Ok(self.repository.current().await?)
    }
    pub async fn history(
        &self,
        ctx: &TenantContext,
        since: NaiveDateTime,
        resolution_secs: i64,
    ) -> Result<MetricsHistory, ApplicationError> {
        // Keep the HTTP contract's validation-before-authorization ordering.
        if resolution_secs < 10 {
            return Err(ApplicationError::InvalidInput(
                "resolution must be >= 10".into(),
            ));
        }
        if resolution_secs.checked_mul(1_000_000).is_none() {
            return Err(ApplicationError::InvalidInput(
                "resolution exceeds the supported microsecond range".into(),
            ));
        }
        // Stored timestamps have microsecond precision. Round the lower bound up,
        // so a sub-microsecond request never includes an earlier stored sample.
        let remainder = since.nanosecond() % 1_000;
        let since = if remainder == 0 {
            Some(since)
        } else {
            since.checked_add_signed(chrono::TimeDelta::nanoseconds(i64::from(1_000 - remainder)))
        }
        .ok_or_else(|| {
            ApplicationError::InvalidInput("since exceeds the supported timestamp range".into())
        })?;
        require_permission(ctx, Permission::ReadServerMetrics)?;
        Ok(self.repository.history(since, resolution_secs).await?)
    }
}

/// Trusted process worker operations; runtime sampling and scheduling stay in the host.
#[derive(Clone)]
pub struct MetricsWorkerApplication {
    repository: Arc<dyn MetricsRepository>,
    clock: Arc<dyn crate::Clock>,
}
impl MetricsWorkerApplication {
    pub fn new(repository: Arc<dyn MetricsRepository>, clock: Arc<dyn crate::Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn record_system(
        &self,
        record: crate::metrics::NewSystemMetricRecord,
    ) -> Result<(), ApplicationError> {
        Ok(self.repository.insert_system(record).await?)
    }
    pub async fn record_app(
        &self,
        record: crate::metrics::NewAppMetricRecord,
    ) -> Result<(), ApplicationError> {
        Ok(self.repository.insert_app(record).await?)
    }
    pub async fn retain(&self, retention_hours: u64) -> Result<(usize, usize), ApplicationError> {
        let cutoff = i64::try_from(retention_hours.max(1))
            .ok()
            .and_then(chrono::TimeDelta::try_hours)
            .and_then(|duration| self.clock.now().naive_utc().checked_sub_signed(duration))
            .ok_or_else(|| {
                ApplicationError::InvalidInput(
                    "Metrics retention time range is outside the supported range".into(),
                )
            })?;
        Ok(self.repository.delete_before(cutoff).await?)
    }
}
