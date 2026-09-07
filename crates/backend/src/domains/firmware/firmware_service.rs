// Firmware service — business logic for firmware updates
pub use super::firmware_creation::{
    delete_stored_firmware, prepare_blueprint_firmware, upload_blueprint_firmware,
};

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::firmware::port::FirmwareRepository;
use crate::domains::firmware::types::{
    FirmwareBlobRecord, FirmwarePage, FirmwareRecord, GlobalOtaDeploymentPage,
    NewFirmwareBlobRecord, NewFirmwareRecord, OtaDeploymentPage, TriggerOtaOutcome,
};
use crate::error::AppError;
use crate::persistence::PersistenceError;
use crate::state::ZenohMetrics;

pub async fn list_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    device_type_id: Option<i32>,
    blueprint_revision_id: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<FirmwarePage, AppError> {
    policy::require(ctx, Permission::ReadFirmware)?;
    Ok(repository
        .list(
            ctx.tenant_id(),
            device_type_id,
            blueprint_revision_id,
            limit,
            offset,
        )
        .await?)
}

pub async fn list_all_deployments_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    status: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<GlobalOtaDeploymentPage, AppError> {
    policy::require(ctx, Permission::ReadFirmware)?;
    Ok(repository
        .list_all_deployments(ctx.tenant_id(), status, limit, offset)
        .await?)
}

pub async fn create_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    record: NewFirmwareRecord,
    blob: Option<NewFirmwareBlobRecord>,
) -> Result<FirmwareRecord, AppError> {
    policy::require(ctx, Permission::ManageFirmware)?;
    repository
        .create(ctx.tenant_id(), record, blob)
        .await
        .map_err(|error| match error {
            PersistenceError::UniqueViolation { .. } => {
                AppError::Conflict("Firmware version already exists for this device type".into())
            }
            other => AppError::Persistence(other),
        })?
        .ok_or_else(|| AppError::NotFound("Device type not found".into()))
}

pub async fn next_version_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    device_type_id: i32,
) -> Result<String, AppError> {
    policy::require(ctx, Permission::ReadFirmware)?;
    Ok(repository
        .next_version(ctx.tenant_id(), device_type_id)
        .await?)
}

pub async fn next_blueprint_version_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    blueprint_revision_id: &str,
) -> Result<String, AppError> {
    policy::require(ctx, Permission::ReadFirmware)?;
    Ok(repository
        .next_blueprint_version(ctx.tenant_id(), blueprint_revision_id)
        .await?)
}

pub async fn get_blob_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    firmware_update_id: i32,
) -> Result<FirmwareBlobRecord, AppError> {
    policy::require(ctx, Permission::ReadFirmware)?;
    repository
        .get_blob(ctx.tenant_id(), firmware_update_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Firmware blob for update {firmware_update_id} not found"
            ))
        })
}

pub async fn delete_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    firmware_update_id: i32,
) -> Result<Option<FirmwareBlobRecord>, AppError> {
    policy::require(ctx, Permission::ManageFirmware)?;
    repository
        .delete(ctx.tenant_id(), firmware_update_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("Firmware update {firmware_update_id} not found"))
        })
}

pub async fn list_device_deployments_with_repository(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    device_id: &str,
    limit: i64,
    offset: i64,
) -> Result<OtaDeploymentPage, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;
    repository
        .list_device_deployments(ctx.tenant_id(), device_id, limit, offset)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}

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
