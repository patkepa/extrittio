#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRecord {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetSummary {
    pub fleet: FleetRecord,
    pub device_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetList {
    pub records: Vec<FleetSummary>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateFleetRecord {
    pub name: String,
}
