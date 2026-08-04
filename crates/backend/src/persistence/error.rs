use std::time::Duration;

/// Stable semantic name for a constraint whose identity affects domain
/// behavior. Adapters must not expose engine-specific error text as this name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintName(String);

impl ConstraintName {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConstraintName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Backend-neutral failure taxonomy used at persistence boundaries.
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("resource was not found")]
    NotFound,

    #[error("unique constraint violated: {constraint}")]
    UniqueViolation { constraint: ConstraintName },

    #[error("foreign-key constraint violated: {constraint}")]
    ForeignKeyViolation { constraint: ConstraintName },

    #[error("check constraint violated: {constraint}")]
    CheckViolation { constraint: ConstraintName },

    #[error("database is busy")]
    Busy { retry_after: Option<Duration> },

    #[error("database is unavailable: {0}")]
    Unavailable(String),

    #[error("database migration failed: {0}")]
    Migration(String),

    #[error("database returned corrupt data: {0}")]
    CorruptData(String),

    #[error("internal persistence failure: {0}")]
    Internal(String),
}
