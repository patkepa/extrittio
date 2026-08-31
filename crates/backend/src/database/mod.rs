//! Host composition boundary for concrete database adapters.
//!
//! Legacy persistence modules consume only backend-neutral ports; construction
//! of extracted adapters is centralized here until the remaining repositories
//! move into their dedicated crates.

#[cfg(any(feature = "postgres", feature = "turso"))]
use std::sync::Arc;

#[cfg(any(feature = "postgres", feature = "turso"))]
use extrittio_backend_core::ZoneRepository;

#[cfg(feature = "postgres")]
pub use extrittio_backend_postgres::{models, schema};

#[cfg(feature = "postgres")]
pub(crate) fn postgres_zones(
    pool: &crate::persistence::postgres::executor::PostgresPool,
) -> Arc<dyn ZoneRepository> {
    Arc::new(extrittio_backend_postgres::PostgresZoneRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_zones(
    database: &crate::persistence::turso::TursoDatabase,
) -> Arc<dyn ZoneRepository> {
    let (database, writer) = database.shared_handles();
    Arc::new(extrittio_backend_turso::TursoZoneRepository::from_shared_handles(database, writer))
}
