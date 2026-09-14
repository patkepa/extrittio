use super::require_permission;
use crate::shadows::{ShadowRecord, ShadowRepository};
use crate::{ApplicationError, Clock, Permission, TenantContext, TenantId};
use serde_json::{Map, Value};
use std::sync::Arc;

fn missing(device_id: &str) -> ApplicationError {
    ApplicationError::NotFound(format!("Shadow for device '{device_id}' not found"))
}

/// User-facing operations; authorization precedes validation and persistence.
#[derive(Clone)]
pub struct ShadowApplication {
    device: DeviceShadowApplication,
}
impl ShadowApplication {
    pub fn new(repository: Arc<dyn ShadowRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            device: DeviceShadowApplication::new(repository, clock),
        }
    }
    pub async fn get(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<ShadowRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadShadows)?;
        self.device.get(ctx.tenant_id(), device_id).await
    }
    /// Commit before returning the delta to the host for best-effort publication.
    pub async fn update_desired(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        patch: Map<String, Value>,
    ) -> Result<ShadowRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageShadows)?;
        if patch.contains_key("ota") {
            return Err(ApplicationError::InvalidInput(
                "The ota shadow key is reserved; use the firmware deployment endpoint".into(),
            ));
        }
        self.device
            .repository
            .update_desired(ctx.tenant_id(), device_id, patch, self.device.clock.now())
            .await?
            .ok_or_else(|| missing(device_id))
    }
    pub async fn update_reported(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        patch: Map<String, Value>,
    ) -> Result<ShadowRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageShadows)?;
        self.device
            .update_reported(ctx.tenant_id(), device_id, patch)
            .await
    }
    pub async fn reset(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<(), ApplicationError> {
        require_permission(ctx, Permission::ManageShadows)?;
        if self
            .device
            .repository
            .reset(ctx.tenant_id(), device_id, self.device.clock.now())
            .await?
        {
            Ok(())
        } else {
            Err(missing(device_id))
        }
    }
}

/// Narrow device-ingress operations. The host supplies an authenticated,
/// tenant-bound device identity; these operations grant no desired-state access.
#[derive(Clone)]
pub struct DeviceShadowApplication {
    repository: Arc<dyn ShadowRepository>,
    clock: Arc<dyn Clock>,
}
impl DeviceShadowApplication {
    pub fn new(repository: Arc<dyn ShadowRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<ShadowRecord, ApplicationError> {
        self.repository
            .get(tenant, device_id)
            .await?
            .ok_or_else(|| missing(device_id))
    }
    pub async fn update_reported(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
    ) -> Result<ShadowRecord, ApplicationError> {
        self.repository
            .update_reported(tenant, device_id, patch, self.clock.now())
            .await?
            .ok_or_else(|| missing(device_id))
    }
}
