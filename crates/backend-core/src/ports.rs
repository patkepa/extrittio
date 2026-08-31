//! Business and outbound ports owned by the application layer.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::EncodedPasswordHash;

pub use crate::roles::RoleRepository;
pub use crate::users::UserRepository;
pub use crate::zones::{RuleZoneSnapshotRepository, ZoneRepository};

/// Opaque failure from a host-owned password implementation.
///
/// The error contains no plaintext password. Its detail is retained for
/// internal diagnostics while HTTP mapping returns the stable safe body.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct PasswordHasherError(String);

impl PasswordHasherError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Host-owned password hashing and verification boundary.
///
/// Owned inputs allow the host to move secret material into a blocking task
/// and zeroize it when that task finishes. Invalid stored encodings should
/// return `Ok(false)` so they fail authentication closed.
#[async_trait]
pub trait PasswordHasher: Send + Sync {
    async fn hash(&self, plaintext: String) -> Result<EncodedPasswordHash, PasswordHasherError>;

    async fn verify(
        &self,
        plaintext: String,
        password_hash: EncodedPasswordHash,
    ) -> Result<bool, PasswordHasherError>;
}

/// Deterministic UTC clock boundary for application-owned timestamps.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
