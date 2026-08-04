use async_trait::async_trait;

use super::error::PersistenceError;

/// Backend health is deliberately small at this stage. Migration head and
/// backend-specific diagnostics are added as boot migrates into this port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseHealth {
    pub reachable: bool,
}

#[async_trait]
pub trait BootstrapRepository: Send + Sync {
    async fn health(&self) -> Result<DatabaseHealth, PersistenceError>;
}
