#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceTypeRecord {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub color_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceTypeList {
    pub records: Vec<DeviceTypeRecord>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateDeviceTypeRecord {
    pub name: String,
    pub icon: String,
    pub color_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateDeviceTypeRecord {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteDeviceTypeOutcome {
    Deleted,
    NotFound,
    InUse { device_count: i64 },
}
