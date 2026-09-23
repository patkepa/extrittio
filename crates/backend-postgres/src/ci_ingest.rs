use async_trait::async_trait;
use diesel::prelude::*;
use extrittio_backend_core::{
    CiIngestOutcome, CiIngestParams, CiIngestRepository, PersistenceError, authorize_ci_blueprint,
};

use crate::error::map_diesel_error;
use crate::models::{ApiKey, FirmwareUpdate, NewFirmwareUpdate};
use crate::schema::{api_keys, device_blueprint_revisions as revisions, firmware_updates};
use crate::{PostgresExecutor, PostgresPool};

#[derive(Clone)]
pub struct PostgresCiIngestRepository {
    executor: PostgresExecutor,
}

#[derive(Debug, thiserror::Error)]
enum CiIngestTransactionError {
    #[error(transparent)]
    Diesel(#[from] diesel::result::Error),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
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
                connection
                    .transaction::<CiIngestOutcome, CiIngestTransactionError, _>(|connection| {
                        let Some(key) = api_keys::table
                            .filter(api_keys::key_hash.eq(&key_hash))
                            .for_update()
                            .select(ApiKey::as_select())
                            .first(connection)
                            .optional()?
                        else {
                            return Ok(CiIngestOutcome::Unauthorized);
                        };
                        let Some((blueprint_id, document)) = revisions::table
                            .filter(revisions::tenant_id.eq(&key.tenant_id))
                            .filter(revisions::id.eq(&params.blueprint_revision_id))
                            .for_update()
                            .select((revisions::blueprint_id, revisions::document))
                            .first::<(String, serde_json::Value)>(connection)
                            .optional()?
                        else {
                            return Ok(CiIngestOutcome::BlueprintRevisionNotFound);
                        };
                        if let Err(outcome) =
                            authorize_ci_blueprint(key.blueprint_id.as_deref(), &blueprint_id)
                        {
                            return Ok(outcome);
                        }
                        let Some((compatibility, update_strategy)) =
                            extrittio_backend_core::ci_ingest::ci_firmware_metadata(document)?
                        else {
                            return Ok(CiIngestOutcome::UnsupportedFirmware);
                        };
                        let revision_id = params.blueprint_revision_id.clone();
                        let record = NewFirmwareUpdate {
                            tenant_id: key.tenant_id.clone(),
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
                            blueprint_revision_id: params.blueprint_revision_id,
                            compatibility,
                            update_strategy,
                        };
                        diesel::insert_into(firmware_updates::table)
                            .values(&record)
                            .execute(connection)?;
                        diesel::update(api_keys::table.filter(api_keys::id.eq(key.id)))
                            .set(api_keys::last_used_at.eq(diesel::dsl::now))
                            .execute(connection)?;
                        let firmware = firmware_updates::table
                            .filter(firmware_updates::tenant_id.eq(&key.tenant_id))
                            .filter(
                                firmware_updates::blueprint_revision_id
                                    .eq(&record.blueprint_revision_id),
                            )
                            .filter(firmware_updates::version.eq(&record.version))
                            .select(FirmwareUpdate::as_select())
                            .first(connection)?;
                        Ok(CiIngestOutcome::Created {
                            firmware_id: firmware.id,
                            version: firmware.version,
                            blueprint_revision_id: revision_id,
                        })
                    })
                    .map_err(|error| match error {
                        CiIngestTransactionError::Diesel(error) => map_diesel_error(error),
                        CiIngestTransactionError::Persistence(error) => error,
                    })
            })
            .await
    }
}
