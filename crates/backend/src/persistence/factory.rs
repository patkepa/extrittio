use crate::config::DatabaseConfig;

use super::Persistence;

/// Construct exactly the configured database adapter. This function never
/// falls back to a different engine after an open or configuration failure.
pub async fn create(config: &DatabaseConfig) -> anyhow::Result<Persistence> {
    match config {
        DatabaseConfig::Postgres { url, pool_size } => {
            #[cfg(feature = "postgres")]
            {
                let pool = crate::init::create_db_pool(url, *pool_size)?;
                Ok(super::postgres::create_persistence(pool))
            }
            #[cfg(not(feature = "postgres"))]
            {
                let _ = (url, pool_size);
                anyhow::bail!(
                    "PostgreSQL was requested but this binary was built without the `postgres` feature"
                )
            }
        }
        DatabaseConfig::Turso { .. } => {
            #[cfg(feature = "turso")]
            anyhow::bail!(
                "the Turso persistence adapter is not available yet; this build will not fall back to PostgreSQL"
            );
            #[cfg(not(feature = "turso"))]
            anyhow::bail!(
                "Turso was requested but this binary was built without the `turso` feature"
            )
        }
    }
}
