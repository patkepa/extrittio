use sha2::{Digest, Sha256};
use turso::Connection;

use crate::lifecycle::{TursoLifecycleError, map_migration_error};

const BASELINE: &str = include_str!("../migrations/0001_baseline.sql");
pub const LATEST_SCHEMA_VERSION: i64 = 1;
const MIGRATIONS: &[(i64, &str)] = &[(1, BASELINE)];

// This sizable migration-contract suite stays next to its embedded SQL inputs.
#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn blueprint_baseline_supports_scoped_keys_and_object_firmware() {
        use extrittio_backend_core::firmware::{
            FirmwareRepository, NewFirmwareBlobRecord, NewFirmwareRecord,
        };
        use extrittio_backend_core::{ApiKeyRepository, CreateApiKeyRecord, TenantId};
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("publishing.db"),
            std::time::Duration::from_secs(1),
        )
        .await
        .unwrap();
        database.migrate().await.unwrap();
        let connection = database.shared_handles().connect().unwrap();
        connection.execute_batch(
            "INSERT INTO device_blueprints VALUES ('blueprint','default','sensor','Sensor',NULL,0,0);
             INSERT INTO device_blueprint_revisions VALUES ('revision','default','blueprint',1,'{}',
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','{}',0);"
        ).await.unwrap();
        let tenant = TenantId::new("default").unwrap();
        let keys = crate::api_keys::TursoApiKeyRepository::from_handles(database.shared_handles());
        let key = keys
            .create(
                &tenant,
                CreateApiKeyRecord {
                    name: "CI".into(),
                    key_hash: "hash".into(),
                    key_prefix: "prefix".into(),
                    blueprint_id: Some("blueprint".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(key.blueprint_id.as_deref(), Some("blueprint"));
        // A blueprint referenced by a scoped key cannot disappear and widen its scope.
        assert!(
            database
                .shared_handles()
                .lock_writer()
                .await
                .execute("DELETE FROM device_blueprints WHERE id='blueprint'", ())
                .await
                .is_err()
        );
        let firmware =
            crate::firmware::TursoFirmwareRepository::from_handles(database.shared_handles());
        let record = firmware
            .create(
                &tenant,
                NewFirmwareRecord {
                    version: "1".into(),
                    url: String::new(),
                    sha256: Some("a".repeat(64)),
                    description: None,
                    commit_sha: None,
                    branch: None,
                    ci_run_url: None,
                    build_timestamp: None,
                    changelog: None,
                    source: None,
                    blueprint_revision_id: "revision".into(),
                    compatibility: serde_json::json!({}),
                    update_strategy: Some("partition_swap".into()),
                },
                Some(NewFirmwareBlobRecord {
                    size: 3,
                    filename: "fw.bin".into(),
                    storage_key: "object".into(),
                    storage_backend: "local".into(),
                }),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.file_size, Some(3));
        let blob = firmware
            .get_blob(&tenant, record.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(blob.storage_key, "object");
        assert_eq!(blob.filename, "fw.bin");
        assert_eq!(blob.size, 3);
        assert!(
            firmware
                .get_blob(&TenantId::new("other").unwrap(), record.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(firmware.delete(&tenant, record.id).await.unwrap().is_some());
        assert!(
            firmware
                .get_blob(&tenant, record.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn blueprint_baseline_initializes_and_reopens_without_retired_objects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("baseline.db");
        let database =
            crate::TursoDatabase::open(directory.path(), &path, std::time::Duration::from_secs(1))
                .await
                .unwrap();
        database.migrate().await.unwrap();
        database.migrate().await.unwrap();
        let connection = database.shared_handles().connect().unwrap();
        let mut rows = connection
            .query(
                "SELECT name,sql FROM sqlite_schema WHERE type='table' ORDER BY name",
                (),
            )
            .await
            .unwrap();
        let mut names = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            let name: String = row.get(0).unwrap();
            let sql: String = row.get(1).unwrap();
            assert!(!sql.contains("device_type_id"), "{name}: {sql}");
            assert!(!sql.contains("latest_latitude"), "{name}: {sql}");
            names.push(name);
        }
        for retired in [
            "device_types",
            "rule_zone_handoffs",
            "rule_cooldown_resets",
            "telemetry",
            "telemetry_rollups_hourly",
            "telemetry_maintenance_state",
            "network_observed_hosts",
        ] {
            assert!(!names.iter().any(|name| name == retired));
        }
        for required in [
            "device_blueprints",
            "device_blueprint_revisions",
            "device_contracts",
            "device_contract_assignments",
            "device_events",
            "device_metric_samples",
            "rule_alert_deliveries",
            "rule_zone_entries",
            "api_keys",
            "firmware_updates",
        ] {
            assert!(names.iter().any(|name| name == required), "{required}");
        }
        drop(rows);
        let mut rows = connection
            .query("PRAGMA foreign_key_check", ())
            .await
            .unwrap();
        assert!(rows.next().await.unwrap().is_none());
        drop(rows);
        let mut rows = connection
            .query("SELECT count(*) FROM _extrittio_migrations", ())
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            1
        );
        drop(rows);
        drop(connection);
        drop(database);
        let reopened =
            crate::TursoDatabase::open(directory.path(), &path, std::time::Duration::from_secs(1))
                .await
                .unwrap();
        reopened.migrate().await.unwrap();
    }
}

pub(crate) async fn run(writer: &mut Connection) -> Result<(), TursoLifecycleError> {
    writer
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS _extrittio_migrations (
                version INTEGER PRIMARY KEY,
                checksum TEXT NOT NULL,
                applied_at_us INTEGER NOT NULL
            );",
        )
        .await
        .map_err(map_migration_error)?;

    for &(version, migration) in MIGRATIONS {
        let checksum = format!("{:x}", Sha256::digest(migration.as_bytes()));
        let mut rows = writer
            .query(
                "SELECT checksum FROM _extrittio_migrations WHERE version = ?1",
                turso::params![version],
            )
            .await
            .map_err(map_migration_error)?;
        let applied_checksum = rows
            .next()
            .await
            .map_err(map_migration_error)?
            .map(|row| row.get::<String>(0))
            .transpose()
            .map_err(map_migration_error)?;
        drop(rows);

        if let Some(applied_checksum) = applied_checksum {
            if applied_checksum != checksum {
                return Err(TursoLifecycleError::Migration(format!(
                    "Turso migration {version} checksum mismatch: expected {checksum}, found {applied_checksum}"
                )));
            }
            continue;
        }

        let transaction = writer.transaction().await.map_err(map_migration_error)?;
        transaction
            .execute_batch(migration)
            .await
            .map_err(map_migration_error)?;
        transaction
            .execute(
                "INSERT INTO _extrittio_migrations (version, checksum, applied_at_us)
                 VALUES (?1, ?2, CAST(unixepoch('subsec') * 1000000 AS INTEGER))",
                turso::params![version, checksum],
            )
            .await
            .map_err(map_migration_error)?;
        transaction.commit().await.map_err(map_migration_error)?;
    }
    Ok(())
}
