use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use extrittio_backend_core::PersistenceError;
use tokio::sync::{Mutex, MutexGuard};
use turso::{Builder, Connection, Database};

use crate::error::map_open_error as map_persistence_open_error;
use crate::lifecycle::{TursoLifecycleError, map_open_error, map_operation_error};
use crate::migrations;

/// Cloneable connection lease shared by legacy and migrated repositories.
///
/// The private owner also contains the process lock, so a repository cannot
/// keep the engine or writer alive after accidentally releasing that lock.
#[derive(Clone)]
pub struct TursoConnectionHandles {
    inner: Arc<TursoDatabaseInner>,
}

impl TursoConnectionHandles {
    pub(crate) fn connect_raw(&self) -> Result<Connection, turso::Error> {
        self.inner.database.connect()
    }

    pub(crate) fn connect(&self) -> Result<Connection, PersistenceError> {
        self.connect_raw().map_err(map_persistence_open_error)
    }

    /// Lock the adapter's serialized writer without allowing the connection to
    /// outlive this handle (and therefore its process-lock lease).
    pub async fn lock_writer(&self) -> MutexGuard<'_, Connection> {
        self.inner.writer.lock().await
    }
}

/// Cloneable owner of a single-process local Turso database.
///
/// Clones share the engine, one serialized writer connection, and the process
/// lock. This lets migrated and legacy repositories share one database during
/// the incremental crate split without creating duplicate pools or writes.
#[derive(Clone)]
pub struct TursoDatabase {
    inner: Arc<TursoDatabaseInner>,
}

struct TursoDatabaseInner {
    database: Arc<Database>,
    writer: Mutex<Connection>,
    _process_lock: File,
    path: PathBuf,
}

impl TursoDatabase {
    /// Open a local database and acquire the data-directory process lock.
    pub async fn open(
        data_dir: &Path,
        path: &Path,
        busy_timeout: Duration,
    ) -> Result<Self, TursoLifecycleError> {
        std::fs::create_dir_all(data_dir).map_err(|error| {
            TursoLifecycleError::Unavailable(format!("cannot create data directory: {error}"))
        })?;
        let process_lock = open_process_lock(data_dir)?;
        let path_text = path.to_str().ok_or_else(|| {
            TursoLifecycleError::Unavailable("Turso database path is not valid UTF-8".into())
        })?;
        let database = Arc::new(
            Builder::new_local(path_text)
                .build()
                .await
                .map_err(map_open_error)?,
        );
        let writer = database.connect().map_err(map_open_error)?;
        writer.busy_timeout(busy_timeout).map_err(map_open_error)?;
        writer
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")
            .await
            .map_err(map_open_error)?;

        Ok(Self {
            inner: Arc::new(TursoDatabaseInner {
                database,
                writer: Mutex::new(writer),
                _process_lock: process_lock,
                path: path.to_path_buf(),
            }),
        })
    }

    /// Open a database and apply all embedded migrations before returning it.
    pub async fn open_and_migrate(
        data_dir: &Path,
        path: &Path,
        busy_timeout: Duration,
    ) -> Result<Self, TursoLifecycleError> {
        let database = Self::open(data_dir, path, busy_timeout).await?;
        database.migrate().await?;
        Ok(database)
    }

    /// Apply pending embedded migrations and reject changed applied assets.
    pub async fn migrate(&self) -> Result<(), TursoLifecycleError> {
        let mut writer = self.inner.writer.lock().await;
        migrations::run(&mut writer).await
    }

    pub async fn health(&self) -> Result<(), TursoLifecycleError> {
        let connection = self.inner.database.connect().map_err(map_operation_error)?;
        scalar_i64(&connection, "SELECT 1").await?;
        Ok(())
    }

    pub async fn integrity_check(&self) -> Result<(), TursoLifecycleError> {
        let connection = self.inner.database.connect().map_err(map_integrity_error)?;
        let mut rows = connection
            .query("PRAGMA integrity_check", ())
            .await
            .map_err(map_integrity_error)?;
        let row = rows
            .next()
            .await
            .map_err(map_integrity_error)?
            .ok_or_else(|| {
                TursoLifecycleError::CorruptData("integrity check returned no row".into())
            })?;
        let result: String = row.get(0).map_err(map_integrity_error)?;
        if result != "ok" {
            return Err(TursoLifecycleError::CorruptData(result));
        }
        Ok(())
    }

