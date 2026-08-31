use std::time::Duration;

use extrittio_backend_core::{ConstraintName, PersistenceError};

pub(crate) fn map_error(error: turso::Error) -> PersistenceError {
    match error {
        turso::Error::Constraint(message) => map_constraint(&message),
        turso::Error::Busy(_) | turso::Error::BusySnapshot(_) => PersistenceError::Busy {
            retry_after: Some(Duration::from_millis(50)),
        },
        turso::Error::Corrupt(message) | turso::Error::NotAdb(message) => {
            PersistenceError::CorruptData(message)
        }
        turso::Error::IoError(_, _) | turso::Error::Readonly(_) | turso::Error::DatabaseFull(_) => {
            PersistenceError::Unavailable(error.to_string())
        }
        turso::Error::ConversionFailure(message) => PersistenceError::CorruptData(message),
        other => PersistenceError::Internal(other.to_string()),
    }
}

pub(crate) fn map_open_error(error: turso::Error) -> PersistenceError {
    match error {
        turso::Error::Corrupt(message) | turso::Error::NotAdb(message) => {
            PersistenceError::CorruptData(message)
        }
        other => PersistenceError::Unavailable(other.to_string()),
    }
}

fn map_constraint(message: &str) -> PersistenceError {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("unique") || normalized.contains("primary key") {
        let constraint =
            if normalized.contains("zones.tenant_id") && normalized.contains("zones.name") {
                "zones.tenant_name"
            } else if normalized.contains("zones.id") {
                "zones_id"
            } else {
                "unique"
            };
        PersistenceError::UniqueViolation {
            constraint: ConstraintName::new(constraint),
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
