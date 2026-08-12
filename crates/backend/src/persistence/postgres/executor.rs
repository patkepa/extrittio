use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};

use crate::persistence::error::{ConstraintName, PersistenceError};

pub type PostgresPool = Pool<ConnectionManager<PgConnection>>;

/// The only async boundary around synchronous Diesel operations in migrated
/// PostgreSQL repositories.
#[derive(Clone)]
pub struct PostgresExecutor {
    pool: PostgresPool,
}

impl PostgresExecutor {
    #[must_use]
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    pub async fn run<T, F>(&self, operation: F) -> Result<T, PersistenceError>
    where
        T: Send + 'static,
        F: FnOnce(&mut PgConnection) -> Result<T, PersistenceError> + Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = pool
                .get()
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
            operation(&mut connection)
        })
        .await
        .map_err(|error| PersistenceError::Internal(format!("database task failed: {error}")))?
    }

    #[must_use]
    pub fn connection_counts(&self) -> (i32, i32) {
        let state = self.pool.state();
        (
            state.connections as i32 - state.idle_connections as i32,
            state.idle_connections as i32,
        )
    }
}

pub fn map_diesel_error(error: diesel::result::Error) -> PersistenceError {
    use diesel::result::{DatabaseErrorKind, Error};

    match error {
        Error::NotFound => PersistenceError::NotFound,
        Error::DatabaseError(kind, information) => {
            let constraint = information
                .constraint_name()
                .map_or_else(|| "unknown_constraint".to_string(), ToOwned::to_owned);
            let constraint = ConstraintName::new(constraint);
            match kind {
                DatabaseErrorKind::UniqueViolation => {
                    PersistenceError::UniqueViolation { constraint }
                }
                DatabaseErrorKind::ForeignKeyViolation => {
                    PersistenceError::ForeignKeyViolation { constraint }
                }
                DatabaseErrorKind::CheckViolation => {
                    PersistenceError::CheckViolation { constraint }
                }
                DatabaseErrorKind::UnableToSendCommand | DatabaseErrorKind::ClosedConnection => {
                    PersistenceError::Unavailable(information.message().to_string())
                }
                DatabaseErrorKind::SerializationFailure => {
                    PersistenceError::Busy { retry_after: None }
                }
                _ => PersistenceError::Internal(information.message().to_string()),
            }
        }
        other => PersistenceError::Internal(other.to_string()),
    }
}
