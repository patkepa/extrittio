//! PostgreSQL migration assets owned by this adapter's lifecycle surface.

use diesel::PgConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

#[derive(Debug, thiserror::Error)]
#[error("PostgreSQL migration failed: {source}")]
pub struct PostgresMigrationError {
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn run_pending_migrations(
    connection: &mut PgConnection,
) -> Result<Vec<String>, PostgresMigrationError> {
    connection
        .run_pending_migrations(MIGRATIONS)
        .map(|versions| {
            versions
                .into_iter()
                .map(|version| version.to_string())
                .collect()
        })
        .map_err(|source| PostgresMigrationError { source })
}
