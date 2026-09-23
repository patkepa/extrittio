use super::require_permission;
use crate::firmware::{
    FirmwareBlobRecord, FirmwarePage, FirmwareRecord, FirmwareRepository, GlobalOtaDeploymentPage,
    NewFirmwareBlobRecord, NewFirmwareRecord, OtaDeploymentPage,
};
use crate::{ApplicationError, Permission, PersistenceError, TenantContext};
use std::sync::Arc;
#[derive(Clone)]
pub struct FirmwareApplication {
    repository: Arc<dyn FirmwareRepository>,
}
impl FirmwareApplication {
    pub fn new(repository: Arc<dyn FirmwareRepository>) -> Self {
        Self { repository }
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        blueprint_revision_id: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<FirmwarePage, ApplicationError> {
        require_permission(ctx, Permission::ReadFirmware)?;
        Ok(self
            .repository
            .list(ctx.tenant_id(), blueprint_revision_id, limit, offset)
            .await?)
    }

    pub async fn list_all_deployments(
        &self,
        ctx: &TenantContext,
        status: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<GlobalOtaDeploymentPage, ApplicationError> {
        require_permission(ctx, Permission::ReadFirmware)?;
        Ok(self
            .repository
            .list_all_deployments(ctx.tenant_id(), status, limit, offset)
            .await?)
    }

    pub async fn create(
        &self,
        ctx: &TenantContext,
        record: NewFirmwareRecord,
        blob: Option<NewFirmwareBlobRecord>,
    ) -> Result<FirmwareRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageFirmware)?;
        self.repository
            .create(ctx.tenant_id(), record, blob)
            .await
            .map_err(|error| match error {
                PersistenceError::UniqueViolation { .. } => ApplicationError::Conflict(
                    "Firmware version already exists for this blueprint revision".into(),
                ),
                other => ApplicationError::Persistence(other),
            })?
            .ok_or_else(|| ApplicationError::NotFound("Blueprint revision not found".into()))
    }

    pub async fn next_blueprint_version(
        &self,
        ctx: &TenantContext,
        blueprint_revision_id: &str,
    ) -> Result<String, ApplicationError> {
        require_permission(ctx, Permission::ReadFirmware)?;
        Ok(self
            .repository
            .next_blueprint_version(ctx.tenant_id(), blueprint_revision_id)
            .await?)
    }

    pub async fn get_blob(
        &self,
        ctx: &TenantContext,
        firmware_update_id: i32,
    ) -> Result<FirmwareBlobRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadFirmware)?;
        self.repository
            .get_blob(ctx.tenant_id(), firmware_update_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "Firmware blob for update {firmware_update_id} not found"
                ))
            })
    }

    pub async fn delete(
        &self,
        ctx: &TenantContext,
        firmware_update_id: i32,
    ) -> Result<Option<FirmwareBlobRecord>, ApplicationError> {
        require_permission(ctx, Permission::ManageFirmware)?;
        self.repository
            .delete(ctx.tenant_id(), firmware_update_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "Firmware update {firmware_update_id} not found"
                ))
            })
    }

    pub async fn list_device_deployments(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<OtaDeploymentPage, ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        self.repository
            .list_device_deployments(ctx.tenant_id(), device_id, limit, offset)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))
    }
}

impl FirmwareApplication {
    /// Store the object first; preserve the metadata error if compensation fails.
    pub async fn upload(
        &self,
        ctx: &TenantContext,
        store: &dyn crate::firmware::FirmwareObjectStorage,
        record: NewFirmwareRecord,
        filename: String,
        data: Vec<u8>,
    ) -> Result<FirmwareRecord, ApplicationError> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let size = data.len() as i32;
        require_permission(ctx, Permission::ManageFirmware)?;
        let key = store.allocate_key(ctx.tenant_id(), &filename);
        store
            .put(&key, data)
            .await
            .map_err(|_| ApplicationError::Internal("Firmware storage is unavailable".into()))?;
        let result = self
            .create(
                ctx,
                record,
                Some(NewFirmwareBlobRecord {
                    size,
                    filename,
                    storage_key: key.clone(),
                    storage_backend: store.backend().to_string(),
                }),
            )
            .await;
        if result.is_err() {
            // The host storage implementation logs a cleanup failure. It must not
            // replace the original metadata/authorization error.
            let _ = store.delete(&key).await;
        }
        result
    }

    /// Metadata deletion commits before best-effort object cleanup. A backend
    /// mismatch is returned only as a diagnostic; deletion still succeeds.
    pub async fn delete_stored(
        &self,
        ctx: &TenantContext,
        store: &dyn crate::firmware::FirmwareObjectStorage,
        id: i32,
    ) -> Result<Option<String>, ApplicationError> {
        if let Some(blob) = self.delete(ctx, id).await? {
            if blob.storage_backend == store.backend() {
                let _ = store.delete(&blob.storage_key).await;
            } else {
                return Ok(Some(blob.storage_backend));
            }
        }
        Ok(None)
    }
}

