use crate::DeviceIdentity;
use crate::PersistenceError;
use crate::rule_snapshots::DeviceRuleEvaluation;
use async_trait::async_trait;
use chrono::NaiveDateTime;
#[derive(Debug, Clone)]
pub struct DeviceIngressContext {
    pub identity: DeviceIdentity,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub blueprint_id: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct HeartbeatWrite {
    pub expected_status: String,
    pub status: String,
    pub firmware: String,
    pub uptime_seconds: i32,
    pub observed_at: NaiveDateTime,
    pub rule_evaluation: Option<DeviceRuleEvaluation>,
}

#[derive(Debug, Clone)]
pub struct OfflineTransition {
    pub context: DeviceIngressContext,
    pub rule_evaluation: Option<DeviceRuleEvaluation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceWriteOutcome {
    pub applied: bool,
    pub actions_enqueued: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflineWriteOutcome {
    pub devices_updated: usize,
    pub actions_enqueued: usize,
}

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
