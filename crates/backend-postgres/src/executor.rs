//! Cloneable async boundary around Diesel's synchronous PostgreSQL pool.

use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use extrittio_backend_core::PersistenceError;

pub type PostgresPool = Pool<ConnectionManager<PgConnection>>;

#[derive(Clone)]
pub struct PostgresExecutor {
    pool: PostgresPool,
}

impl PostgresExecutor {
    #[must_use]
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    #[must_use]
    pub fn pool(&self) -> &PostgresPool {
        &self.pool
    }

    pub async fn run<T, F>(&self, operation: F) -> Result<T, PersistenceError>
    where
        T: Send + 'static,
        F: FnOnce(&mut PgConnection) -> Result<T, PersistenceError> + Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = pool
                .get()
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
            operation(&mut connection)
        })
        .await
        .map_err(|error| PersistenceError::Internal(format!("database task failed: {error}")))?
    }

    #[must_use]
    pub fn connection_counts(&self) -> (u32, u32) {
        let state = self.pool.state();
        (
            state.connections - state.idle_connections,
            state.idle_connections,
        )
    }
}
