// Firmware service — business logic for firmware updates
pub use super::firmware_creation::{delete_stored_firmware, upload_blueprint_firmware};

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::state::ZenohMetrics;
use extrittio_backend_core::application::FirmwareApplication;

pub async fn trigger_ota(
    ctx: &RequestContext,
    application: &FirmwareApplication,
    zenoh_session: &std::sync::Arc<zenoh::Session>,
    device_id: &str,
    firmware_update_id: i32,
    public_url: &str,
    download_secret: &str,
    zenoh_metrics: &ZenohMetrics,
) -> Result<(), AppError> {
    let (delta, version) = application
        .trigger_ota(
            &ctx.tenant_context(),
            device_id,
            firmware_update_id,
            public_url,
            &crate::auth::SystemClock,
            &crate::domains::firmware::download::JwtFirmwareDownloadSigner(download_secret),
        )
        .await?;
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
