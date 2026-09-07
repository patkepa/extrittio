#![forbid(unsafe_code)]

//! PostgreSQL persistence adapters for the backend application core.

pub mod api_keys;
pub mod executor;
pub use api_keys::PostgresApiKeyRepository;
pub mod migrations;
#[cfg(feature = "migration-bridge")]
#[doc(hidden)]
pub mod models;
#[cfg(not(feature = "migration-bridge"))]
#[allow(dead_code)]
mod models;
pub mod roles;
#[cfg(feature = "migration-bridge")]
#[doc(hidden)]
pub mod schema;
#[cfg(not(feature = "migration-bridge"))]
mod schema;
pub mod users;
pub mod zones;

mod error;

pub use executor::{PostgresExecutor, PostgresPool};
pub use migrations::{MIGRATIONS, PostgresMigrationError, run_pending_migrations};
pub use roles::PostgresRoleRepository;
pub use users::PostgresUserRepository;
pub use zones::PostgresZoneRepository;

/// Adapter name used by the runtime composition root. The walking skeleton has
/// one repository today; later slices can replace this alias with a façade
/// without changing the core port implementation.
pub type PostgresAdapter = PostgresZoneRepository;
