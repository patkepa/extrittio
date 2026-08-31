use sha2::{Digest, Sha256};
use turso::Connection;

use crate::lifecycle::{TursoLifecycleError, map_migration_error};

const BASELINE: &str = include_str!("../migrations/0001_baseline.sql");
const DEVICE_BLUEPRINTS: &str = include_str!("../migrations/0002_device_blueprints.sql");
const DEVICE_CONTRACTS: &str = include_str!("../migrations/0003_device_contracts.sql");
const DEVICE_EVENTS: &str = include_str!("../migrations/0004_device_events.sql");
const RULE_BLUEPRINT_TARGETS: &str = include_str!("../migrations/0005_rule_blueprint_targets.sql");
const FIRMWARE_BLUEPRINT_TARGETS: &str =
    include_str!("../migrations/0006_firmware_blueprint_targets.sql");
const REMOVE_RETIRED_DEVICE_FEATURE: &str =
    include_str!("../migrations/0007_remove_retired_device_feature.sql");

pub const LATEST_SCHEMA_VERSION: i64 = 7;

const MIGRATIONS: &[(i64, &str)] = &[
    (1, BASELINE),
    (2, DEVICE_BLUEPRINTS),
    (3, DEVICE_CONTRACTS),
    (4, DEVICE_EVENTS),
    (5, RULE_BLUEPRINT_TARGETS),
    (6, FIRMWARE_BLUEPRINT_TARGETS),
    (7, REMOVE_RETIRED_DEVICE_FEATURE),
];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_order_and_checksums_match_the_compatibility_manifest() {
        let expected = include_str!("../tests/fixtures/migration-checksums-v1.txt")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let mut fields = line.split_whitespace();
                let file = fields.next().unwrap();
                let checksum = fields.next().unwrap();
                assert!(fields.next().is_none(), "invalid fixture line: {line}");
                (file, checksum)
            })
            .collect::<Vec<_>>();

        assert_eq!(expected.len(), MIGRATIONS.len());
        for ((version, migration), (file, expected_checksum)) in MIGRATIONS.iter().zip(expected) {
            let fixture_version = file.split_once('_').unwrap().0.parse::<i64>().unwrap();
            assert_eq!(*version, fixture_version, "{file}");
            assert_eq!(
                format!("{:x}", Sha256::digest(migration.as_bytes())),
                expected_checksum,
                "{file}"
            );
        }
        assert_eq!(MIGRATIONS.last().unwrap().0, LATEST_SCHEMA_VERSION);
    }
}
