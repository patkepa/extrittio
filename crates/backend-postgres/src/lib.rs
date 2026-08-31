#![forbid(unsafe_code)]

//! PostgreSQL persistence adapters for the backend application core.

pub mod executor;
pub mod migrations;
pub mod models;
pub mod schema;
pub mod zones;

mod error;

pub use executor::{PostgresExecutor, PostgresPool};
pub use migrations::{MIGRATIONS, PostgresMigrationError, run_pending_migrations};
pub use zones::PostgresZoneRepository;

/// Adapter name used by the runtime composition root. The walking skeleton has
/// one repository today; later slices can replace this alias with a façade
/// without changing the core port implementation.
pub type PostgresAdapter = PostgresZoneRepository;
