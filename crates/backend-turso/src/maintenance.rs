use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use extrittio_backend_core::PersistenceError;
use sha2::{Digest, Sha256};
use turso::{Builder, Connection};

use crate::{TursoDatabase, TursoLifecycleError};

#[derive(Debug, Clone, serde::Serialize)]
pub struct TursoDatabaseInfo {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub schema_version: i64,
    pub integrity: &'static str,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TursoBackupInfo {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
    pub schema_version: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LogicalArchiveInfo {
    pub path: PathBuf,
    pub schema_version: i64,
    pub table_count: usize,
    pub row_count: usize,
    pub sha256: String,
}

const LOGICAL_TABLES: &[&str] = &[
    "organizations",
    "server_config",
    "device_blueprints",
    "device_blueprint_drafts",
    "device_blueprint_revisions",
    "device_contracts",
    "device_contract_assignments",
    "device_events",
    "device_event_receipts",
    "device_metric_samples",
    "device_metric_rollups_hourly",
    "device_metric_retention_state",
    "fleets",
    "devices",
    "users",
    "roles",
    "role_permissions",
    "user_roles",
    "api_keys",
    "ca_certificates",
    "device_certificates",
    "device_configs",
    "device_shadows",
    "device_logs",
    "command_history",
    "firmware_updates",
    "firmware_blobs",
    "ota_deployments",
    "zones",
    "rules",
    "rule_conditions",
    "rule_actions",
    "rule_cooldowns",
    "rule_zone_entries",
    "alerts",
    "rule_action_outbox",
    "rule_alert_deliveries",
    "audit_events",
    "server_metrics",
    "app_metrics",
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LogicalArchive {
    format: String,
    format_version: u32,
    schema_version: i64,
    tables: Vec<LogicalTable>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LogicalTable {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<LogicalValue>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum LogicalValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(String),
}

impl TursoDatabase {
    pub async fn info(&self) -> Result<TursoDatabaseInfo, PersistenceError> {
        self.integrity_check()
            .await
            .map_err(map_lifecycle_persistence)?;
        let connection = connect(self)?;
        let schema_version = scalar_i64(
            &connection,
            "SELECT COALESCE(max(version), 0) FROM _extrittio_migrations",
        )
        .await
        .map_err(map_unavailable)?;
        let size_bytes = std::fs::metadata(self.path())
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?
            .len();
        Ok(TursoDatabaseInfo {
            path: self.path().to_path_buf(),
            size_bytes,
            schema_version,
            integrity: "ok",
        })
    }

    pub async fn backup(&self, destination: &Path) -> Result<TursoBackupInfo, PersistenceError> {
        if destination == self.path() {
            return Err(PersistenceError::Unavailable(
                "backup path must differ from the active database path".into(),
            ));
        }
        if destination.exists() {
            return Err(PersistenceError::Unavailable(format!(
                "backup destination already exists: {}",
                destination.display()
            )));
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        }

        // Hold the single writer for the checkpoint and file copy so the
        // backup is a point-in-time image of this process's database.
        let handles = self.shared_handles();
        let writer = handles.lock_writer().await;
        consume_checkpoint(&writer).await.map_err(map_unavailable)?;
        let temporary = destination.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
        std::fs::copy(self.path(), &temporary)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let schema_version = match verify_database_file(&temporary).await {
            Ok(version) => version,
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(error);
            }
        };
        let (sha256, size_bytes) = checksum(&temporary)?;
        std::fs::rename(&temporary, destination)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        std::fs::write(
            manifest_path(destination),
            manifest_contents(&sha256, destination),
        )
        .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        drop(writer);
        Ok(TursoBackupInfo {
            path: destination.to_path_buf(),
            size_bytes,
            sha256,
            schema_version,
        })
    }

    pub async fn verify_backup(path: &Path) -> Result<TursoBackupInfo, PersistenceError> {
        let schema_version = verify_database_file(path).await?;
        let (sha256, size_bytes) = checksum(path)?;
        let manifest = manifest_path(path);
        if manifest.exists() {
            let expected = std::fs::read_to_string(manifest)
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            if expected != sha256 {
                return Err(PersistenceError::CorruptData(format!(
                    "backup checksum mismatch: expected {expected}, got {sha256}"
                )));
            }
        }
        Ok(TursoBackupInfo {
            path: path.to_path_buf(),
            size_bytes,
            sha256,
            schema_version,
        })
    }

    pub async fn restore_backup(
        data_dir: &Path,
        database_path: &Path,
        backup_path: &Path,
        force: bool,
    ) -> Result<TursoBackupInfo, PersistenceError> {
        if database_path == backup_path {
            return Err(PersistenceError::Unavailable(
                "backup path must differ from the database path".into(),
            ));
        }
        let verified = Self::verify_backup(backup_path).await?;
        std::fs::create_dir_all(data_dir)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let _lock =
            crate::database::open_process_lock(data_dir).map_err(map_lifecycle_persistence)?;
        if database_path.exists() && !force {
            return Err(PersistenceError::Unavailable(format!(
                "database already exists: {}; pass --force to preserve and replace it",
                database_path.display()
            )));
        }
        if let Some(parent) = database_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        }
        let temporary =
            database_path.with_extension(format!("restore-{}.tmp", uuid::Uuid::new_v4()));
        std::fs::copy(backup_path, &temporary)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        if let Err(error) = verify_database_file(&temporary).await {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }

        let recovery_path = database_path.with_extension(format!(
            "pre-restore-{}.db",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
        ));
        if database_path.exists() {
            std::fs::rename(database_path, &recovery_path)
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
            preserve_sidecar(database_path, &recovery_path, "-wal")?;
            preserve_sidecar(database_path, &recovery_path, "-shm")?;
        }
        if let Err(error) = std::fs::rename(&temporary, database_path) {
            if recovery_path.exists() {
                let _ = std::fs::rename(&recovery_path, database_path);
            }
            let _ = std::fs::remove_file(&temporary);
            return Err(PersistenceError::Unavailable(error.to_string()));
        }
        Ok(TursoBackupInfo {
            path: database_path.to_path_buf(),
            ..verified
        })
    }

    pub async fn export_logical(
        &self,
        path: &Path,
    ) -> Result<LogicalArchiveInfo, PersistenceError> {
        if path.exists() {
            return Err(PersistenceError::Unavailable(format!(
                "logical archive already exists: {}",
                path.display()
            )));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        }
        let connection = connect(self)?;
        let schema_version = scalar_i64(
            &connection,
            "SELECT COALESCE(max(version),0) FROM _extrittio_migrations",
        )
        .await
        .map_err(map_unavailable)?;
        let mut tables = Vec::with_capacity(LOGICAL_TABLES.len());
        let mut row_count = 0_usize;
        for name in LOGICAL_TABLES {
            let mut rows = connection
                .query(&format!("SELECT * FROM {name}"), ())
                .await
                .map_err(map_unavailable)?;
            let columns = rows.column_names();
            let mut archived = Vec::new();
            while let Some(row) = rows.next().await.map_err(map_unavailable)? {
                let mut values = Vec::with_capacity(columns.len());
                for index in 0..columns.len() {
                    values.push(match row.get_value(index).map_err(map_unavailable)? {
                        turso::Value::Null => LogicalValue::Null,
                        turso::Value::Integer(value) => LogicalValue::Integer(value),
                        turso::Value::Real(value) => LogicalValue::Real(value),
                        turso::Value::Text(value) => LogicalValue::Text(value),
                        turso::Value::Blob(value) => LogicalValue::Blob(BASE64.encode(value)),
                    });
                }
                archived.push(values);
                row_count += 1;
            }
            tables.push(LogicalTable {
                name: (*name).into(),
                columns,
                rows: archived,
            });
        }
        let archive = LogicalArchive {
            format: "extrittio-logical-archive".into(),
            format_version: 1,
            schema_version,
            tables,
        };
        let bytes = serde_json::to_vec(&archive)
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        std::fs::write(path, &bytes)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        Ok(LogicalArchiveInfo {
            path: path.to_path_buf(),
            schema_version,
            table_count: archive.tables.len(),
            row_count,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        })
    }

    pub async fn import_logical(
        &self,
        path: &Path,
        dry_run: bool,
    ) -> Result<LogicalArchiveInfo, PersistenceError> {
        let bytes = std::fs::read(path)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let archive: LogicalArchive = serde_json::from_slice(&bytes)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
        if archive.format != "extrittio-logical-archive" || archive.format_version != 1 {
            return Err(PersistenceError::CorruptData(
                "unsupported logical archive format".into(),
            ));
        }
        if archive.tables.len() != LOGICAL_TABLES.len()
            || archive
                .tables
                .iter()
                .zip(LOGICAL_TABLES)
                .any(|(actual, expected)| actual.name != *expected)
        {
            return Err(PersistenceError::CorruptData(
                "logical archive table set or ordering does not match this schema".into(),
            ));
        }
        let row_count = archive.tables.iter().map(|table| table.rows.len()).sum();
        let info = LogicalArchiveInfo {
            path: path.to_path_buf(),
            schema_version: archive.schema_version,
            table_count: archive.tables.len(),
            row_count,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        };
        if dry_run {
            return Ok(info);
        }

        let handles = self.shared_handles();
        let mut writer = handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_unavailable)?;
        transaction
            .execute_batch("PRAGMA defer_foreign_keys=ON;")
            .await
            .map_err(map_unavailable)?;
        for name in LOGICAL_TABLES.iter().rev() {
            transaction
                .execute(&format!("DELETE FROM {name}"), ())
                .await
                .map_err(map_unavailable)?;
        }
        for table in archive.tables {
            let placeholders = (1..=table.columns.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table.name,
                table.columns.join(","),
                placeholders
            );
            for values in table.rows {
                let params = values
                    .into_iter()
                    .map(|value| match value {
                        LogicalValue::Null => Ok(turso::Value::Null),
                        LogicalValue::Integer(value) => Ok(turso::Value::Integer(value)),
                        LogicalValue::Real(value) => Ok(turso::Value::Real(value)),
                        LogicalValue::Text(value) => Ok(turso::Value::Text(value)),
                        LogicalValue::Blob(value) => BASE64
                            .decode(value)
                            .map(turso::Value::Blob)
                            .map_err(|error| PersistenceError::CorruptData(error.to_string())),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                transaction
                    .execute(&sql, params)
                    .await
                    .map_err(map_unavailable)?;
            }
        }
        transaction.commit().await.map_err(map_unavailable)?;
        Ok(info)
    }
}

fn connect(database: &TursoDatabase) -> Result<Connection, PersistenceError> {
    database
        .shared_handles()
        .connect_raw()
        .map_err(map_unavailable)
}

fn preserve_sidecar(source: &Path, recovery: &Path, suffix: &str) -> Result<(), PersistenceError> {
    let source = sidecar_path(source, suffix);
    if source.exists() {
        let recovery = sidecar_path(recovery, suffix);
        std::fs::rename(source, recovery)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
    }
    Ok(())
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", path.display()))
}

async fn consume_checkpoint(connection: &Connection) -> Result<(), turso::Error> {
    let mut rows = connection
        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
        .await?;
    while rows.next().await?.is_some() {}
    Ok(())
}

fn manifest_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.sha256", path.display()))
}

fn manifest_contents(sha256: &str, backup_path: &Path) -> String {
    let backup_filename = backup_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("backup.db");
    format!("{sha256}  {backup_filename}\n")
}

fn checksum(path: &Path) -> Result<(String, u64), PersistenceError> {
    let mut file =
        File::open(path).map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        if count == 0 {
            break;
        }
        size += count as u64;
        digest.update(&buffer[..count]);
    }
    Ok((format!("{:x}", digest.finalize()), size))
}

