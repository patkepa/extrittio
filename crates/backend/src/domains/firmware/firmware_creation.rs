use sha2::{Digest, Sha256};

use crate::auth::context::RequestContext;
use crate::domains::firmware_store::FirmwareObjectStore;
use crate::error::AppError;
use extrittio_backend_core::application::FirmwareApplication;
use extrittio_backend_core::firmware::{FirmwareRecord, NewFirmwareRecord};

pub async fn upload_blueprint_firmware(
    ctx: &RequestContext,
    application: &FirmwareApplication,
    store: &FirmwareObjectStore,
    mut record: NewFirmwareRecord,
    filename: String,
    file_data: Vec<u8>,
) -> Result<FirmwareRecord, AppError> {
    record.sha256 = Some(format!("{:x}", Sha256::digest(&file_data)));
    application
        .upload(&ctx.tenant_context(), store, record, filename, file_data)
        .await
        .map_err(Into::into)
}

pub async fn delete_stored_firmware(
    ctx: &RequestContext,
    application: &FirmwareApplication,
    store: &FirmwareObjectStore,
    id: i32,
) -> Result<(), AppError> {
    if let Some(stored_backend) = application
        .delete_stored(&ctx.tenant_context(), store, id)
        .await?
    {
        tracing::error!(firmware_update_id = id, %stored_backend, configured_backend = store.backend(),
            "Firmware metadata deleted but object backend was not configured for cleanup");
    }
    Ok(())
}
