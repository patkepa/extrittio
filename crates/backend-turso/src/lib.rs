#![forbid(unsafe_code)]

pub mod api_keys;
mod ci_ingest;
pub use ci_ingest::TursoCiIngestRepository;
mod database;
pub use api_keys::TursoApiKeyRepository;
mod error;
mod lifecycle;
mod maintenance;
mod migrations;
pub mod roles;
mod row;
pub mod users;
mod zones;

pub use database::{TursoConnectionHandles, TursoDatabase};
pub use lifecycle::TursoLifecycleError;
pub use maintenance::{LogicalArchiveInfo, TursoBackupInfo, TursoDatabaseInfo};
pub use migrations::LATEST_SCHEMA_VERSION;
pub use roles::TursoRoleRepository;
pub use users::TursoUserRepository;
pub use zones::TursoZoneRepository;

/// Temporary compatibility exports for host repositories that still compile
/// outside this adapter. New code must use adapter-owned repositories instead.
#[cfg(feature = "migration-bridge")]
#[doc(hidden)]
pub mod migration_bridge {
    use extrittio_backend_core::PersistenceError;
    use tokio::sync::MutexGuard;
    use turso::Connection;

    pub use crate::TursoConnectionHandles;
    pub use crate::row::{datetime, i32, legacy_error};

    /// Open a legacy-host connection from the adapter-owned engine.
    pub fn connect(handles: &TursoConnectionHandles) -> Result<Connection, PersistenceError> {
        handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }

    /// Borrow the adapter-owned serialized writer for an unmigrated host
    /// repository. The guard cannot outlive the handle that owns the process
    /// lock.
    pub async fn lock_writer(handles: &TursoConnectionHandles) -> MutexGuard<'_, Connection> {
        handles.lock_writer().await
    }
}