use crate::DeviceBlueprintApplication;
pub struct PreparedBlueprintFirmware {
    blueprint_revision_id: String,
    compatibility: serde_json::Value,
    update_strategy: Option<String>,
}

impl PreparedBlueprintFirmware {
    pub fn into_record(
        self,
        version: String,
        url: String,
        sha256: Option<String>,
        description: Option<String>,
    ) -> NewFirmwareRecord {
        NewFirmwareRecord {
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
            blueprint_revision_id: self.blueprint_revision_id,
            compatibility: self.compatibility,
            update_strategy: self.update_strategy,
        }
    }
}

impl FirmwareApplication {
    pub async fn prepare_blueprint(
        &self,
        ctx: &TenantContext,
        blueprints: &DeviceBlueprintApplication,
        revision_id: &str,
    ) -> Result<PreparedBlueprintFirmware, ApplicationError> {
        require_permission(ctx, Permission::ManageFirmware)?;
        let revision = blueprints.get_revision(ctx, revision_id).await?;
        let blueprint: extrittio_device_contract::DeviceBlueprint =
            serde_json::from_value(revision.document).map_err(|error| {
                ApplicationError::Internal(format!(
                    "Stored device blueprint revision is invalid: {error}"
                ))
            })?;
        let firmware_definition = blueprint.spec.firmware.ok_or_else(|| {
            ApplicationError::InvalidOperation(
                "The selected blueprint does not declare firmware update behavior".into(),
            )
        })?;
        let update_strategy = serde_json::to_value(firmware_definition.strategy)
            .map_err(|error| ApplicationError::Internal(error.to_string()))?
            .as_str()
            .map(ToOwned::to_owned);
        let compatibility = serde_json::to_value(firmware_definition.compatibility)
            .map_err(|error| ApplicationError::Internal(error.to_string()))?;

        Ok(PreparedBlueprintFirmware {
            blueprint_revision_id: revision_id.to_owned(),
            compatibility,
            update_strategy,
        })
    }
}

impl FirmwareApplication {
    pub async fn download(
        &self,
        ctx: &TenantContext,
        id: i32,
        store: &dyn crate::firmware::FirmwareObjectStorage,
    ) -> Result<crate::firmware::FirmwareDownload, ApplicationError> {
        let blob = self.get_blob(ctx, id).await?;
        Self::read_download(blob, store).await
    }
    pub async fn download_granted(
        &self,
        grant: &crate::firmware::VerifiedFirmwareDownload,
        store: &dyn crate::firmware::FirmwareObjectStorage,
    ) -> Result<crate::firmware::FirmwareDownload, ApplicationError> {
        let blob = self
            .repository
            .get_blob(grant.tenant(), grant.firmware_id())
            .await?
            .ok_or_else(|| ApplicationError::NotFound("Firmware not found".into()))?;
        Self::read_download(blob, store).await
    }
    async fn read_download(
        blob: FirmwareBlobRecord,
        store: &dyn crate::firmware::FirmwareObjectStorage,
    ) -> Result<crate::firmware::FirmwareDownload, ApplicationError> {
        if blob.storage_backend != store.backend() {
            return Err(ApplicationError::Internal(
                "Firmware storage configuration does not match stored metadata".into(),
            ));
        }
        let data = store
            .get(&blob.storage_key)
            .await
            .map_err(|_| ApplicationError::Internal("Firmware storage is unavailable".into()))?;
        if data.len() != blob.size as usize {
            return Err(ApplicationError::Internal(
                "Firmware object failed integrity validation".into(),
            ));
        }
        Ok(crate::firmware::FirmwareDownload {
            data,
            filename: blob.filename,
            size: blob.size,
        })
    }
}

