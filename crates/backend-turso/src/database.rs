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

/// Temporary P2 bridge shared by the host's legacy repositories and migrated
/// Turso repositories. Both handles must originate from the same local engine.
#[derive(Clone)]
pub struct TursoConnectionHandles {
    database: Arc<Database>,
    writer: Arc<Mutex<Connection>>,
}

impl TursoConnectionHandles {
    #[must_use]
    pub fn new(database: Arc<Database>, writer: Arc<Mutex<Connection>>) -> Self {
        Self { database, writer }
    }

    #[must_use]
    pub fn database(&self) -> Arc<Database> {
        self.database.clone()
    }

    #[must_use]
    pub fn writer(&self) -> Arc<Mutex<Connection>> {
        self.writer.clone()
    }

    pub(crate) fn connect(&self) -> Result<Connection, PersistenceError> {
        self.database.connect().map_err(map_persistence_open_error)
    }

    pub(crate) async fn lock_writer(&self) -> MutexGuard<'_, Connection> {
        self.writer.lock().await
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
    handles: TursoConnectionHandles,
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
                handles: TursoConnectionHandles::new(database, Arc::new(Mutex::new(writer))),
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
        let mut writer = self.inner.handles.lock_writer().await;
        migrations::run(&mut writer).await
    }

    pub async fn health(&self) -> Result<(), TursoLifecycleError> {
        let connection = self
            .inner
            .handles
            .database
            .connect()
            .map_err(map_operation_error)?;
        scalar_i64(&connection, "SELECT 1").await?;
        Ok(())
    }

    pub async fn integrity_check(&self) -> Result<(), TursoLifecycleError> {
        let connection = self
            .inner
            .handles
            .database
            .connect()
            .map_err(map_operation_error)?;
        let mut rows = connection
            .query("PRAGMA integrity_check", ())
            .await
            .map_err(map_operation_error)?;
        let row = rows
            .next()
            .await
            .map_err(map_operation_error)?
            .ok_or_else(|| {
                TursoLifecycleError::CorruptData("integrity check returned no row".into())
            })?;
        let result: String = row.get(0).map_err(map_operation_error)?;
        if result != "ok" {
            return Err(TursoLifecycleError::CorruptData(result));
        }
        Ok(())
    }

    pub async fn checkpoint(&self) -> Result<(), TursoLifecycleError> {
        let writer = self.inner.handles.lock_writer().await;
        let mut rows = writer
            .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
            .await
            .map_err(map_operation_error)?;
        while rows.next().await.map_err(map_operation_error)?.is_some() {}
        Ok(())
    }

    pub async fn schema_version(&self) -> Result<i64, TursoLifecycleError> {
        let connection = self
            .inner
            .handles
            .database
            .connect()
            .map_err(map_operation_error)?;
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

    /// Clone raw handles for the temporary legacy-host migration bridge.
    #[must_use]
    pub fn shared_handles(&self) -> TursoConnectionHandles {
        self.inner.handles.clone()
    }
}

fn open_process_lock(data_dir: &Path) -> Result<File, TursoLifecycleError> {
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
