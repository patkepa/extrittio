use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::sync::MutexGuard;
use turso::{Builder, Connection, Database};

use crate::persistence::{DatabaseHealth, PersistenceError};

const BASELINE: &str = include_str!("../../../migrations/turso/0001_baseline.sql");

/// Local Turso lifecycle owner. Keeping the lock file alive enforces the
/// supported single-process deployment model; writes share one connection.
pub struct TursoDatabase {
    database: Arc<Database>,
    writer: Mutex<Connection>,
    _process_lock: File,
    path: PathBuf,
}

impl TursoDatabase {
    pub async fn open(
        data_dir: &Path,
        path: &Path,
        busy_timeout: Duration,
    ) -> Result<Arc<Self>, PersistenceError> {
        std::fs::create_dir_all(data_dir).map_err(|error| {
            PersistenceError::Unavailable(format!("cannot create data directory: {error}"))
        })?;
        let process_lock = OpenOptions::new()
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
                PersistenceError::Unavailable(format!(
                    "data directory is already in use or cannot be locked: {error}"
                ))
            })?;
        let path_text = path.to_str().ok_or_else(|| {
            PersistenceError::Unavailable("Turso database path is not valid UTF-8".to_string())
        })?;
        let database = Arc::new(
            Builder::new_local(path_text)
                .build()
                .await
                .map_err(map_unavailable)?,
        );
        let writer = database.connect().map_err(map_unavailable)?;
        writer.busy_timeout(busy_timeout).map_err(map_unavailable)?;
        writer
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")
            .await
            .map_err(map_unavailable)?;
        Ok(Arc::new(Self {
            database,
            writer: Mutex::new(writer),
            _process_lock: process_lock,
            path: path.to_path_buf(),
        }))
    }

    pub async fn migrate(&self) -> Result<(), PersistenceError> {
        let mut writer = self.writer.lock().await;
        writer
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS _extrittio_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at_us INTEGER NOT NULL
                );",
            )
            .await
            .map_err(map_migration)?;
        let applied = scalar_i64(
            &writer,
            "SELECT count(*) FROM _extrittio_migrations WHERE version = 1",
        )
        .await
        .map_err(map_migration)?;
        if applied == 0 {
            let transaction = writer.transaction().await.map_err(map_migration)?;
            transaction
                .execute_batch(BASELINE)
                .await
                .map_err(map_migration)?;
            transaction
                .execute(
                    "INSERT INTO _extrittio_migrations (version, applied_at_us) VALUES (1, unixepoch('subsec') * 1000000)",
                    (),
                )
                .await
                .map_err(map_migration)?;
            transaction.commit().await.map_err(map_migration)?;
        }
        Ok(())
    }

    pub async fn health(&self) -> Result<DatabaseHealth, PersistenceError> {
        let connection = self.database.connect().map_err(map_unavailable)?;
        scalar_i64(&connection, "SELECT 1")
            .await
            .map_err(map_unavailable)?;
        Ok(DatabaseHealth { reachable: true })
    }

    pub async fn integrity_check(&self) -> Result<(), PersistenceError> {
        let connection = self.database.connect().map_err(map_unavailable)?;
        let mut rows = connection
            .query("PRAGMA integrity_check", ())
            .await
            .map_err(map_unavailable)?;
        let row = rows.next().await.map_err(map_unavailable)?.ok_or_else(|| {
            PersistenceError::CorruptData("integrity check returned no row".into())
        })?;
        let result: String = row.get(0).map_err(map_unavailable)?;
        if result != "ok" {
            return Err(PersistenceError::CorruptData(result));
        }
        Ok(())
    }

    pub(crate) fn connect(&self) -> Result<Connection, PersistenceError> {
        self.database.connect().map_err(map_unavailable)
    }

    pub(crate) async fn writer(&self) -> MutexGuard<'_, Connection> {
        self.writer.lock().await
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

async fn scalar_i64(connection: &Connection, sql: &str) -> Result<i64, turso::Error> {
    let mut rows = connection.query(sql, ()).await?;
    let row = rows
        .next()
        .await?
        .ok_or(turso::Error::QueryReturnedNoRows)?;
    row.get(0)
}

fn map_unavailable(error: turso::Error) -> PersistenceError {
    PersistenceError::Unavailable(error.to_string())
}

fn map_migration(error: turso::Error) -> PersistenceError {
    PersistenceError::Migration(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn opens_migrates_and_reopens_a_local_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("extrittio.db");
        let database = TursoDatabase::open(directory.path(), &path, Duration::from_secs(1))
            .await
            .unwrap();
        database.migrate().await.unwrap();
        database.health().await.unwrap();
        database.integrity_check().await.unwrap();
        assert_eq!(database.path(), path);
        drop(database);

        let reopened = TursoDatabase::open(directory.path(), &path, Duration::from_secs(1))
            .await
            .unwrap();
        reopened.migrate().await.unwrap();
        reopened.integrity_check().await.unwrap();
    }
}
