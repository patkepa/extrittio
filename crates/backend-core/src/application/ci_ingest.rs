use crate::{
    ApplicationError, CiIngestOutcome, CiIngestParams, CiIngestRepository, PersistenceError,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct CiIngestApplication {
    repository: Arc<dyn CiIngestRepository>,
}

impl CiIngestApplication {
    pub fn new(repository: Arc<dyn CiIngestRepository>) -> Self {
        Self { repository }
    }

    pub async fn ingest(
        &self,
        key_hash: &str,
        params: CiIngestParams,
    ) -> Result<(i32, String, String), ApplicationError> {
        let version = params.version.clone();
        let blueprint_revision_id = params.blueprint_revision_id.clone();
        let outcome = self.repository.ingest_ci(key_hash, params).await.map_err(
            |error| match error {
                PersistenceError::UniqueViolation { .. } => ApplicationError::Conflict(format!(
                    "Version '{version}' already exists for blueprint revision '{blueprint_revision_id}'"
                )),
                other => ApplicationError::Persistence(other),
            },
        )?;
        match outcome {
            CiIngestOutcome::UnsupportedFirmware => Err(ApplicationError::InvalidOperation(
                "The selected blueprint does not declare firmware update behavior".into(),
            )),
            CiIngestOutcome::Unauthorized => Err(ApplicationError::Unauthorized),
            CiIngestOutcome::BlueprintRevisionNotFound => Err(ApplicationError::NotFound(format!(
                "Blueprint revision '{blueprint_revision_id}' not found"
            ))),
            CiIngestOutcome::Forbidden {
                scoped_blueprint_id,
            } => Err(ApplicationError::Forbidden(format!(
                "API key is scoped to blueprint ID {scoped_blueprint_id}, not '{blueprint_revision_id}'"
            ))),
            CiIngestOutcome::Created {
                firmware_id,
                version,
                blueprint_revision_id,
            } => Ok((firmware_id, version, blueprint_revision_id)),
        }
    }
}
