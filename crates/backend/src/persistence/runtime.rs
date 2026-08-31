use super::{BackendDescriptor, RepositorySet};

/// Result of the smallest backend-neutral readiness probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseHealth {
    pub reachable: bool,
}

/// Failures owned by database lifecycle and maintenance operations.
///
/// This vocabulary is intentionally separate from business persistence
/// failures: schema migrations are process lifecycle work, not repository
/// operations.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error("database is unavailable: {0}")]
    Unavailable(String),

    #[error("database migration failed: {0}")]
    Migration(String),

    #[error("database returned corrupt data: {0}")]
    CorruptData(String),

    #[error("internal database lifecycle failure: {0}")]
    Internal(String),
}

#[derive(Clone)]
enum DatabaseLifecycle {
    #[allow(dead_code)]
    Unavailable,
    #[cfg(feature = "postgres")]
    Postgres(super::postgres::lifecycle::PostgresLifecycle),
    #[cfg(feature = "turso")]
    Turso(std::sync::Arc<super::turso::TursoDatabase>),
}

/// Host-local database composition result.
///
/// Business code receives a cloned `RepositorySet`; boot, readiness and
/// maintenance retain this façade for operational work and capability data.
#[derive(Clone)]
pub struct DatabaseRuntime {
    repositories: RepositorySet,
    descriptor: BackendDescriptor,
    lifecycle: DatabaseLifecycle,
}

impl DatabaseRuntime {
    #[cfg(feature = "postgres")]
    pub(crate) fn postgres(
        repositories: RepositorySet,
        lifecycle: super::postgres::lifecycle::PostgresLifecycle,
    ) -> Self {
        Self {
            repositories,
            descriptor: BackendDescriptor::postgres(),
            lifecycle: DatabaseLifecycle::Postgres(lifecycle),
        }
    }

    #[cfg(feature = "turso")]
    pub(crate) fn turso(
        repositories: RepositorySet,
        database: std::sync::Arc<super::turso::TursoDatabase>,
    ) -> Self {
        let descriptor = BackendDescriptor::turso(database.path().to_path_buf());
        Self {
            repositories,
            descriptor,
            lifecycle: DatabaseLifecycle::Turso(database),
        }
    }

    #[must_use]
    pub fn repositories(&self) -> &RepositorySet {
        &self.repositories
    }

    #[must_use]
    pub fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    pub async fn health(&self) -> Result<DatabaseHealth, LifecycleError> {
        match &self.lifecycle {
            DatabaseLifecycle::Unavailable => Err(LifecycleError::Unavailable(
                "no database adapter is available in this build".to_string(),
            )),
            #[cfg(feature = "postgres")]
            DatabaseLifecycle::Postgres(lifecycle) => lifecycle.health().await,
            #[cfg(feature = "turso")]
            DatabaseLifecycle::Turso(database) => database.health().await,
        }
    }

    pub async fn run_migrations(&self) -> Result<(), LifecycleError> {
        match &self.lifecycle {
            DatabaseLifecycle::Unavailable => Err(LifecycleError::Unavailable(
                "no database adapter is available in this build".to_string(),
            )),
            #[cfg(feature = "postgres")]
            DatabaseLifecycle::Postgres(lifecycle) => lifecycle.run_migrations().await,
            #[cfg(feature = "turso")]
            DatabaseLifecycle::Turso(database) => database.migrate().await,
        }
    }

    /// Flush backend-local durability state. Server databases intentionally
    /// no-op because durability is owned by the server.
    pub async fn maintenance_checkpoint(&self) -> Result<(), LifecycleError> {
        match &self.lifecycle {
            DatabaseLifecycle::Unavailable => Err(LifecycleError::Unavailable(
                "no database adapter is available in this build".to_string(),
            )),
            #[cfg(feature = "postgres")]
            DatabaseLifecycle::Postgres(_) => Ok(()),
            #[cfg(feature = "turso")]
            DatabaseLifecycle::Turso(database) => database.checkpoint().await,
        }
    }

    /// Report backend connection activity for host-level operational metrics.
    #[must_use]
    pub fn connection_counts(&self) -> (i32, i32) {
        match &self.lifecycle {
            DatabaseLifecycle::Unavailable => (0, 0),
            #[cfg(feature = "postgres")]
            DatabaseLifecycle::Postgres(lifecycle) => lifecycle.connection_counts(),
            #[cfg(feature = "turso")]
            DatabaseLifecycle::Turso(_) => (1, 0),
        }
    }
}
