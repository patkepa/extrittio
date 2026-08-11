use async_trait::async_trait;
use chrono::NaiveDateTime;

use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::types::{
    TelemetryMaintenanceOutcome, TelemetryQuery, TelemetryRecord, TelemetryRollup, TelemetryWrite,
    TelemetryWriteOutcome,
};

#[async_trait]
pub trait TelemetryRepository: Send + Sync {
    async fn record(
        &self,
        identity: &DeviceIdentity,
        write: TelemetryWrite,
    ) -> Result<TelemetryWriteOutcome, PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRecord>>, PersistenceError>;

    async fn latest(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError>;

    async fn list_hourly(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRollup>>, PersistenceError>;

    async fn latest_location(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError>;

    async fn maintain(
        &self,
        rollup_since: NaiveDateTime,
        rollup_before: NaiveDateTime,
        retention_cutoff: NaiveDateTime,
    ) -> Result<TelemetryMaintenanceOutcome, PersistenceError>;
}
