use crate::commands::{CommandRepository, CommandResponseStatus};
use crate::{ApplicationError, Clock, TenantId};
use std::sync::Arc;

/// Device responses and system-wide timeout maintenance share core transition policy.
#[derive(Clone)]
pub struct CommandWorkerApplication {
    repository: Arc<dyn CommandRepository>,
    clock: Arc<dyn Clock>,
}
impl CommandWorkerApplication {
    pub fn new(repository: Arc<dyn CommandRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn handle_response(
        &self,
        tenant: &TenantId,
        device_id: &str,
        correlation_id: &str,
        device_status: &str,
        payload: Option<String>,
    ) -> Result<Option<String>, ApplicationError> {
        if correlation_id.is_empty() {
            return Ok(None);
        }
        Ok(self
            .repository
            .apply_response(
                tenant,
                device_id,
                correlation_id.to_owned(),
                CommandResponseStatus::from_device_status(device_status),
                payload,
                self.clock.now().naive_utc(),
            )
            .await?)
    }
    pub async fn timeout_stale(&self, timeout_secs: u64) -> Result<usize, ApplicationError> {
        let now = self.clock.now().naive_utc();
        let cutoff = i64::try_from(timeout_secs)
            .ok()
            .and_then(chrono::TimeDelta::try_seconds)
            .and_then(|duration| now.checked_sub_signed(duration))
            .ok_or_else(|| {
                ApplicationError::InvalidInput(
                    "Command timeout is outside the supported range".into(),
                )
            })?;
        Ok(self.repository.timeout_stale(cutoff, now).await?)
    }
}
