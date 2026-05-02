use std::fmt;

pub const DEFAULT_TENANT_ID: &str = "default";

/// Stable tenant identifier carried through request and service boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(value: impl Into<String>) -> Result<Self, TenantIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(TenantIdError::Empty);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TenantIdError {
    #[error("tenant id must not be empty")]
    Empty,
}
