use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::domains::device_types::types::DeviceTypeRecord;
use crate::domains::fleets::types::FleetRecord;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateDeviceRecord {
    pub id: String,
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub firmware: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateDeviceRecord {
    pub name: Option<String>,
    pub device_type_id: Option<i32>,
    pub fleet_id: Option<Option<i32>>,
    pub firmware: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}
