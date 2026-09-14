use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ActivityQuery {
    pub source: Option<String>,
    pub severity: Option<String>,
    pub category: Option<String>,
    pub device_id: Option<String>,
    pub search: Option<String>,
    pub since: Option<NaiveDateTime>,
    pub until: Option<NaiveDateTime>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct ActivityEventRecord {
    pub id: String,
    pub source: String,
    pub severity: String,
    pub event_type: String,
    pub category: String,
    pub message: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub request_id: Option<String>,
    pub metadata: Value,
    pub occurred_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ActivityEventPage {
    pub data: Vec<ActivityEventRecord>,
    pub total: i64,
}

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
