use super::require_permission;
use crate::configuration::*;
use crate::{ApplicationError, Clock, Permission, TenantContext};
use serde_json::{Map, Value};
use std::sync::Arc;
#[derive(Clone)]
pub struct ConfigurationApplication {
    repository: Arc<dyn DeviceConfigRepository>,
    clock: Arc<dyn Clock>,
}
impl ConfigurationApplication {
    pub fn new(repository: Arc<dyn DeviceConfigRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn get(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<DeviceConfigRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        match self
            .repository
            .get_for_device(ctx.tenant_id(), device_id)
            .await?
        {
            GetDeviceConfigOutcome::DeviceNotFound => Err(ApplicationError::NotFound(format!(
                "Device '{device_id}' not found"
            ))),
            GetDeviceConfigOutcome::Found(config) => Ok(config),
        }
    }
    pub async fn merge(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        patch: Map<String, Value>,
    ) -> Result<DeviceConfigRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        match self
            .repository
            .merge_for_device(ctx.tenant_id(), device_id, patch, self.clock.now())
            .await?
        {
            MergeDeviceConfigOutcome::DeviceNotFound => Err(ApplicationError::NotFound(format!(
                "Device '{device_id}' not found"
            ))),
            MergeDeviceConfigOutcome::Updated(config) => Ok(config),
        }
    }
}
