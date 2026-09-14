use async_trait::async_trait;
use diesel::prelude::*;
use extrittio_backend_core::{
    CiIngestOutcome, CiIngestParams, CiIngestRepository, PersistenceError, authorize_ci_device_type,
};

use crate::error::map_diesel_error;
use crate::models::{ApiKey, DeviceType, FirmwareUpdate, NewFirmwareUpdate};
use crate::schema::{api_keys, device_types, firmware_updates};
use crate::{PostgresExecutor, PostgresPool};

#[derive(Clone)]
pub struct PostgresCiIngestRepository {
    executor: PostgresExecutor,
}

impl PostgresCiIngestRepository {
    #[must_use]
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}

#[async_trait]
impl CiIngestRepository for PostgresCiIngestRepository {
    async fn ingest_ci(
        &self,
        key_hash: &str,
        params: CiIngestParams,
    ) -> Result<CiIngestOutcome, PersistenceError> {
        let key_hash = key_hash.to_owned();
        self.executor
            .run(move |connection| {
                let Some(key) = api_keys::table
                    .filter(api_keys::key_hash.eq(key_hash))
                    .select(ApiKey::as_select())
                    .first(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                else {
                    return Ok(CiIngestOutcome::Unauthorized);
                };
                // Preserve the legacy best-effort touch outside the insert transaction.
                let _ = diesel::update(api_keys::table.filter(api_keys::id.eq(key.id)))
                    .set(api_keys::last_used_at.eq(diesel::dsl::now))
                    .execute(connection);
                let Some(device_type) = device_types::table
                    .filter(device_types::tenant_id.eq(&key.tenant_id))
                    .filter(device_types::name.eq(&params.device_type_name))
                    .select(DeviceType::as_select())
                    .first(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                else {
                    return Ok(CiIngestOutcome::DeviceTypeNotFound);
                };
                if let Err(outcome) = authorize_ci_device_type(key.device_type_id, device_type.id) {
                    return Ok(outcome);
                }
                let record = NewFirmwareUpdate {
                    tenant_id: key.tenant_id.clone(),
                    device_type_id: device_type.id,
                    version: params.version,
                    url: params.artifact_url,
                    description: params.description,
                    sha256: params.sha256,
                    commit_sha: params.commit_sha,
                    branch: params.branch,
                    ci_run_url: params.ci_run_url,
                    build_timestamp: params.build_timestamp.map(|value| value.naive_utc()),
                    changelog: params.changelog,
                    source: Some("ci".to_string()),
                    blueprint_revision_id: None,
                    compatibility: serde_json::json!({}),
                    update_strategy: None,
                };
                // Keep the insert/read transaction and exact tenant/version lookup.
                let firmware = connection
                    .transaction::<FirmwareUpdate, diesel::result::Error, _>(|connection| {
                        diesel::insert_into(firmware_updates::table)
                            .values(&record)
                            .execute(connection)?;
                        firmware_updates::table
                            .filter(firmware_updates::tenant_id.eq(&key.tenant_id))
                            .filter(firmware_updates::device_type_id.eq(record.device_type_id))
                            .filter(firmware_updates::version.eq(&record.version))
                            .select(FirmwareUpdate::as_select())
                            .first(connection)
                    })
                    .map_err(map_diesel_error)?;
                Ok(CiIngestOutcome::Created {
                    firmware_id: firmware.id,
                    version: firmware.version,
                    device_type_name: device_type.name,
                })
            })
            .await
    }
}
