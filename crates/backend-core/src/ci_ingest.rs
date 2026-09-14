//! API-key-authenticated CI firmware ingestion.
//!
//! The repository operation deliberately combines key lookup, usage bookkeeping,
//! tenant-scoped device-type lookup, and insertion. Splitting these into separate
//! async calls would discard Turso's existing transaction boundary.
use crate::PersistenceError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct CiIngestParams {
    pub device_type_name: String,
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
    DeviceTypeNotFound,
    Forbidden {
        scoped_device_type_id: i32,
    },
    Created {
        firmware_id: i32,
        version: String,
        device_type_name: String,
    },
}

/// Scope policy shared by both storage engines inside their ingest operation.
/// A missing key or device type must be resolved before calling this policy.
pub fn authorize_ci_device_type(
    scope: Option<i32>,
    device_type_id: i32,
) -> Result<(), CiIngestOutcome> {
    if let Some(scoped_device_type_id) = scope
        && scoped_device_type_id != device_type_id
    {
        return Err(CiIngestOutcome::Forbidden {
            scoped_device_type_id,
        });
    }
    Ok(())
}

/// Resolve the tenant from the stored key hash; never accept a caller tenant.
///
/// Compatibility contract: unknown keys do not mutate data. Recognized keys are
/// touched before device-type/scope rejection. Turso commits those rejections
/// and rolls back on insert failure; PostgreSQL retains its best-effort touch
/// outside the firmware insert transaction. Concurrent deletion retains each
/// engine's existing semantics. A stronger cross-engine guarantee is separate
/// work, not an implicit consequence of extracting this port.
#[async_trait]
pub trait CiIngestRepository: Send + Sync {
    async fn ingest_ci(
        &self,
        key_hash: &str,
        params: CiIngestParams,
    ) -> Result<CiIngestOutcome, PersistenceError>;
}

#[cfg(test)]
#[async_trait]
impl CiIngestRepository for crate::api_keys::tests::RecordingRepository {
    async fn ingest_ci(
        &self,
        _key_hash: &str,
        _params: CiIngestParams,
    ) -> Result<CiIngestOutcome, PersistenceError> {
        panic!("unused by application composition tests")
    }
}
