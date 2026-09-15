//! API-key-authenticated CI firmware ingestion.
//!
//! The repository operation deliberately combines key lookup, usage bookkeeping,
//! tenant-scoped blueprint revision lookup, and insertion. Splitting these into separate
//! async calls would discard Turso's existing transaction boundary.
use crate::PersistenceError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct CiIngestParams {
    pub blueprint_revision_id: String,
    pub version: String,
    pub artifact_url: String,
    pub sha256: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub changelog: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CiIngestOutcome {
    Unauthorized,
    BlueprintRevisionNotFound,
    UnsupportedFirmware,
    Forbidden {
        scoped_blueprint_id: String,
    },
    Created {
        firmware_id: i32,
        version: String,
        blueprint_revision_id: String,
    },
}

/// Scope policy shared by both storage engines inside their ingest operation.
/// The key and tenant-owned revision must be resolved before this policy.
pub fn authorize_ci_blueprint(
    scope: Option<&str>,
    blueprint_id: &str,
) -> Result<(), CiIngestOutcome> {
    if let Some(scoped_blueprint_id) = scope
        && scoped_blueprint_id != blueprint_id
    {
        return Err(CiIngestOutcome::Forbidden {
            scoped_blueprint_id: scoped_blueprint_id.to_owned(),
        });
    }
    Ok(())
}

/// Resolve the tenant from the stored key hash; never accept a caller tenant.
#[async_trait]
pub trait CiIngestRepository: Send + Sync {
    async fn ingest_ci(
        &self,
        key_hash: &str,
        params: CiIngestParams,
    ) -> Result<CiIngestOutcome, PersistenceError>;
}

/// CI metadata comes from the published document, never from caller-supplied
/// compatibility claims.
pub fn ci_firmware_metadata(
    document: serde_json::Value,
) -> Result<Option<(serde_json::Value, Option<String>)>, PersistenceError> {
    let blueprint: extrittio_device_contract::DeviceBlueprint = serde_json::from_value(document)
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
    let Some(firmware) = blueprint.spec.firmware else {
        return Ok(None);
    };
    let compatibility = serde_json::to_value(firmware.compatibility)
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
    let strategy = serde_json::to_value(firmware.strategy)
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?
        .as_str()
        .map(str::to_owned);
    Ok(Some((compatibility, strategy)))
}
