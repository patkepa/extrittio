use crate::certificates::NewDeviceCertificateRecord;
use crate::device_types::DeviceTypeRecord;
use crate::fleets::FleetRecord;
use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceRecord {
    pub id: String,
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub status: String,
    pub firmware: String,
    pub last_seen: Option<DateTime<Utc>>,
    pub uptime_seconds: i32,
    pub latest_latitude: Option<f64>,
    pub latest_longitude: Option<f64>,
    pub declared_connections: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceDetails {
    pub device: DeviceRecord,
    pub device_type: DeviceTypeRecord,
    pub fleet: Option<FleetRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceList {
    pub records: Vec<DeviceDetails>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceListQuery {
    pub status: Option<String>,
    pub search: Option<String>,
    pub fleet_id: Option<i32>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceFilter {
    pub status: Option<String>,
    pub search: Option<String>,
    pub fleet_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateDeviceRecord {
    pub id: String,
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub firmware: String,
    pub contract: NewDeviceContractRecord,
}

pub use crate::device_contracts::NewDeviceContractRecord;

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceContractRecord {
    pub id: String,
    pub device_id: String,
    pub blueprint_revision_id: String,
    pub document: Value,
    pub contract_hash: String,
    pub assignment_status: String,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateDeviceRecord {
    pub name: Option<String>,
    pub device_type_id: Option<i32>,
    pub fleet_id: Option<Option<i32>>,
    pub firmware: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn list(
        &self,
        tenant: &TenantId,
        query: DeviceListQuery,
    ) -> Result<DeviceList, PersistenceError>;

    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceDetails>, PersistenceError>;

    /// Returns the contract currently assigned to the device. Until the device
    /// acknowledges a replacement, the desired contract is authoritative.
    async fn assigned_contract(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceContractRecord>, PersistenceError>;

    /// Atomically creates device, compiled contract/assignment, initial configuration
    /// (when declared), shadow, and prepared certificate. Failure rolls back all rows.
    /// Duplicate identity/name is a conflict, never an overwrite or credential rotation.
    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceRecord,
        certificate: Option<NewDeviceCertificateRecord>,
    ) -> Result<DeviceDetails, PersistenceError>;

    async fn update(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: UpdateDeviceRecord,
    ) -> Result<Option<DeviceDetails>, PersistenceError>;

    async fn delete(&self, tenant: &TenantId, device_id: &str) -> Result<bool, PersistenceError>;

    async fn resolve_ids(
        &self,
        tenant: &TenantId,
        filter: DeviceFilter,
    ) -> Result<Vec<String>, PersistenceError>;

    /// Count unique matching tenant-owned devices, including those already in
    /// this fleet. Duplicate IDs never inflate counts; absent/foreign IDs count zero.
    async fn bulk_assign_fleet(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
        fleet_id: Option<i32>,
        updated_at: DateTime<Utc>,
    ) -> Result<usize, PersistenceError>;

    /// Count unique rows actually deleted. Retrying a completed deletion returns zero.
    async fn bulk_delete(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
    ) -> Result<usize, PersistenceError>;
}
