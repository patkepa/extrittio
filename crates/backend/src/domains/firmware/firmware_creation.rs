use sha2::{Digest, Sha256};

use super::firmware_service;
use crate::auth::context::RequestContext;
use crate::domains::device_blueprints::repository::DeviceBlueprintRepository;
use crate::domains::device_types::repository::DeviceTypeRepository;
use crate::domains::firmware::port::FirmwareRepository;
use crate::domains::firmware::types::{FirmwareRecord, NewFirmwareBlobRecord, NewFirmwareRecord};
use crate::domains::firmware_store::FirmwareObjectStore;
use crate::error::AppError;

#[cfg(test)]
#[path = "firmware_creation_tests.rs"]
mod tests;

pub struct PreparedBlueprintFirmware {
    device_type_id: i32,
    compatibility: serde_json::Value,
    update_strategy: Option<String>,
}

impl PreparedBlueprintFirmware {
    pub fn into_record(
        self,
        revision_id: String,
        version: String,
        url: String,
        sha256: Option<String>,
        description: Option<String>,
    ) -> NewFirmwareRecord {
        NewFirmwareRecord {
            device_type_id: self.device_type_id,
            version,
            url,
            sha256,
            description,
            commit_sha: None,
            branch: None,
            ci_run_url: None,
            build_timestamp: None,
            changelog: None,
            source: None,
            blueprint_revision_id: Some(revision_id),
            compatibility: self.compatibility,
            update_strategy: self.update_strategy,
        }
    }
}

pub async fn prepare_blueprint_firmware(
    ctx: &RequestContext,
    blueprints: &dyn DeviceBlueprintRepository,
    device_types: &dyn DeviceTypeRepository,
    revision_id: &str,
) -> Result<PreparedBlueprintFirmware, AppError> {
    let revision = crate::domains::device_blueprints::blueprint_service::get_revision(
        ctx,
        blueprints,
        revision_id,
    )
    .await?;
    let blueprint: extrittio_device_contract::DeviceBlueprint =
        serde_json::from_value(revision.document).map_err(|error| {
            AppError::Internal(format!(
                "Stored device blueprint revision is invalid: {error}"
            ))
        })?;
    let firmware_definition = blueprint.spec.firmware.ok_or_else(|| {
        AppError::UnprocessableEntity(
            "The selected blueprint does not declare firmware update behavior".into(),
        )
    })?;
    let update_strategy = serde_json::to_value(firmware_definition.strategy)?
        .as_str()
        .map(ToOwned::to_owned);
    let compatibility = serde_json::to_value(firmware_definition.compatibility)?;
    let compatibility_type =
        crate::services::device_type_service::resolve_for_device_creation(ctx, device_types, None)
            .await?;

    Ok(PreparedBlueprintFirmware {
        device_type_id: compatibility_type.id,
        compatibility,
        update_strategy,
    })
}

pub async fn upload_blueprint_firmware(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    store: &FirmwareObjectStore,
    mut record: NewFirmwareRecord,
    filename: String,
    file_data: Vec<u8>,
) -> Result<FirmwareRecord, AppError> {
    record.sha256 = Some(format!("{:x}", Sha256::digest(&file_data)));
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let size = file_data.len() as i32;
    let storage_key = store.allocate_key(ctx.tenant_id_str(), &filename);
    store.put(&storage_key, file_data).await.map_err(|error| {
        tracing::error!(%error, "Firmware object upload failed");
        AppError::Internal("Firmware storage is unavailable".to_string())
    })?;
    let result = firmware_service::create_with_repository(
        ctx,
        repository,
        record,
        Some(NewFirmwareBlobRecord {
            size,
            filename,
            storage_key: storage_key.clone(),
            storage_backend: store.backend().to_string(),
        }),
    )
    .await;
    if result.is_err()
        && let Err(cleanup_error) = store.delete(&storage_key).await
    {
        tracing::error!(%cleanup_error, "Failed to clean up unreferenced firmware object");
    }
    result
}

pub async fn delete_stored_firmware(
    ctx: &RequestContext,
    repository: &dyn FirmwareRepository,
    store: &FirmwareObjectStore,
    id: i32,
) -> Result<(), AppError> {
    let blob = firmware_service::delete_with_repository(ctx, repository, id).await?;

    if let Some(blob) = blob
        && let Some(storage_key) = blob.storage_key
    {
        if blob.storage_backend == store.backend() {
            if let Err(error) = store.delete(&storage_key).await {
                tracing::error!(
                    %error,
                    firmware_update_id = id,
                    "Firmware metadata deleted but object cleanup failed"
                );
            }
        } else {
            tracing::error!(
                firmware_update_id = id,
                stored_backend = %blob.storage_backend,
                configured_backend = store.backend(),
                "Firmware metadata deleted but object backend was not configured for cleanup"
            );
        }
    }

    Ok(())
}
