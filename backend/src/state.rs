use diesel::SqliteConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use std::sync::Arc;

use crate::error::AppError;
use crate::rate_limit::RateLimiter;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub struct AppState {
    pub db_pool: DbPool,
    pub zenoh_session: Arc<zenoh::Session>,
    pub jwt_secret: String,
    pub api_rate_limiter: RateLimiter,
    pub login_rate_limiter: RateLimiter,
}

/// Run a synchronous DB operation on a blocking thread to avoid starving the
/// Tokio runtime. Acquires a pooled connection, passes it to the closure, and
/// returns the result.
pub async fn run_db<F, T>(pool: &DbPool, f: F) -> Result<T, AppError>
where
    F: FnOnce(&mut SqliteConnection) -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get()?;
        f(&mut conn)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {e}")))?
}
