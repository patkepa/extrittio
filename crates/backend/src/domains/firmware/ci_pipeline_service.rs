use crate::domains::firmware::port::FirmwareRepository;
use crate::domains::firmware::types::CiIngestOutcome;
pub use crate::domains::firmware::types::CiIngestParams;
use crate::error::AppError;
use crate::persistence::PersistenceError;

pub async fn ingest(
    repository: &dyn FirmwareRepository,
    key_hash: &str,
    params: CiIngestParams,
) -> Result<(i32, String, String), AppError> {
    let version = params.version.clone();
    let device_type_name = params.device_type_name.clone();
    let outcome = repository
        .ingest_ci(key_hash, params)
        .await
        .map_err(|error| match error {
            PersistenceError::UniqueViolation { .. } => AppError::Conflict(format!(
                "Version '{version}' already exists for device type '{device_type_name}'"
            )),
            other => AppError::Persistence(other),
        })?;
    match outcome {
        CiIngestOutcome::Unauthorized => Err(AppError::Unauthorized),
        CiIngestOutcome::DeviceTypeNotFound => Err(AppError::NotFound(format!(
            "Device type '{device_type_name}' not found"
        ))),
        CiIngestOutcome::Forbidden {
            scoped_device_type_id,
        } => Err(AppError::Forbidden(format!(
            "API key is scoped to device type ID {scoped_device_type_id}, not '{device_type_name}'"
        ))),
        CiIngestOutcome::Created {
            firmware_id,
            version,
            device_type_name,
        } => Ok((firmware_id, version, device_type_name)),
    }
}
