// Firmware service — business logic for firmware updates
pub use super::firmware_creation::{delete_stored_firmware, upload_blueprint_firmware};

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::firmware::port::FirmwareRepository;
use crate::domains::firmware::types::TriggerOtaOutcome;
use crate::error::AppError;
use crate::state::ZenohMetrics;

pub async fn trigger_ota_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    zenoh_session: &std::sync::Arc<zenoh::Session>,
    device_id: &str,
    firmware_update_id: i32,
    public_url: &str,
    download_secret: &str,
    zenoh_metrics: &ZenohMetrics,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::DeployFirmware)?;
    let token = crate::domains::firmware::download::issue(
        ctx.tenant_id(),
        firmware_update_id,
        download_secret,
    )?;
    let download_url = format!(
        "{}/api/v1/ota-downloads/{token}",
        public_url.trim_end_matches('/')
    );
    let outcome = repository
        .trigger_ota(
            ctx.tenant_id(),
            device_id,
            firmware_update_id,
            &download_url,
        )
        .await?;
    let (delta, version) = match outcome {
        TriggerOtaOutcome::DeviceNotFound => {
            return Err(AppError::NotFound(format!(
                "Device '{device_id}' not found"
            )));
        }
        TriggerOtaOutcome::FirmwareNotFound => {
            return Err(AppError::NotFound(format!(
                "Firmware update {firmware_update_id} not found"
            )));
        }
        TriggerOtaOutcome::Incompatible => {
            return Err(AppError::BadRequest(
                "Firmware device type does not match device".into(),
            ));
        }
        TriggerOtaOutcome::InvalidArtifact => {
            return Err(AppError::BadRequest("Firmware requires a valid SHA-256, a version of at most 63 bytes and a download URL of at most 1023 bytes".into()));
        }
        TriggerOtaOutcome::Ready { delta, version } => (delta, version),
    };
    crate::services::shadow_service::publish_delta_if_nonempty(
        zenoh_session,
        device_id,
        &delta,
        version,
        zenoh_metrics,
    )
    .await;
    Ok(())
}
