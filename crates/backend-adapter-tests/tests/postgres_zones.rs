use std::sync::Arc;

use async_trait::async_trait;
use diesel::Connection;
use diesel::RunQueryDsl;
use diesel::r2d2::{ConnectionManager, Pool};
use extrittio_backend_adapter_tests::{ContractHarness, zones};
use extrittio_backend_core::{PersistenceError, RuleZoneSnapshotRepository, ZoneRepository};
use extrittio_backend_postgres::{PostgresZoneRepository, run_pending_migrations};

struct PostgresHarness {
    pool: Pool<ConnectionManager<diesel::PgConnection>>,
    repository: Arc<PostgresZoneRepository>,
}

impl PostgresHarness {
    fn from_environment() -> Option<Self> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = Pool::builder()
            .max_size(2)
            .build(ConnectionManager::new(url))
            .expect("connect to disposable PostgreSQL contract database");
        run_pending_migrations(&mut pool.get().unwrap()).expect("run PostgreSQL migrations");
        let repository = Arc::new(PostgresZoneRepository::from_pool(pool.clone()));
        Some(Self { pool, repository })
    }
}

#[async_trait]
impl ContractHarness for PostgresHarness {
    async fn reset(&self) -> Result<(), PersistenceError> {
        let mut connection = self
            .pool
            .get()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        connection
            .transaction::<_, diesel::result::Error, _>(|connection| {
                for statement in [
                    "DELETE FROM rule_conditions WHERE tenant_id IN ('tenant-a', 'tenant-b')",
                    "DELETE FROM rules WHERE tenant_id IN ('tenant-a', 'tenant-b')",
                    "DELETE FROM zones WHERE tenant_id IN ('tenant-a', 'tenant-b')",
                    "INSERT INTO organizations (id, name)
                     VALUES ('tenant-a', 'Tenant A'), ('tenant-b', 'Tenant B')
                     ON CONFLICT (id) DO NOTHING",
                ] {
                    diesel::sql_query(statement).execute(connection)?;
                }
                Ok(())
            })
            .map_err(|error| PersistenceError::Internal(error.to_string()))
    }

    fn zones(&self) -> Arc<dyn ZoneRepository> {
        self.repository.clone()
    }

    fn rule_zone_snapshots(&self) -> Arc<dyn RuleZoneSnapshotRepository> {
        self.repository.clone()
    }

    async fn reference_zone(&self, tenant_id: &str, zone_id: &str) -> Result<(), PersistenceError> {
        let mut connection = self
            .pool
            .get()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        connection
            .transaction::<_, diesel::result::Error, _>(|connection| {
                diesel::sql_query(
                    "INSERT INTO rules (
                        id, tenant_id, name, description, enabled, trigger_type,
                        target_type, target_id, cooldown_seconds
                     ) VALUES ($1, $2, 'Zone contract', NULL, TRUE,
                               'telemetry', 'global', NULL, 60)",
                )
                .bind::<diesel::sql_types::Text, _>("zone-contract-rule")
                .bind::<diesel::sql_types::Text, _>(tenant_id)
                .execute(connection)?;
                diesel::sql_query(
                    "INSERT INTO rule_conditions (
                        id, tenant_id, rule_id, field, operator, value,
                        condition_group, zone_id
                     ) VALUES ($1, $2, 'zone-contract-rule', 'location', 'eq',
                               'true', 0, $3)",
                )
                .bind::<diesel::sql_types::Text, _>("zone-contract-condition")
                .bind::<diesel::sql_types::Text, _>(tenant_id)
                .bind::<diesel::sql_types::Text, _>(zone_id)
                .execute(connection)?;
                Ok(())
            })
            .map_err(|error| PersistenceError::Internal(error.to_string()))
    }
}

#[tokio::test]
async fn postgres_satisfies_the_shared_zone_contract_when_configured() {
    let Some(harness) = PostgresHarness::from_environment() else {
        eprintln!("skipping PostgreSQL zone contract: DATABASE_URL is not set");
        return;
    };
    zones::assert_contract(&harness).await;
}
