use crate::database::PostgresPool;
use crate::persistence::{DatabaseHealth, LifecycleError};
use extrittio_backend_postgres::{PostgresLifecycle as AdapterLifecycle, PostgresLifecycleError};

#[derive(Clone)]
pub(crate) struct PostgresLifecycle {
    adapter: AdapterLifecycle,
}

impl PostgresLifecycle {
    pub(crate) fn new(pool: PostgresPool) -> Self {
        Self {
            adapter: AdapterLifecycle::new(pool),
        }
    }

    pub(crate) async fn health(&self) -> Result<DatabaseHealth, LifecycleError> {
        self.adapter.health().await.map_err(map_error)?;
        Ok(DatabaseHealth { reachable: true })
    }

    pub(crate) async fn run_migrations(&self) -> Result<(), LifecycleError> {
        self.adapter.run_migrations().await.map_err(map_error)
    }

    pub(crate) fn connection_counts(&self) -> (i32, i32) {
        self.adapter.connection_counts()
    }
}

fn map_error(error: PostgresLifecycleError) -> LifecycleError {
    match error {
        PostgresLifecycleError::Unavailable(message) => LifecycleError::Unavailable(message),
        PostgresLifecycleError::Migration(message) => LifecycleError::Migration(message),
        PostgresLifecycleError::CorruptData(message) => LifecycleError::CorruptData(message),
        PostgresLifecycleError::Internal(message) => LifecycleError::Internal(message),
    }
}
