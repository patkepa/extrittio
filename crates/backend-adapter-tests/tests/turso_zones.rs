use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use extrittio_backend_adapter_tests::{ContractHarness, zones};
use extrittio_backend_core::{PersistenceError, RuleZoneSnapshotRepository, ZoneRepository};
use extrittio_backend_turso::{TursoDatabase, TursoZoneRepository};
use turso::params;

struct TursoHarness {
    _directory: tempfile::TempDir,
    database: TursoDatabase,
    repository: Arc<TursoZoneRepository>,
}

impl TursoHarness {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = TursoDatabase::open_and_migrate(
            directory.path(),
            &directory.path().join("contract.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let repository = Arc::new(TursoZoneRepository::new(database.clone()));
        Self {
            _directory: directory,
            database,
            repository,
        }
    }

    async fn with_writer<T>(
        &self,
        operation: impl AsyncFnOnce(&turso::Connection) -> Result<T, turso::Error>,
    ) -> Result<T, PersistenceError> {
        let handles = self.database.shared_handles();
        let connection = handles.lock_writer().await;
        operation(&connection)
            .await
            .map_err(|error| PersistenceError::Internal(error.to_string()))
    }
}

#[async_trait]
impl ContractHarness for TursoHarness {
    async fn reset(&self) -> Result<(), PersistenceError> {
        self.with_writer(async |connection| {
            connection
                .execute_batch(
                    "DELETE FROM rule_conditions;
                     DELETE FROM rules;
                     DELETE FROM zones;",
                )
                .await?;
            for (id, name) in [("tenant-a", "Tenant A"), ("tenant-b", "Tenant B")] {
                connection
                    .execute(
                        "INSERT OR IGNORE INTO organizations (id, name, created_at, updated_at)
                         VALUES (?1, ?2, 1, 1)",
                        params![id, name],
                    )
                    .await?;
            }
            Ok(())
        })
        .await
    }

    fn zones(&self) -> Arc<dyn ZoneRepository> {
        self.repository.clone()
    }

    fn rule_zone_snapshots(&self) -> Arc<dyn RuleZoneSnapshotRepository> {
        self.repository.clone()
    }

    async fn reference_zone(&self, tenant_id: &str, zone_id: &str) -> Result<(), PersistenceError> {
        self.with_writer(async |connection| {
            connection
                .execute(
                    "INSERT INTO rules (
                        id, tenant_id, name, description, enabled, trigger_type,
                        target_type, target_id, cooldown_seconds, created_at, updated_at
                     ) VALUES ('zone-contract-rule', ?1, 'Zone contract', NULL, 1,
                               'telemetry', 'global', NULL, 60, 1, 1)",
                    params![tenant_id],
                )
                .await?;
            connection
                .execute(
                    "INSERT INTO rule_conditions (
                        id, tenant_id, rule_id, field, operator, value,
                        condition_group, zone_id
                     ) VALUES ('zone-contract-condition', ?1, 'zone-contract-rule',
                               'location', 'inside', 'true', 0, ?2)",
                    params![tenant_id, zone_id],
                )
                .await?;
            Ok(())
        })
        .await
    }
}

#[tokio::test]
async fn turso_satisfies_the_shared_zone_contract() {
    zones::assert_contract(&TursoHarness::new().await).await;
}
