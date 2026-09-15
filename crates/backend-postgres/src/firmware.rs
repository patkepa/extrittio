use async_trait::async_trait;
use diesel::sql_types::Text;
use diesel::{Connection, OptionalExtension, QueryableByName, RunQueryDsl};

use crate::models::{
    FirmwareBlob, FirmwareUpdate, NewFirmwareBlob, NewFirmwareUpdate, NewOtaDeployment,
};
use extrittio_backend_core::firmware::FirmwareRepository;
use extrittio_backend_core::firmware::{
    FirmwareBlobRecord, FirmwarePage, FirmwareRecord, GlobalOtaDeploymentPage,
    GlobalOtaDeploymentRecord, LegacyFirmwareBlob, NewFirmwareBlobRecord, NewFirmwareRecord,
    OtaDeploymentPage, OtaDeploymentRecord, OtaStatusUpdate, TriggerOtaOutcome,
};

use crate::firmware_sql as firmware_repo;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::{DeviceIdentity, TenantId};

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresFirmwareRepository {
    executor: PostgresExecutor,
}
impl PostgresFirmwareRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
#[derive(Debug, thiserror::Error)]
enum FirmwareTransactionError {
    #[error(transparent)]
    Database(#[from] diesel::result::Error),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}
use crate::error::map_diesel_error;

fn map_app_error(error: FirmwareTransactionError) -> PersistenceError {
    match error {
        FirmwareTransactionError::Database(error) => map_diesel_error(error),
        FirmwareTransactionError::Persistence(error) => error,
    }
}

fn firmware_record(
    firmware: FirmwareUpdate,
    device_type_name: String,
    file_size: Option<i32>,
    filename: Option<String>,
) -> FirmwareRecord {
    FirmwareRecord {
        id: firmware.id,
        device_type_id: firmware.device_type_id,
        device_type_name,
        version: firmware.version,
        url: firmware.url,
        sha256: firmware.sha256,
        description: firmware.description,
        created_at: firmware.created_at,
        file_size,
        filename,
        commit_sha: firmware.commit_sha,
        branch: firmware.branch,
        ci_run_url: firmware.ci_run_url,
        build_timestamp: firmware.build_timestamp,
        changelog: firmware.changelog,
        source: firmware.source,
        blueprint_revision_id: firmware.blueprint_revision_id,
        compatibility: firmware.compatibility,
        update_strategy: firmware.update_strategy,
    }
}

fn blob_record(blob: FirmwareBlob) -> FirmwareBlobRecord {
    FirmwareBlobRecord {
        data: blob.data,
        size: blob.size,
        filename: blob.filename,
        storage_key: blob.storage_key,
        storage_backend: blob.storage_backend,
    }
}

#[derive(QueryableByName)]
struct AssignedBlueprintRevision {
    #[diesel(sql_type = Text)]
    blueprint_revision_id: String,
}

fn assigned_blueprint_revision(
    connection: &mut diesel::PgConnection,
    tenant_id: &str,
    device_id: &str,
) -> Result<Option<String>, diesel::result::Error> {
    diesel::sql_query(
        "SELECT contract.blueprint_revision_id
         FROM device_contract_assignments assignment
         JOIN device_contracts contract
           ON contract.tenant_id = assignment.tenant_id
          AND contract.id = assignment.desired_contract_id
         WHERE assignment.tenant_id = $1 AND assignment.device_id = $2",
    )
    .bind::<Text, _>(tenant_id)
    .bind::<Text, _>(device_id)
    .get_result::<AssignedBlueprintRevision>(connection)
    .optional()
    .map(|row| row.map(|row| row.blueprint_revision_id))
}

fn update_desired_shadow(
    connection: &mut diesel::PgConnection,
    tenant_id: &str,
    device_id: &str,
    patch: &serde_json::Map<String, serde_json::Value>,
) -> Result<(serde_json::Value, i32), FirmwareTransactionError> {
    let current = crate::shadows::lock_in_transaction(connection, tenant_id, device_id)?
        .ok_or(PersistenceError::NotFound)?;
    let updated =
        extrittio_backend_core::shadows::apply_desired_patch(current, patch, chrono::Utc::now())
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
    crate::shadows::store_in_transaction(connection, tenant_id, &updated)?;
    Ok((updated.delta, updated.version))
}

#[async_trait]
impl FirmwareRepository for PostgresFirmwareRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        device_type_id: Option<i32>,
        blueprint_revision_id: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<FirmwarePage, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                let (records, total) = firmware_repo::list_firmware_updates(
                    connection,
                    &tenant_id,
                    device_type_id,
                    blueprint_revision_id.as_deref(),
                    limit,
                    offset,
                )
                .map_err(map_diesel_error)?;
                Ok(FirmwarePage {
                    records: records
                        .into_iter()
                        .map(|(firmware, device_type, size, filename)| {
                            firmware_record(firmware, device_type.name, size, filename)
                        })
                        .collect(),
                    total,
                })
            })
            .await
    }

    async fn list_all_deployments(
        &self,
        tenant: &TenantId,
        status: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<GlobalOtaDeploymentPage, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                let (records, total) = firmware_repo::list_all_ota_deployments(
                    connection,
                    &tenant_id,
                    status.as_deref(),
                    limit,
                    offset,
                )
                .map_err(map_diesel_error)?;
                Ok(GlobalOtaDeploymentPage {
                    records: records
                        .into_iter()
                        .map(|(deployment, firmware, device, device_type, fleet)| {
                            GlobalOtaDeploymentRecord {
                                id: deployment.id,
                                device_id: device.id,
                                device_name: device.name,
                                device_status: device.status,
                                current_firmware: device.firmware,
                                device_type_id: device_type.id,
                                device_type_name: device_type.name,
                                fleet_id: fleet.as_ref().map(|fleet| fleet.id),
                                fleet_name: fleet.map(|fleet| fleet.name),
                                firmware_update_id: firmware.id,
                                firmware_version: firmware.version,
                                status: deployment.status,
                                error_message: deployment.error_message,
                                initiated_at: deployment.initiated_at,
                                completed_at: deployment.completed_at,
                            }
                        })
                        .collect(),
                    total,
                })
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: NewFirmwareRecord,
        blob: Option<NewFirmwareBlobRecord>,
    ) -> Result<Option<FirmwareRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let device_type = firmware_repo::find_device_type_by_id(
                            connection,
                            &tenant_id,
                            record.device_type_id,
                        )
                        .optional()?;
                        let Some(device_type) = device_type else {
                            return Ok(None);
                        };
                        let firmware = firmware_repo::insert_firmware_update(
                            connection,
                            &tenant_id,
                            &NewFirmwareUpdate {
                                tenant_id: tenant_id.clone(),
                                device_type_id: record.device_type_id,
                                version: record.version,
                                url: record.url,
                                description: record.description,
                                sha256: record.sha256,
                                commit_sha: record.commit_sha,
                                branch: record.branch,
                                ci_run_url: record.ci_run_url,
                                build_timestamp: record.build_timestamp,
                                changelog: record.changelog,
                                source: record.source,
                                blueprint_revision_id: record.blueprint_revision_id,
                                compatibility: record.compatibility,
                                update_strategy: record.update_strategy,
                            },
                        )?;
                        let (firmware, size, filename) = if let Some(blob) = blob {
                            firmware_repo::insert_firmware_blob(
                                connection,
                                &NewFirmwareBlob {
                                    firmware_update_id: firmware.id,
                                    tenant_id: tenant_id.clone(),
                                    data: None,
                                    size: blob.size,
                                    filename: blob.filename.clone(),
                                    storage_key: Some(blob.storage_key),
                                    storage_backend: blob.storage_backend,
                                },
                            )?;
                            let url = format!("/api/v1/firmware-updates/{}/download", firmware.id);
                            firmware_repo::update_firmware_url(
                                connection,
                                &tenant_id,
                                firmware.id,
                                &url,
                            )?;
                            let firmware = firmware_repo::find_firmware_update(
                                connection,
                                &tenant_id,
                                firmware.id,
                            )?;
                            (firmware, Some(blob.size), Some(blob.filename))
                        } else {
                            (firmware, None, None)
                        };
                        Ok(Some(firmware_record(
                            firmware,
                            device_type.name,
                            size,
                            filename,
                        )))
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn next_version(
        &self,
        tenant: &TenantId,
        device_type_id: i32,
    ) -> Result<String, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                firmware_repo::find_next_version(connection, &tenant_id, device_type_id)
                    .map(|version| {
                        version.map_or_else(
                            || "1.0.0".to_string(),
                            |v| extrittio_backend_core::firmware::increment_firmware_version(&v),
                        )
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn next_blueprint_version(
        &self,
        tenant: &TenantId,
        blueprint_revision_id: &str,
    ) -> Result<String, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let blueprint_revision_id = blueprint_revision_id.to_string();
        self.executor
            .run(move |connection| {
                firmware_repo::find_next_blueprint_version(
                    connection,
                    &tenant_id,
                    &blueprint_revision_id,
                )
                .map(|version| {
                    version.map_or_else(
                        || "1.0.0".to_string(),
                        |v| extrittio_backend_core::firmware::increment_firmware_version(&v),
                    )
                })
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_blob(
        &self,
        tenant: &TenantId,
        firmware_update_id: i32,
    ) -> Result<Option<FirmwareBlobRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                firmware_repo::find_optional_firmware_blob(
                    connection,
                    &tenant_id,
                    firmware_update_id,
                )
                .map(|blob| blob.map(blob_record))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        firmware_update_id: i32,
    ) -> Result<Option<Option<FirmwareBlobRecord>>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let blob = firmware_repo::find_optional_firmware_blob(
                            connection,
                            &tenant_id,
                            firmware_update_id,
                        )?;
                        let deleted = firmware_repo::delete_firmware_update(
                            connection,
                            &tenant_id,
                            firmware_update_id,
                        )?;
                        Ok(deleted.then(|| blob.map(blob_record)))
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn apply_ota_status(
        &self,
        identity: &DeviceIdentity,
        update: OtaStatusUpdate,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = identity.tenant_id_str().to_string();
        let device_id = identity.device_id().to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, FirmwareTransactionError, _>(|connection| {
                        use crate::schema::ota_deployments;
                        use diesel::prelude::*;
                        let shadow = crate::shadows::lock_in_transaction(
                            connection, &tenant_id, &device_id,
                        )?
                        .ok_or(PersistenceError::NotFound)?;
                        let previous = ota_deployments::table
                            .filter(ota_deployments::tenant_id.eq(&tenant_id))
                            .filter(ota_deployments::device_id.eq(&device_id))
                            .filter(ota_deployments::id.eq(update.deployment_id))
                            .select((ota_deployments::status, ota_deployments::firmware_update_id))
                            .first::<(String, i32)>(connection)
                            .optional()?;
                        let Some((status, firmware_id)) = previous else {
                            return Ok(false);
                        };
                        if update.firmware_update_id != Some(firmware_id)
                            || !extrittio_backend_core::firmware::ota_transition_allowed(
                                &status,
                                &update.status,
                            )
                        {
                            return Ok(false);
                        }
                        firmware_repo::update_ota_deployment_status(
                            connection,
                            &tenant_id,
                            update.deployment_id,
                            &update.status,
                            update.error_message.as_deref(),
                            update.completed_at,
                        )?;
                        if extrittio_backend_core::firmware::ota_status_is_terminal(&update.status)
                            && let Some(updated) =
                                extrittio_backend_core::shadows::clear_ota_for_deployment(
                                    shadow,
                                    i64::from(update.deployment_id),
                                    chrono::Utc::now(),
                                )
                                .map_err(|error| PersistenceError::CorruptData(error.to_string()))?
                        {
                            crate::shadows::store_in_transaction(connection, &tenant_id, &updated)?;
                        }
                        Ok(true)
                    })
                    .map_err(map_app_error)
            })
            .await
    }

    async fn list_device_deployments(
        &self,
        tenant: &TenantId,
        device_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Option<OtaDeploymentPage>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                if firmware_repo::find_device_for_tenant(connection, &tenant_id, &device_id)
                    .optional()
                    .map_err(map_diesel_error)?
                    .is_none()
                {
                    return Ok(None);
                }
                let (records, total) = firmware_repo::list_ota_deployments(
                    connection, &tenant_id, &device_id, limit, offset,
                )
                .map_err(map_diesel_error)?;
                Ok(Some(OtaDeploymentPage {
                    records: records
                        .into_iter()
                        .map(|(deployment, firmware)| OtaDeploymentRecord {
                            id: deployment.id,
                            device_id: deployment.device_id,
                            firmware_update_id: deployment.firmware_update_id,
                            firmware_version: firmware.version,
                            status: deployment.status,
                            error_message: deployment.error_message,
                            initiated_at: deployment.initiated_at,
                            completed_at: deployment.completed_at,
                        })
                        .collect(),
                    total,
                }))
            })
            .await
    }

    async fn trigger_ota(
        &self,
        tenant: &TenantId,
        device_id: &str,
        firmware_update_id: i32,
        public_url: &str,
    ) -> Result<TriggerOtaOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        let public_url = public_url.to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, FirmwareTransactionError, _>(|connection| {
                        let device = firmware_repo::find_device_for_tenant(
                            connection, &tenant_id, &device_id,
                        )
                        .optional()?;
                        let Some(device) = device else {
                            return Ok(TriggerOtaOutcome::DeviceNotFound);
                        };
                        let firmware = firmware_repo::find_firmware_update(
                            connection,
                            &tenant_id,
                            firmware_update_id,
                        )
                        .optional()?;
                        let Some(firmware) = firmware else {
                            return Ok(TriggerOtaOutcome::FirmwareNotFound);
                        };
                        if let Some(target_revision) = firmware.blueprint_revision_id.as_deref() {
                            if assigned_blueprint_revision(connection, &tenant_id, &device_id)?
                                .as_deref()
                                != Some(target_revision)
                            {
                                return Ok(TriggerOtaOutcome::Incompatible);
                            }
                        } else if firmware.device_type_id != device.device_type_id {
                            return Ok(TriggerOtaOutcome::Incompatible);
                        }

                        let Some(artifact) =
                            extrittio_backend_core::firmware::PreparedOtaArtifact::prepare(
                                &firmware.version,
                                firmware.sha256.as_deref(),
                                &firmware.url,
                                &public_url,
                            )
                        else {
                            return Ok(TriggerOtaOutcome::InvalidArtifact);
                        };
                        crate::shadows::lock_in_transaction(connection, &tenant_id, &device_id)?
                            .ok_or(PersistenceError::NotFound)?;
                        use crate::schema::ota_deployments;
                        use diesel::prelude::*;
                        diesel::update(
                            ota_deployments::table
                                .filter(ota_deployments::tenant_id.eq(&tenant_id))
                                .filter(ota_deployments::device_id.eq(&device_id))
                                .filter(ota_deployments::status.ne("success"))
                                .filter(ota_deployments::status.ne("failed")),
                        )
                        .set((
                            ota_deployments::status.eq("failed"),
                            ota_deployments::error_message.eq("Superseded by a new deployment"),
                            ota_deployments::completed_at.eq(chrono::Utc::now().naive_utc()),
                        ))
                        .execute(connection)?;
                        let deployment_id = firmware_repo::insert_ota_deployment(
                            connection,
                            &NewOtaDeployment {
                                tenant_id: tenant_id.clone(),
                                device_id: device_id.clone(),
                                firmware_update_id: firmware.id,
                            },
                        )?;
                        let patch = artifact.desired_patch(firmware.id, i64::from(deployment_id));
                        let (delta, version) =
                            update_desired_shadow(connection, &tenant_id, &device_id, &patch)?;
                        Ok(TriggerOtaOutcome::Ready { delta, version })
                    })
                    .map_err(map_app_error)
            })
            .await
    }

    async fn next_legacy_blob(&self) -> Result<Option<LegacyFirmwareBlob>, PersistenceError> {
        self.executor
            .run(move |connection| {
                firmware_repo::list_legacy_firmware_blobs(connection, 1)
                    .map_err(map_diesel_error)
                    .map(|mut blobs| {
                        blobs.pop().and_then(|blob| {
                            blob.data.map(|data| LegacyFirmwareBlob {
                                tenant_id: blob.tenant_id,
                                firmware_update_id: blob.firmware_update_id,
                                filename: blob.filename,
                                data,
                            })
                        })
                    })
            })
            .await
    }

    async fn mark_blob_migrated(
        &self,
        tenant_id: &str,
        firmware_update_id: i32,
        storage_backend: &str,
        storage_key: &str,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = tenant_id.to_string();
        let storage_backend = storage_backend.to_string();
        let storage_key = storage_key.to_string();
        self.executor
            .run(move |connection| {
                firmware_repo::move_firmware_blob_to_object_storage(
                    connection,
                    &tenant_id,
                    firmware_update_id,
                    &storage_backend,
                    &storage_key,
                )
                .map_err(map_diesel_error)
            })
            .await
    }
}