async fn verify_database_file(path: &Path) -> Result<i64, PersistenceError> {
    let wal = sidecar_path(path, "-wal");
    if wal.exists()
        && std::fs::metadata(&wal)
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?
            .len()
            > 32
    {
        return Err(PersistenceError::CorruptData(
            "backup contains an uncheckpointed write-ahead log".into(),
        ));
    }
    let path_text = path.to_str().ok_or_else(|| {
        PersistenceError::Unavailable("Turso backup path is not valid UTF-8".into())
    })?;
    let database = Builder::new_local(path_text)
        .build()
        .await
        .map_err(map_unavailable)?;
    let connection = database.connect().map_err(map_unavailable)?;
    let mut rows = connection
        .query("PRAGMA integrity_check", ())
        .await
        .map_err(map_unavailable)?;
    let result: String = rows
        .next()
        .await
        .map_err(map_unavailable)?
        .ok_or_else(|| PersistenceError::CorruptData("integrity check returned no row".into()))?
        .get(0)
        .map_err(map_unavailable)?;
    drop(rows);
    if result != "ok" {
        return Err(PersistenceError::CorruptData(result));
    }
    let version = scalar_i64(
        &connection,
        "SELECT COALESCE(max(version), 0) FROM _extrittio_migrations",
    )
    .await
    .map_err(map_unavailable)?;
    drop(connection);
    drop(database);
    for suffix in ["-wal", "-shm"] {
        let sidecar = sidecar_path(path, suffix);
        if sidecar.exists() {
            std::fs::remove_file(sidecar)
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        }
    }
    Ok(version)
}

