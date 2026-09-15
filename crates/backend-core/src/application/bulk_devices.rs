use super::{Application, DeviceTargetSelection, require_permission};
use crate::{ApplicationError, Permission, TenantContext};

pub type DeviceBatchOutcomes = Vec<(String, Result<(), ApplicationError>)>;

/// Best-effort publication after a committed desired-state update. The host logs
/// transport failures; they do not turn a successful OTA mutation into a failure.
#[async_trait::async_trait]
pub trait DesiredDeltaPublisher: Send + Sync {
    async fn publish(&self, device_id: &str, delta: &serde_json::Value, version: i32);
}

impl Application {
    pub async fn assign_selected_devices(
        &self,
        ctx: &TenantContext,
        selection: DeviceTargetSelection<'_>,
        fleet_id: Option<i32>,
    ) -> Result<usize, ApplicationError> {
        let ids = self.devices.resolve_target_ids(ctx, selection).await?;
        self.devices.bulk_assign_fleet(ctx, ids, fleet_id).await
    }
    pub async fn delete_selected_devices(
        &self,
        ctx: &TenantContext,
        selection: DeviceTargetSelection<'_>,
    ) -> Result<usize, ApplicationError> {
        let ids = self.devices.resolve_target_ids(ctx, selection).await?;
        self.devices.bulk_delete(ctx, ids).await
    }
    pub async fn restart_selected_devices(
        &self,
        ctx: &TenantContext,
        selection: DeviceTargetSelection<'_>,
    ) -> Result<DeviceBatchOutcomes, ApplicationError> {
        self.commands.authorize_send(ctx)?;
        let ids = self.devices.resolve_target_ids(ctx, selection).await?;
        let mut outcomes = Vec::with_capacity(ids.len());
        for id in ids {
            let result = self
                .commands
                .send(ctx, &id, "restart", serde_json::json!({}))
                .await
                .map(|_| ());
            outcomes.push((id, result));
        }
        Ok(outcomes)
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "batch deployment coordinates explicit domain collaborators"
    )]
    pub async fn deploy_selected_devices(
        &self,
        ctx: &TenantContext,
        selection: DeviceTargetSelection<'_>,
        firmware_update_id: i32,
        public_url: &str,
        clock: &dyn crate::Clock,
        signer: &dyn crate::firmware::FirmwareDownloadSigner,
        publisher: &dyn DesiredDeltaPublisher,
    ) -> Result<DeviceBatchOutcomes, ApplicationError> {
        require_permission(ctx, Permission::DeployFirmware)?;
        let ids = self.devices.resolve_target_ids(ctx, selection).await?;
        let mut outcomes = Vec::with_capacity(ids.len());
        for id in ids {
            let result = match self
                .firmware
                .trigger_ota(ctx, &id, firmware_update_id, public_url, clock, signer)
                .await
            {
                Ok((delta, version)) => {
                    publisher.publish(&id, &delta, version).await;
                    Ok(())
                }
                Err(error) => Err(error),
            };
            outcomes.push((id, result));
        }
        Ok(outcomes)
    }
}
