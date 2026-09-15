use async_trait::async_trait;

use crate::{EncodedPasswordHash, TenantId};

use crate::PersistenceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapOwner {
    pub username: String,
    pub password_hash: EncodedPasswordHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedOwnerOutcome {
    Created,
    SkippedUsersExist,
}

#[async_trait]
pub trait BootstrapRepository: Send + Sync {
    async fn get_or_create_server_config(
        &self,
        key: &str,
        generated_value: String,
    ) -> Result<String, PersistenceError>;

    async fn users_exist(&self) -> Result<bool, PersistenceError>;

    async fn seed_owner_if_empty(
        &self,
        tenant: &TenantId,
        owner: BootstrapOwner,
    ) -> Result<SeedOwnerOutcome, PersistenceError>;
}
