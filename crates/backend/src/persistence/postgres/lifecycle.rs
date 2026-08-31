use diesel::prelude::*;
use diesel_migrations::MigrationHarness;

use crate::persistence::{DatabaseHealth, LifecycleError};

use super::executor::PostgresPool;

#[derive(Clone)]
pub(crate) struct PostgresLifecycle {
    pool: PostgresPool,
}

impl PostgresLifecycle {
    #[must_use]
    pub(crate) fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    pub(crate) async fn health(&self) -> Result<DatabaseHealth, LifecycleError> {
        self.run(|connection| {
            #[derive(diesel::QueryableByName)]
            struct HealthRow {
                #[diesel(sql_type = diesel::sql_types::Integer)]
                value: i32,
            }

            let row = diesel::sql_query("SELECT 1 AS value")
                .get_result::<HealthRow>(connection)
                .map_err(|error| LifecycleError::Unavailable(error.to_string()))?;
            if row.value != 1 {
                return Err(LifecycleError::CorruptData(
                    "database health query returned an unexpected value".to_string(),
                ));
            }
            Ok(DatabaseHealth { reachable: true })
        })
        .await
    }

    pub(crate) async fn run_migrations(&self) -> Result<(), LifecycleError> {
        self.run(|connection| {
            connection
                .run_pending_migrations(crate::MIGRATIONS)
                .map(|_| ())
                .map_err(|error| LifecycleError::Migration(error.to_string()))
        })
        .await
    }

    #[must_use]
    pub(crate) fn connection_counts(&self) -> (i32, i32) {
        let state = self.pool.state();
        (
            state.connections.saturating_sub(state.idle_connections) as i32,
            state.idle_connections as i32,
        )
    }

    async fn run<T, F>(&self, operation: F) -> Result<T, LifecycleError>
    where
        T: Send + 'static,
        F: FnOnce(&mut PgConnection) -> Result<T, LifecycleError> + Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = pool
                .get()
                .map_err(|error| LifecycleError::Unavailable(error.to_string()))?;
            operation(&mut connection)
        })
        .await
        .map_err(|error| LifecycleError::Internal(format!("database task failed: {error}")))?
    }
}
