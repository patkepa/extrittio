use async_trait::async_trait;

use crate::domains::activity::types::{ActivityEventPage, ActivityQuery};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

/// Tenant-scoped read model joining operational logs and administrative audit events.
///
/// This is intentionally a read-side port: producers continue to write to their
/// owning domains, while consumers get one stable event vocabulary.
#[async_trait]
pub trait ActivityRepository: Send + Sync {
    async fn list(
        &self,
        tenant: &TenantId,
        query: ActivityQuery,
    ) -> Result<ActivityEventPage, PersistenceError>;
}
