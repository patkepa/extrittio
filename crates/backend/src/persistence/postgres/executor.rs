//! Compatibility surface for legacy PostgreSQL repositories.
//!
//! The executor and pool are owned by `backend-postgres` and routed through
//! the host's approved database composition boundary. Diesel error mapping is
//! kept here until the corresponding repositories move into the adapter.

pub use crate::database::{PostgresExecutor, PostgresPool};

use crate::persistence::{ConstraintName, PersistenceError};

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
