use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct BlueprintRecord {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub latest_revision: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BlueprintList {
    pub records: Vec<BlueprintRecord>,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct BlueprintDraftRecord {
    pub id: String,
    pub blueprint_id: String,
    pub document: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BlueprintRevisionRecord {
    pub id: String,
    pub blueprint_id: String,
    pub revision: i32,
    pub document: Value,
    pub document_hash: String,
    pub compatibility: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateBlueprintRecord {
    pub id: String,
    pub draft_id: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub document: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ReplaceBlueprintDraftRecord {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub document: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PublishBlueprintRecord {
    pub revision_id: String,
    /// Latest immutable revision used to compute compatibility.
    pub expected_previous_revision_id: Option<String>,
    pub expected_draft_updated_at: DateTime<Utc>,
    pub document: Value,
    pub document_hash: String,
    pub compatibility: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum PublishBlueprintOutcome {
    Published(BlueprintRevisionRecord),
    BlueprintNotFound,
    DraftChanged,
    PublicationChanged,
}

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

    /// Compare the locked draft timestamp/document and latest revision used for
    /// compatibility. Return the latest revision unchanged for an identical
    /// document/hash retry; otherwise create one immutable next revision.
    /// A changed draft or publication baseline returns a typed conflict.
    async fn publish(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
        record: PublishBlueprintRecord,
    ) -> Result<PublishBlueprintOutcome, PersistenceError>;
}
