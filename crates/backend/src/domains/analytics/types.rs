use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsMetricSelector {
    pub blueprint_id: String,
    pub stream_key: String,
    pub field_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsMetric {
    pub selector: AnalyticsMetricSelector,
    pub blueprint_key: String,
    pub blueprint_name: String,
    pub label: String,
    pub unit: Option<String>,
    pub value_type: String,
    pub aggregates: Vec<String>,
    pub precision: Option<u8>,
}

impl AnalyticsMetric {
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}.{}", self.selector.stream_key, self.selector.field_path)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsBlueprintRevision {
    pub blueprint_id: String,
    pub blueprint_key: String,
    pub blueprint_name: String,
    pub revision_id: String,
    pub revision: i32,
    pub document: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnalyticsScope {
    pub device_type_ids: Vec<i32>,
    pub fleet_ids: Vec<i32>,
    pub device_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsSeriesMode {
    PerDevice,
    FleetMean,
    MeanAndRange,
    LatestRanking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsWeighting {
    EqualDevice,
    Sample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsDataSource {
    BlueprintMetricSamples,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsRequest {
    pub scope: AnalyticsScope,
    pub metric: AnalyticsMetricSelector,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub bucket_seconds: Option<i64>,
    pub mode: AnalyticsSeriesMode,
    pub weighting: AnalyticsWeighting,
    pub max_points_per_series: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsQuery {
    pub scope: AnalyticsScope,
    pub metric: AnalyticsMetric,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub bucket_seconds: i64,
    pub max_devices: usize,
    pub max_rows: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsBucket {
    pub device_id: String,
    pub device_name: String,
    pub bucket_start: NaiveDateTime,
    pub sample_count: i64,
    pub average: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub latest: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsQueryData {
    pub selected_devices: usize,
    pub compatible_devices: usize,
    pub devices: Vec<AnalyticsDevice>,
    pub buckets: Vec<AnalyticsBucket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsSeriesKind {
    Device,
    Mean,
    Minimum,
    Maximum,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalyticsPoint {
    pub timestamp: NaiveDateTime,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsStats {
    pub minimum: f64,
    pub maximum: f64,
    pub average: f64,
    pub latest: f64,
    pub sample_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsSeries {
    pub id: String,
    pub label: String,
    pub kind: AnalyticsSeriesKind,
    pub device_id: Option<String>,
    pub points: Vec<AnalyticsPoint>,
    pub stats: AnalyticsStats,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsDeviceStats {
    pub device_id: String,
    pub device_name: String,
    pub stats: Option<AnalyticsStats>,
    pub coverage_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsCoveragePoint {
    pub timestamp: NaiveDateTime,
    pub reporting_devices: usize,
    pub selected_devices: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsResult {
    pub metric: AnalyticsMetric,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub bucket_seconds: i64,
    pub source: AnalyticsDataSource,
    pub weighting: AnalyticsWeighting,
    pub selected_devices: usize,
    pub compatible_devices: usize,
    pub series: Vec<AnalyticsSeries>,
    pub devices: Vec<AnalyticsDeviceStats>,
    pub coverage: Vec<AnalyticsCoveragePoint>,
    pub stats: Option<AnalyticsStats>,
    pub warnings: Vec<String>,
}
