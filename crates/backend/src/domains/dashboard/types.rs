/// Backend-neutral dashboard projection returned by persistence adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardSummary {
    pub total_devices: i64,
    pub online_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}
