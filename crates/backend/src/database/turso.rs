//! Compatibility facade over the adapter-owned Turso database foundation.
//!
//! The host still exposes its historical lifecycle result types while legacy
//! repositories migrate. Connection, migration, checkpoint, backup, and
//! logical-archive behavior are implemented by `backend-turso`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use extrittio_backend_turso::TursoConnectionHandles;

use crate::persistence::{DatabaseHealth, LifecycleError, PersistenceError};

pub use extrittio_backend_turso::{LogicalArchiveInfo, TursoBackupInfo, TursoDatabaseInfo};

pub struct TursoDatabase {
    adapter: extrittio_backend_turso::TursoDatabase,
    handles: TursoConnectionHandles,
}

impl TursoDatabase {
    pub async fn open(
        data_dir: &Path,
        path: &Path,
        busy_timeout: Duration,
    ) -> Result<Arc<Self>, LifecycleError> {
        let adapter = extrittio_backend_turso::TursoDatabase::open(data_dir, path, busy_timeout)
            .await
            .map_err(map_unavailable_lifecycle)?;
        let handles = adapter.shared_handles();
        Ok(Arc::new(Self { adapter, handles }))
    }

    pub async fn migrate(&self) -> Result<(), LifecycleError> {
        self.adapter
            .migrate()
            .await
            .map_err(map_migration_lifecycle)
    }

    pub async fn health(&self) -> Result<DatabaseHealth, LifecycleError> {
        self.adapter
            .health()
            .await
            .map_err(map_unavailable_lifecycle)?;
        Ok(DatabaseHealth { reachable: true })
    }

    pub async fn integrity_check(&self) -> Result<(), PersistenceError> {
        self.adapter
            .integrity_check()
            .await
            .map_err(map_integrity_persistence)
    }

    pub async fn checkpoint(&self) -> Result<(), LifecycleError> {
        self.adapter
            .checkpoint()
            .await
            .map_err(map_unavailable_lifecycle)
    }

    pub async fn info(&self) -> Result<TursoDatabaseInfo, PersistenceError> {
        self.adapter.info().await
    }

    pub async fn backup(&self, destination: &Path) -> Result<TursoBackupInfo, PersistenceError> {
        self.adapter.backup(destination).await
    }

    pub async fn verify_backup(path: &Path) -> Result<TursoBackupInfo, PersistenceError> {
        extrittio_backend_turso::TursoDatabase::verify_backup(path).await
    }

    pub async fn restore_backup(
        data_dir: &Path,
        database_path: &Path,
        backup_path: &Path,
        force: bool,
    ) -> Result<TursoBackupInfo, PersistenceError> {
        extrittio_backend_turso::TursoDatabase::restore_backup(
            data_dir,
            database_path,
            backup_path,
            force,
        )
        .await
    }

    pub async fn export_logical(
        &self,
        path: &Path,
    ) -> Result<LogicalArchiveInfo, PersistenceError> {
        self.adapter.export_logical(path).await
    }

    pub async fn import_logical(
        &self,
        path: &Path,
        dry_run: bool,
    ) -> Result<LogicalArchiveInfo, PersistenceError> {
        self.adapter.import_logical(path, dry_run).await
    }

    #[must_use]
    pub(crate) fn shared_handles(&self) -> TursoConnectionHandles {
        self.handles.clone()
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.adapter.path()
    }
}

fn map_unavailable_lifecycle(
    error: extrittio_backend_turso::TursoLifecycleError,
) -> LifecycleError {
    LifecycleError::Unavailable(lifecycle_message(error))
}

fn map_migration_lifecycle(error: extrittio_backend_turso::TursoLifecycleError) -> LifecycleError {
    LifecycleError::Migration(lifecycle_message(error))
}

fn map_integrity_persistence(
    error: extrittio_backend_turso::TursoLifecycleError,
) -> PersistenceError {
    match error {
        extrittio_backend_turso::TursoLifecycleError::Unavailable(message) => {
            PersistenceError::Unavailable(message)
        }
        extrittio_backend_turso::TursoLifecycleError::CorruptData(message) => {
            PersistenceError::CorruptData(message)
        }
        extrittio_backend_turso::TursoLifecycleError::Migration(message)
        | extrittio_backend_turso::TursoLifecycleError::Internal(message) => {
            PersistenceError::Unavailable(message)
        }
    }
}

fn lifecycle_message(error: extrittio_backend_turso::TursoLifecycleError) -> String {
    match error {
        extrittio_backend_turso::TursoLifecycleError::Unavailable(message)
        | extrittio_backend_turso::TursoLifecycleError::Migration(message)
        | extrittio_backend_turso::TursoLifecycleError::CorruptData(message)
        | extrittio_backend_turso::TursoLifecycleError::Internal(message) => message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use extrittio_backend_turso::TursoLifecycleError;

    #[test]
    fn preserves_historical_host_error_classification() {
        assert!(matches!(
            map_unavailable_lifecycle(TursoLifecycleError::CorruptData("open".into())),
            LifecycleError::Unavailable(message) if message == "open"
        ));
        assert!(matches!(
            map_migration_lifecycle(TursoLifecycleError::Unavailable("migration".into())),
            LifecycleError::Migration(message) if message == "migration"
        ));
        assert!(matches!(
            map_integrity_persistence(TursoLifecycleError::Internal("driver".into())),
            PersistenceError::Unavailable(message) if message == "driver"
        ));
        assert!(matches!(
            map_integrity_persistence(TursoLifecycleError::CorruptData("integrity".into())),
            PersistenceError::CorruptData(message) if message == "integrity"
        ));
    }
}
