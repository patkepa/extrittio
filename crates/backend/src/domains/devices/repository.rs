use super::types::{
    DeviceIngressContext, DeviceWriteOutcome, HeartbeatWrite, OfflineTransition,
    OfflineWriteOutcome,
};
use crate::persistence::PersistenceError;
use crate::tenancy::DeviceIdentity;
use async_trait::async_trait;
#[async_trait]
pub trait DeviceIngressRepository: Send + Sync {
    /// Resolves the globally unique protocol device ID to its tenant-qualified
    /// identity before any tenant-owned ingress operation runs.
    async fn resolve_identity(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceIdentity>, PersistenceError>;

    async fn ingress_context(
        &self,
        identity: &DeviceIdentity,
    ) -> Result<Option<DeviceIngressContext>, PersistenceError>;

    /// Updates heartbeat state and enqueues status-rule actions in one commit.
    /// `applied=false` requests a caller retry after concurrent state change.
    async fn apply_heartbeat(
        &self,
        identity: &DeviceIdentity,
        write: HeartbeatWrite,
    ) -> Result<DeviceWriteOutcome, PersistenceError>;

    async fn offline_candidates(
        &self,
        cutoff: chrono::NaiveDateTime,
    ) -> Result<Vec<DeviceIngressContext>, PersistenceError>;

    /// Rechecks cutoff and expected status, then commits offline transitions,
    /// logs, and rule actions atomically.
    async fn apply_offline_transitions(
        &self,
        cutoff: chrono::NaiveDateTime,
        transitions: Vec<OfflineTransition>,
    ) -> Result<OfflineWriteOutcome, PersistenceError>;
}