impl FirmwareApplication {
    pub async fn trigger_ota(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        firmware_update_id: i32,
        public_url: &str,
        clock: &dyn crate::Clock,
        signer: &dyn crate::firmware::FirmwareDownloadSigner,
    ) -> Result<(serde_json::Value, i32), ApplicationError> {
        use crate::firmware::{FirmwareDownloadGrant, TriggerOtaOutcome};
        require_permission(ctx, Permission::DeployFirmware)?;
        let grant = FirmwareDownloadGrant::new(ctx.tenant_id().clone(), firmware_update_id, clock)?;
        let token = signer.sign(&grant)?;
        let download_url = format!(
            "{}/api/v1/ota-downloads/{token}",
            public_url.trim_end_matches('/')
        );
        match self.repository.trigger_ota(ctx.tenant_id(), device_id, firmware_update_id, &download_url).await? {
            TriggerOtaOutcome::DeviceNotFound => Err(ApplicationError::NotFound(format!("Device '{device_id}' not found"))),
            TriggerOtaOutcome::FirmwareNotFound => Err(ApplicationError::NotFound(format!("Firmware update {firmware_update_id} not found"))),
            TriggerOtaOutcome::Incompatible => Err(ApplicationError::InvalidInput("Firmware blueprint revision does not match the device contract".into())),
            TriggerOtaOutcome::InvalidArtifact => Err(ApplicationError::InvalidInput("Firmware requires a valid SHA-256, a version of at most 63 bytes and a download URL of at most 1023 bytes".into())),
            TriggerOtaOutcome::Ready { delta, version } => Ok((delta, version)),
        }
    }
}

#[cfg(test)]
mod download_tests {
    use super::*;

    struct Store;

    #[async_trait::async_trait]
    impl crate::firmware::FirmwareObjectStorage for Store {
        fn backend(&self) -> &str {
            "memory"
        }
        fn allocate_key(&self, _: &crate::TenantId, _: &str) -> String {
            unreachable!()
        }
        async fn get(&self, key: &str) -> Result<Vec<u8>, String> {
            assert_eq!(key, "object");
            Ok(vec![1, 2, 3])
        }
        async fn put(&self, _: &str, _: Vec<u8>) -> Result<(), String> {
            unreachable!()
        }
        async fn delete(&self, _: &str) -> Result<(), String> {
            unreachable!()
        }
    }

    fn blob(key: &str, backend: &str, size: i32) -> FirmwareBlobRecord {
        FirmwareBlobRecord {
            size,
            filename: "firmware.bin".into(),
            storage_key: key.to_owned(),
            storage_backend: backend.into(),
        }
    }

    #[test]
    fn downloads_require_object_storage_and_validate_size() {
        futures::executor::block_on(async {
            let result = FirmwareApplication::read_download(blob("object", "memory", 3), &Store)
                .await
                .unwrap();
            assert_eq!(result.data, vec![1, 2, 3]);
            for invalid in [
                blob("object", "different-store", 3),
                blob("object", "memory", 2),
            ] {
                assert!(
                    FirmwareApplication::read_download(invalid, &Store)
                        .await
                        .is_err()
                );
            }
        });
    }
}

/// OTA reports arrive with an authenticated device identity from the host.
#[derive(Clone)]
pub struct FirmwareReportApplication {
    repository: Arc<dyn FirmwareRepository>,
    clock: Arc<dyn crate::Clock>,
}
impl FirmwareReportApplication {
    pub fn new(repository: Arc<dyn FirmwareRepository>, clock: Arc<dyn crate::Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn process_report(
        &self,
        identity: &crate::DeviceIdentity,
        reported: &serde_json::Value,
    ) -> Result<(), ApplicationError> {
        use crate::firmware::OtaStatusUpdate;

        let Some(serde_json::Value::Object(ota)) = reported.get("ota") else {
            return Ok(());
        };
        let Some(status_raw) = ota.get("status").and_then(serde_json::Value::as_str) else {
            return Ok(());
        };
        let status = status_raw.to_lowercase();
        let Some(deployment_id) = ota
            .get("deployment_id")
            .and_then(serde_json::Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
            .filter(|id| *id > 0)
        else {
            return Ok(());
        };
        let firmware_update_id = ota
            .get("firmware_update_id")
            .and_then(serde_json::Value::as_i64)
            .and_then(|id| i32::try_from(id).ok());
        let error_message = ota
            .get("error")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let completed_at =
            crate::firmware::ota_status_is_terminal(&status).then(|| self.clock.now().naive_utc());
        self.repository
            .apply_ota_status(
                identity,
                OtaStatusUpdate {
                    deployment_id,
                    firmware_update_id,
                    status,
                    error_message,
                    completed_at,
                },
            )
            .await?;
        Ok(())
    }
}
