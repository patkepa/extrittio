use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{
    BlueprintDraftRecord, BlueprintList, BlueprintRecord, BlueprintRevisionRecord,
    CreateBlueprintRecord, PublishBlueprintOutcome, PublishBlueprintRecord,
    ReplaceBlueprintDraftRecord,
};

#[async_trait]
pub trait DeviceBlueprintRepository: Send + Sync {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<BlueprintList, PersistenceError>;

    async fn get(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintRecord>, PersistenceError>;

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateBlueprintRecord,
    ) -> Result<(BlueprintRecord, BlueprintDraftRecord), PersistenceError>;

    async fn get_draft(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintDraftRecord>, PersistenceError>;

    async fn replace_draft(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
        record: ReplaceBlueprintDraftRecord,
    ) -> Result<Option<BlueprintDraftRecord>, PersistenceError>;

    async fn latest_revision(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintRevisionRecord>, PersistenceError>;

    async fn get_revision(
        &self,
        tenant: &TenantId,
        revision_id: &str,
    ) -> Result<Option<BlueprintRevisionRecord>, PersistenceError>;

    async fn publish(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
        record: PublishBlueprintRecord,
    ) -> Result<PublishBlueprintOutcome, PersistenceError>;
}
