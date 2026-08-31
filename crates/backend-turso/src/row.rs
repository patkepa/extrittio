use chrono::{DateTime, Utc};
#[cfg(any(feature = "migration-bridge", test))]
use extrittio_backend_core::ConstraintName;
use extrittio_backend_core::PersistenceError;

/// Decode a microsecond Unix timestamp at the Turso boundary.
pub fn datetime(micros: i64) -> Result<DateTime<Utc>, PersistenceError> {
    DateTime::from_timestamp_micros(micros).ok_or_else(|| {
        PersistenceError::CorruptData(format!("invalid UTC timestamp in database: {micros}"))
    })
}

/// Narrow an integer column while reporting persisted data corruption rather
/// than silently truncating it.
pub fn i32(value: i64, column: &str) -> Result<i32, PersistenceError> {
    value.try_into().map_err(|_| {
        PersistenceError::CorruptData(format!("{column} is outside the supported i32 range"))
    })
}

/// Compatibility mapper for host repositories that have not moved into this
/// adapter yet. Migrated repositories use the richer private adapter mapper.
#[cfg(any(feature = "migration-bridge", test))]
pub fn legacy_error(error: turso::Error) -> PersistenceError {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_and_integer_decoding_reject_invalid_persisted_values() {
        assert!(datetime(i64::MAX).is_err());
        assert!(i32(i64::from(i32::MAX) + 1, "revision").is_err());
    }

    #[test]
    fn legacy_constraint_mapping_stays_semantic() {
        assert!(matches!(
            legacy_error(turso::Error::Constraint(
                "UNIQUE constraint failed: table.id".into()
            )),
            PersistenceError::UniqueViolation { .. }
        ));
    }
}