async fn scalar_i64(connection: &Connection, sql: &str) -> Result<i64, turso::Error> {
    let mut rows = connection.query(sql, ()).await?;
    rows.next()
        .await?
        .ok_or(turso::Error::QueryReturnedNoRows)?
        .get(0)
}

fn map_unavailable(error: turso::Error) -> PersistenceError {
    PersistenceError::Unavailable(error.to_string())
}

fn map_lifecycle_persistence(error: TursoLifecycleError) -> PersistenceError {
    match error {
        TursoLifecycleError::Unavailable(message) => PersistenceError::Unavailable(message),
        TursoLifecycleError::CorruptData(message) => PersistenceError::CorruptData(message),
        TursoLifecycleError::Migration(message) | TursoLifecycleError::Internal(message) => {
            PersistenceError::Internal(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::LATEST_SCHEMA_VERSION;

    #[tokio::test]
    async fn creates_verifies_and_restores_a_checkpointed_backup() {
        let source = tempfile::tempdir().unwrap();
        let source_path = source.path().join("extrittio.db");
        let backup_path = source.path().join("backups/extrittio.db");
        let database =
            TursoDatabase::open_and_migrate(source.path(), &source_path, Duration::from_secs(1))
                .await
                .unwrap();

        let backup = database.backup(&backup_path).await.unwrap();
        assert_eq!(backup.schema_version, LATEST_SCHEMA_VERSION);
        assert_eq!(backup.sha256.len(), 64);
        assert!(manifest_path(&backup_path).exists());
        let verified = TursoDatabase::verify_backup(&backup_path).await.unwrap();
        assert_eq!(verified.sha256, backup.sha256);

        let restored = tempfile::tempdir().unwrap();
        let restored_path = restored.path().join("extrittio.db");
        TursoDatabase::restore_backup(restored.path(), &restored_path, &backup_path, false)
            .await
            .unwrap();
        let reopened = TursoDatabase::open_and_migrate(
            restored.path(),
            &restored_path,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        reopened.integrity_check().await.unwrap();
        assert_eq!(
            reopened.info().await.unwrap().schema_version,
            LATEST_SCHEMA_VERSION
        );
    }

    #[tokio::test]
    async fn logical_archive_round_trip_restores_rows() {
        let source = tempfile::tempdir().unwrap();
        let source_path = source.path().join("extrittio.db");
        let archive_path = source.path().join("export.json");
        let database =
            TursoDatabase::open_and_migrate(source.path(), &source_path, Duration::from_secs(1))
                .await
                .unwrap();
        let handles = database.shared_handles();
        handles
            .lock_writer()
            .await
            .execute(
                "INSERT INTO server_config(key,value) VALUES('archive-test','preserved')",
                (),
            )
            .await
            .unwrap();
        let exported = database.export_logical(&archive_path).await.unwrap();
        assert!(exported.row_count > 0);
        database.import_logical(&archive_path, true).await.unwrap();

        let destination = tempfile::tempdir().unwrap();
        let destination_path = destination.path().join("extrittio.db");
        let restored = TursoDatabase::open_and_migrate(
            destination.path(),
            &destination_path,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        restored.import_logical(&archive_path, false).await.unwrap();
        let connection = connect(&restored).unwrap();
        let mut rows = connection
            .query(
                "SELECT value FROM server_config WHERE key='archive-test'",
                (),
            )
            .await
            .unwrap();
        assert_eq!(
            rows.next()
                .await
                .unwrap()
                .unwrap()
                .get::<String>(0)
                .unwrap(),
            "preserved"
        );
    }

    #[test]
    fn backup_manifest_wire_format_is_stable() {
        assert_eq!(
            manifest_contents(
                "0123456789abcdef",
                Path::new("/var/backups/extrittio-2026-08-31.db")
            ),
            "0123456789abcdef  extrittio-2026-08-31.db\n"
        );
    }
}
