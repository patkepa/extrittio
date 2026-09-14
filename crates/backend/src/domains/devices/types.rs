use crate::tenancy::DeviceIdentity;
use chrono::NaiveDateTime;
use extrittio_backend_core::rule_snapshots::DeviceRuleEvaluation;
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
