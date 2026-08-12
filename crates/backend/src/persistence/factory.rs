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
        DatabaseConfig::Turso {
            data_dir,
            database_path,
            busy_timeout,
            ..
        } => {
            #[cfg(feature = "turso")]
            {
                let database =
                    super::turso::TursoDatabase::open(data_dir, database_path, *busy_timeout)
                        .await?;
                Ok(super::turso::create_persistence(database))
            }
            #[cfg(not(feature = "turso"))]
            {
                let _ = (data_dir, database_path, busy_timeout);
                anyhow::bail!(
                    "Turso was requested but this binary was built without the `turso` feature"
                )
            }
        }
    }
}