    pub async fn checkpoint(&self) -> Result<(), TursoLifecycleError> {
        let writer = self.inner.writer.lock().await;
        let mut rows = writer
            .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
            .await
            .map_err(map_operation_error)?;
        while rows.next().await.map_err(map_operation_error)?.is_some() {}
        Ok(())
    }

    pub async fn schema_version(&self) -> Result<i64, TursoLifecycleError> {
        let connection = self.inner.database.connect().map_err(map_operation_error)?;
        scalar_i64(
            &connection,
            "SELECT COALESCE(max(version), 0) FROM _extrittio_migrations",
        )
        .await
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// Clone a lease on the exact engine, writer, and process lock owned by
    /// this database.
    #[must_use]
    pub fn shared_handles(&self) -> TursoConnectionHandles {
        TursoConnectionHandles {
            inner: self.inner.clone(),
        }
    }
}

// Historically the host classified driver failures encountered during an
// integrity check as unavailable. Only a completed check that reports damage
// (or no result) is classified as corrupt data. Keep that distinction while
// the host compatibility facade delegates this operation to the adapter.
fn map_integrity_error(error: turso::Error) -> TursoLifecycleError {
    TursoLifecycleError::Unavailable(error.to_string())
}

pub(crate) fn open_process_lock(data_dir: &Path) -> Result<File, TursoLifecycleError> {
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(data_dir.join("extrittio.lock"))
        .and_then(|file| {
            file.try_lock().map_err(std::io::Error::from)?;
            Ok(file)
        })
        .map_err(|error| {
            TursoLifecycleError::Unavailable(format!(
                "data directory is already in use or cannot be locked: {error}"
            ))
        })
}

async fn scalar_i64(connection: &Connection, sql: &str) -> Result<i64, TursoLifecycleError> {
    let mut rows = connection
        .query(sql, ())
        .await
        .map_err(map_operation_error)?;
    rows.next()
        .await
        .map_err(map_operation_error)?
        .ok_or_else(|| TursoLifecycleError::CorruptData("query returned no rows".into()))?
        .get(0)
        .map_err(map_operation_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn opens_migrates_and_reopens_one_local_engine() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("extrittio.db");
        let database =
            TursoDatabase::open_and_migrate(directory.path(), &path, Duration::from_secs(1))
                .await
                .unwrap();
        database.health().await.unwrap();
        database.integrity_check().await.unwrap();
        assert_eq!(database.path(), path);
        drop(database);

        let reopened =
            TursoDatabase::open_and_migrate(directory.path(), &path, Duration::from_secs(1))
                .await
                .unwrap();
        reopened.integrity_check().await.unwrap();
    }

    #[tokio::test]
    async fn rejects_a_changed_applied_migration_checksum() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("extrittio.db");
        let database =
            TursoDatabase::open_and_migrate(directory.path(), &path, Duration::from_secs(1))
                .await
                .unwrap();
        database
            .shared_handles()
            .lock_writer()
            .await
            .execute(
                "UPDATE _extrittio_migrations SET checksum='changed' WHERE version=1",
                (),
            )
            .await
            .unwrap();
        assert!(matches!(
            database.migrate().await,
            Err(TursoLifecycleError::Migration(message)) if message.contains("checksum mismatch")
        ));
    }

    #[tokio::test]
    async fn rejects_a_second_owner_for_the_same_data_directory() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("extrittio.db");
        let _owner = TursoDatabase::open(directory.path(), &path, Duration::from_secs(1))
            .await
            .unwrap();
        assert!(matches!(
            TursoDatabase::open(directory.path(), &path, Duration::from_secs(1)).await,
            Err(TursoLifecycleError::Unavailable(message)) if message.contains("already in use")
        ));
    }

    #[tokio::test]
    async fn repository_handles_keep_the_process_lock_alive() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("extrittio.db");
        let database = TursoDatabase::open(directory.path(), &path, Duration::from_secs(1))
            .await
            .unwrap();
        let repository = crate::TursoZoneRepository::new(database);

        assert!(matches!(
            TursoDatabase::open(directory.path(), &path, Duration::from_secs(1)).await,
            Err(TursoLifecycleError::Unavailable(message)) if message.contains("already in use")
        ));

        drop(repository);
        TursoDatabase::open(directory.path(), &path, Duration::from_secs(1))
            .await
            .expect("dropping the final repository handle must release the process lock");
    }
}
