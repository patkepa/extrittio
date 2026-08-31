use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{PersistenceError, TenantId};

#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub id: String,
    pub tenant_id: TenantId,
    pub name: String,
    pub description: String,
    pub geometry_type: String,
    pub geometry_json: Value,
    pub color: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewZone {
    pub id: String,
    pub name: String,
    pub description: String,
    pub geometry_type: String,
    pub geometry_json: Value,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZonePatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geometry_type: Option<String>,
    pub geometry_json: Option<Value>,
    pub color: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteZoneOutcome {
    NotFound,
    InUse,
    Deleted,
}

/// Tenant-scoped zone CRUD. Cross-tenant reads intentionally do not belong here.
#[async_trait]
pub trait ZoneRepository: Send + Sync {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<Zone>, PersistenceError>;
    async fn get(&self, tenant: &TenantId, zone_id: &str)
    -> Result<Option<Zone>, PersistenceError>;
    async fn create(&self, tenant: &TenantId, zone: NewZone) -> Result<Zone, PersistenceError>;
    async fn update(
        &self,
        tenant: &TenantId,
        zone_id: &str,
        patch: ZonePatch,
    ) -> Result<Option<Zone>, PersistenceError>;
    async fn delete(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<DeleteZoneOutcome, PersistenceError>;
}

/// System-scoped zone projection used while building immutable rule snapshots.
#[async_trait]
pub trait RuleZoneSnapshotRepository: Send + Sync {
    async fn list_for_rule_snapshot(&self) -> Result<Vec<Zone>, PersistenceError>;
}
