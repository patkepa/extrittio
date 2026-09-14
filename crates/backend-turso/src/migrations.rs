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
const USER_AUTH_EPOCH: &str = include_str!("../migrations/0008_user_auth_epoch.sql");
const RULE_ACTION_OUTBOX_ACTIVE_IDEMPOTENCY: &str =
    include_str!("../migrations/0009_rule_action_outbox_active_idempotency.sql");

const RULE_ALERT_DELIVERIES: &str = include_str!("../migrations/0010_rule_alert_deliveries.sql");

const RULE_ZONE_ENTRIES: &str = include_str!("../migrations/0011_rule_zone_entries.sql");
const RULE_COOLDOWN_RESETS: &str = include_str!("../migrations/0012_rule_cooldown_resets.sql");
const RULE_ZONE_HANDOFFS: &str = include_str!("../migrations/0013_rule_zone_handoffs.sql");
const TELEMETRY_MAINTENANCE_BOUNDARY: &str =
    include_str!("../migrations/0014_telemetry_maintenance_boundary.sql");
pub const LATEST_SCHEMA_VERSION: i64 = 14;

const MIGRATIONS: &[(i64, &str)] = &[
    (1, BASELINE),
    (2, DEVICE_BLUEPRINTS),
    (3, DEVICE_CONTRACTS),
    (4, DEVICE_EVENTS),
    (5, RULE_BLUEPRINT_TARGETS),
    (6, FIRMWARE_BLUEPRINT_TARGETS),
    (7, REMOVE_RETIRED_DEVICE_FEATURE),
    (8, USER_AUTH_EPOCH),
    (9, RULE_ACTION_OUTBOX_ACTIVE_IDEMPOTENCY),
    (10, RULE_ALERT_DELIVERIES),
    (11, RULE_ZONE_ENTRIES),
    (12, RULE_COOLDOWN_RESETS),
    (13, RULE_ZONE_HANDOFFS),
    (14, TELEMETRY_MAINTENANCE_BOUNDARY),
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
