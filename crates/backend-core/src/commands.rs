use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRecord {
    pub id: String,
    pub device_id: String,
    pub command: String,
    pub params: String,
    pub status: String,
    pub response_payload: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NewCommandRecord {
    pub id: String,
    pub command: String,
    pub params: String,
}

#[derive(Debug, Clone)]
pub struct CommandQuery {
    pub status: Option<String>,
    pub limit: i64,
}

/// Only these states can accept a device response or expire.
pub const ACTIVE_STATUSES: &[&str] = &["sent", "delivered"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandResponseStatus {
    Delivered,
    Succeeded,
    Failed,
}
impl CommandResponseStatus {
    /// Preserve PostgreSQL's established interpretation of device responses.
    pub fn from_device_status(status: &str) -> Self {
        match status {
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            _ => Self::Delivered,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[async_trait]
pub trait CommandRepository: Send + Sync {
    async fn create(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: NewCommandRecord,
    ) -> Result<Option<CommandRecord>, PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: CommandQuery,
    ) -> Result<Option<Vec<CommandRecord>>, PersistenceError>;

    /// Applies a device response only when the command belongs to that device
    /// and is not already terminal. Returns the committed status on update.
    async fn apply_response(
        &self,
        tenant: &TenantId,
        device_id: &str,
        correlation_id: String,
        status: CommandResponseStatus,
        payload: Option<String>,
        updated_at: NaiveDateTime,
    ) -> Result<Option<String>, PersistenceError>;

    async fn timeout_stale(
        &self,
        cutoff: NaiveDateTime,
        now: NaiveDateTime,
    ) -> Result<usize, PersistenceError>;
}
