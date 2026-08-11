use async_trait::async_trait;

use chrono::NaiveDateTime;

use crate::domains::commands::types::{CommandQuery, CommandRecord, NewCommandRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

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
        identity: &DeviceIdentity,
        correlation_id: String,
        device_status: String,
        payload: Option<String>,
    ) -> Result<Option<String>, PersistenceError>;

    async fn timeout_stale(
        &self,
        cutoff: NaiveDateTime,
        now: NaiveDateTime,
    ) -> Result<usize, PersistenceError>;
}
