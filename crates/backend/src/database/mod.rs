//! Host composition boundary for concrete database adapters.
//!
//! Legacy persistence modules consume only backend-neutral ports; construction
//! of extracted adapters is centralized here until the remaining repositories
//! move into their dedicated crates.

#[cfg(feature = "postgres")]
pub(crate) fn postgres_api_keys(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::ApiKeyRepository> {
    Arc::new(extrittio_backend_postgres::PostgresApiKeyRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_api_keys(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::ApiKeyRepository> {
    Arc::new(
        extrittio_backend_turso::TursoApiKeyRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(any(feature = "postgres", feature = "turso"))]
use std::sync::Arc;

#[cfg(any(feature = "postgres", feature = "turso"))]
use extrittio_backend_core::{
    RoleRepository, RuleZoneSnapshotRepository, UserRepository, ZoneRepository,
};

#[cfg(feature = "postgres")]
pub use extrittio_backend_postgres::{PostgresExecutor, PostgresPool, models, schema};

#[cfg(feature = "turso")]
mod turso;
#[cfg(feature = "turso")]
pub(crate) use extrittio_backend_turso::migration_bridge::{
    connect as turso_connect, datetime as turso_datetime, i32 as turso_i32,
    legacy_error as turso_error, lock_writer as turso_lock_writer,
};
#[cfg(feature = "turso")]
pub use turso::{LogicalArchiveInfo, TursoBackupInfo, TursoDatabase, TursoDatabaseInfo};

#[cfg(feature = "postgres")]
pub(crate) fn postgres_zones(
    pool: &crate::persistence::postgres::executor::PostgresPool,
) -> (Arc<dyn ZoneRepository>, Arc<dyn RuleZoneSnapshotRepository>) {
    let adapter =
        Arc::new(extrittio_backend_postgres::PostgresZoneRepository::from_pool(pool.clone()));
    (adapter.clone(), adapter)
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_roles(
    pool: &crate::persistence::postgres::executor::PostgresPool,
) -> Arc<dyn RoleRepository> {
    Arc::new(extrittio_backend_postgres::PostgresRoleRepository::from_pool(pool.clone()))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_users(
    pool: &crate::persistence::postgres::executor::PostgresPool,
) -> Arc<dyn UserRepository> {
    Arc::new(extrittio_backend_postgres::PostgresUserRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_zones(
    database: &crate::persistence::turso::TursoDatabase,
) -> (Arc<dyn ZoneRepository>, Arc<dyn RuleZoneSnapshotRepository>) {
    let adapter = Arc::new(extrittio_backend_turso::TursoZoneRepository::from_handles(
        database.shared_handles(),
    ));
    (adapter.clone(), adapter)
}

#[cfg(feature = "turso")]
pub(crate) fn turso_roles(
    database: &crate::persistence::turso::TursoDatabase,
) -> Arc<dyn RoleRepository> {
    Arc::new(extrittio_backend_turso::TursoRoleRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_users(
    database: &crate::persistence::turso::TursoDatabase,
) -> Arc<dyn UserRepository> {
    Arc::new(extrittio_backend_turso::TursoUserRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_ci_ingest(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::CiIngestRepository> {
    Arc::new(extrittio_backend_postgres::PostgresCiIngestRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_ci_ingest(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::CiIngestRepository> {
    Arc::new(
        extrittio_backend_turso::TursoCiIngestRepository::from_handles(database.shared_handles()),
    )
}
