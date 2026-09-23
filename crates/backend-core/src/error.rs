use std::time::Duration;

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

/// Stable failures exposed by business persistence ports.
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("resource was not found")]
    NotFound,
    #[error("requested metric history is outside the retained range")]
    HistoryExpired,
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
    #[error("database returned corrupt data: {0}")]
    CorruptData(String),
    #[error("internal persistence failure: {0}")]
    Internal(String),
}

/// Transport-independent application failure.
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("{0}")]
    DeviceCommunication(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    InvalidInput(String),
    /// Validly parsed input that violates an operation precondition.
    #[error("{0}")]
    InvalidOperation(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    /// Failure to generate a password verifier. The host maps this to its
    /// existing safe `authentication_error` response category.
    #[error("{0}")]
    Authentication(String),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("{0}")]
    Internal(String),
}
