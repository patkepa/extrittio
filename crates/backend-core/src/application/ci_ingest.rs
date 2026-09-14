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
        let device_type_name = params.device_type_name.clone();
        let outcome = self.repository.ingest_ci(key_hash, params).await.map_err(
            |error| match error {
                PersistenceError::UniqueViolation { .. } => ApplicationError::Conflict(format!(
                    "Version '{version}' already exists for device type '{device_type_name}'"
                )),
                other => ApplicationError::Persistence(other),
            },
        )?;
        match outcome {
            CiIngestOutcome::Unauthorized => Err(ApplicationError::Unauthorized),
            CiIngestOutcome::DeviceTypeNotFound => Err(ApplicationError::NotFound(format!(
                "Device type '{device_type_name}' not found"
            ))),
            CiIngestOutcome::Forbidden {
                scoped_device_type_id,
            } => Err(ApplicationError::Forbidden(format!(
                "API key is scoped to device type ID {scoped_device_type_id}, not '{device_type_name}'"
            ))),
            CiIngestOutcome::Created {
                firmware_id,
                version,
                device_type_name,
            } => Ok((firmware_id, version, device_type_name)),
        }
    }
}
