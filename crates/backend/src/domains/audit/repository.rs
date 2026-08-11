use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{AuditEventRecord, NewAuditEventRecord};

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn record(
        &self,
        tenant: &TenantId,
        event: NewAuditEventRecord,
    ) -> Result<(), PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEventRecord>, PersistenceError>;
}
