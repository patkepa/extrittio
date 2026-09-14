use diesel::prelude::*;
use diesel_migrations::MigrationHarness;

use crate::PostgresPool;

#[derive(Debug, thiserror::Error)]
pub enum PostgresLifecycleError {
    #[error("PostgreSQL database is unavailable: {0}")]
    Unavailable(String),
    #[error("PostgreSQL migration failed: {0}")]
    Migration(String),
    #[error("PostgreSQL database returned corrupt data: {0}")]
    CorruptData(String),
    #[error("internal PostgreSQL lifecycle failure: {0}")]
    Internal(String),
}

#[derive(Clone)]
pub struct PostgresLifecycle {
    pool: PostgresPool,
}

impl PostgresLifecycle {
    #[must_use]
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    pub async fn health(&self) -> Result<(), PostgresLifecycleError> {
        self.run(|connection| {
            #[derive(diesel::QueryableByName)]
            struct HealthRow {
                #[diesel(sql_type = diesel::sql_types::Integer)]
                value: i32,
            }

            let row = diesel::sql_query("SELECT 1 AS value")
                .get_result::<HealthRow>(connection)
                .map_err(|error| PostgresLifecycleError::Unavailable(error.to_string()))?;
            if row.value != 1 {
                return Err(PostgresLifecycleError::CorruptData(
                    "database health query returned an unexpected value".to_string(),
                ));
            }
            Ok(())
        })
        .await
    }

    pub async fn run_migrations(&self) -> Result<(), PostgresLifecycleError> {
        self.run(|connection| {
            connection
                .run_pending_migrations(crate::MIGRATIONS)
                .map(|_| ())
                .map_err(|error| PostgresLifecycleError::Migration(error.to_string()))
        })
        .await
    }

    #[must_use]
    pub fn connection_counts(&self) -> (i32, i32) {
        let state = self.pool.state();
        (
            state.connections.saturating_sub(state.idle_connections) as i32,
            state.idle_connections as i32,
        )
    }

    async fn run<T, F>(&self, operation: F) -> Result<T, PostgresLifecycleError>
    where
        T: Send + 'static,
        F: FnOnce(&mut PgConnection) -> Result<T, PostgresLifecycleError> + Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = pool
                .get()
                .map_err(|error| PostgresLifecycleError::Unavailable(error.to_string()))?;
            operation(&mut connection)
        })
        .await
        .map_err(|error| {
            PostgresLifecycleError::Internal(format!("database task failed: {error}"))
        })?
    }
}
