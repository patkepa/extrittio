use async_trait::async_trait;

use crate::tenancy::TenantId;

use super::error::PersistenceError;

/// Backend health is deliberately small at this stage. Migration head and
/// backend-specific diagnostics are added as boot migrates into this port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseHealth {
    pub reachable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinDeviceType {
    pub name: String,
    pub icon: String,
    pub color_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapOwner {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedOwnerOutcome {
    Created,
    SkippedUsersExist,
}

#[async_trait]
pub trait BootstrapRepository: Send + Sync {
    async fn health(&self) -> Result<DatabaseHealth, PersistenceError>;

    async fn run_migrations(&self) -> Result<(), PersistenceError>;

    /// Flush backend-local durability state. Remote/server databases may no-op.
    async fn maintenance_checkpoint(&self) -> Result<(), PersistenceError> {
        Ok(())
    }

    async fn seed_builtin_device_types(
        &self,
        tenant: &TenantId,
        records: Vec<BuiltinDeviceType>,
    ) -> Result<(), PersistenceError>;

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
