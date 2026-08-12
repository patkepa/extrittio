use async_trait::async_trait;
use chrono::NaiveDateTime;

use crate::persistence::PersistenceError;
use crate::rule_engine::cache::RuleCache;
use crate::tenancy::TenantId;

use super::types::{NewRuleRecord, RuleDetails, RuleFilter, UpdateRuleRecord};

#[async_trait]
pub trait RuleRepository: Send + Sync {
    async fn list(
        &self,
        tenant: &TenantId,
        filter: RuleFilter,
    ) -> Result<Vec<RuleDetails>, PersistenceError>;
    async fn get(
        &self,
        tenant: &TenantId,
        id: &str,
    ) -> Result<Option<RuleDetails>, PersistenceError>;
    async fn create(
        &self,
        tenant: &TenantId,
        record: NewRuleRecord,
    ) -> Result<RuleDetails, PersistenceError>;
    async fn update(
        &self,
        tenant: &TenantId,
        id: &str,
        record: UpdateRuleRecord,
    ) -> Result<Option<RuleDetails>, PersistenceError>;
    async fn delete(&self, tenant: &TenantId, id: &str) -> Result<bool, PersistenceError>;
    async fn toggle(
        &self,
        tenant: &TenantId,
        id: &str,
        enabled: bool,
        updated_at: NaiveDateTime,
    ) -> Result<Option<RuleDetails>, PersistenceError>;
    async fn build_cache(&self) -> Result<RuleCache, PersistenceError>;
    async fn delete_stale_cooldowns(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError>;
}
