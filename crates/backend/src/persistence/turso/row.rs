use chrono::{DateTime, Utc};

use crate::persistence::{ConstraintName, PersistenceError};

pub(crate) fn datetime(micros: i64) -> Result<DateTime<Utc>, PersistenceError> {
    DateTime::from_timestamp_micros(micros).ok_or_else(|| {
        PersistenceError::CorruptData(format!("invalid UTC timestamp in database: {micros}"))
    })
}

pub(crate) fn i32(value: i64, column: &str) -> Result<i32, PersistenceError> {
    value.try_into().map_err(|_| {
        PersistenceError::CorruptData(format!("{column} is outside the supported i32 range"))
    })
}

pub(crate) fn error(error: turso::Error) -> PersistenceError {
    match error {
        turso::Error::Constraint(message) => {
            let normalized = message.to_ascii_lowercase();
            if normalized.contains("unique") || normalized.contains("primary key") {
                PersistenceError::UniqueViolation {
                    constraint: ConstraintName::new("unique"),
                }
            } else if normalized.contains("foreign key") {
                PersistenceError::ForeignKeyViolation {
                    constraint: ConstraintName::new("foreign_key"),
                }
            } else if normalized.contains("check") {
                PersistenceError::CheckViolation {
                    constraint: ConstraintName::new("check"),
                }
            } else {
                PersistenceError::Internal(format!(
                    "database constraint rejected the operation: {message}"
                ))
            }
        }
        other => PersistenceError::Internal(other.to_string()),
    }
}
