use std::fmt;

use thiserror::Error;

/// A precise, stable blueprint or instance validation issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub path: String,
    pub message: String,
}

impl ValidationIssue {
    #[must_use]
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("blueprint is invalid ({0} issue(s))")]
    InvalidBlueprint(usize, Vec<ValidationIssue>),

    #[error("instance does not satisfy schema ({0} issue(s))")]
    InvalidInstance(usize, Vec<ValidationIssue>),

    #[error("failed to serialize canonical contract: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("transport binding '{0}' was not provided")]
    MissingTransportBinding(String),

    #[error("invalid compile context: {0}")]
    InvalidContext(String),
}

impl ContractError {
    #[must_use]
    pub fn validation_issues(&self) -> Option<&[ValidationIssue]> {
        match self {
            Self::InvalidBlueprint(_, issues) | Self::InvalidInstance(_, issues) => Some(issues),
            Self::Serialization(_) | Self::MissingTransportBinding(_) | Self::InvalidContext(_) => {
                None
            }
        }
    }
}
