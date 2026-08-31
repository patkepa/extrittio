use std::sync::Arc;

use crate::domains::activity::repository::ActivityRepository;
use crate::domains::alerts::port::AlertRepository;
use crate::domains::analytics::repository::AnalyticsRepository;
use crate::domains::audit::port::AuditRepository;
use crate::domains::commands::port::CommandRepository;
use crate::domains::configuration::repository::DeviceConfigRepository;
use crate::domains::dashboard::repository::DashboardReadRepository;
use crate::domains::device_blueprints::repository::DeviceBlueprintRepository;
use crate::domains::device_types::repository::DeviceTypeRepository;
use crate::domains::devices::repository::DeviceRepository;
use crate::domains::events::repository::DeviceEventRepository;
use crate::domains::firmware::port::FirmwareRepository;
use crate::domains::fleets::repository::FleetRepository;
use crate::domains::identity::api_key_repository::ApiKeyRepository;
use crate::domains::identity::certificate_repository::CertificateRepository;
use crate::domains::logs::port::LogRepository;
use crate::domains::operations::metrics_repository::MetricsRepository;
use crate::domains::operations::outbox_repository::OutboxRepository;
use crate::domains::rules::port::RuleRepository;
use crate::domains::shadows::repository::ShadowRepository;
use crate::domains::telemetry::port::TelemetryRepository;
use extrittio_backend_core::{
    RoleRepository, RuleZoneSnapshotRepository, UserRepository, ZoneRepository,
};

pub mod backend;
pub mod bootstrap;
pub mod error;
pub mod factory;
#[cfg(feature = "postgres")]
pub mod postgres;
pub mod runtime;
#[cfg(feature = "turso")]
pub mod turso;

pub use backend::{BackendCapabilities, BackendDescriptor, BackendKind};
pub use bootstrap::{BootstrapOwner, BootstrapRepository, BuiltinDeviceType, SeedOwnerOutcome};
pub use error::{ConstraintName, PersistenceError};
pub use runtime::{DatabaseHealth, DatabaseRuntime, LifecycleError};

/// Cloneable collection of backend-neutral persistence ports owned by
/// `AppState`. Additional domain ports are added as their PostgreSQL code is
/// extracted.
#[derive(Clone)]
pub struct RepositorySet {
    pub activity: Arc<dyn ActivityRepository>,
    pub analytics: Arc<dyn AnalyticsRepository>,
    pub api_keys: Arc<dyn ApiKeyRepository>,
    pub alerts: Arc<dyn AlertRepository>,
    pub audit: Arc<dyn AuditRepository>,
    pub bootstrap: Arc<dyn BootstrapRepository>,
    pub certificates: Arc<dyn CertificateRepository>,
    pub commands: Arc<dyn CommandRepository>,
    pub configuration: Arc<dyn DeviceConfigRepository>,
    pub dashboard: Arc<dyn DashboardReadRepository>,
    pub device_blueprints: Arc<dyn DeviceBlueprintRepository>,
    pub device_types: Arc<dyn DeviceTypeRepository>,
    pub devices: Arc<dyn DeviceRepository>,
    pub events: Arc<dyn DeviceEventRepository>,
    pub fleets: Arc<dyn FleetRepository>,
    pub firmware: Arc<dyn FirmwareRepository>,
    pub logs: Arc<dyn LogRepository>,
    pub metrics: Arc<dyn MetricsRepository>,
    pub outbox: Arc<dyn OutboxRepository>,
    pub roles: Arc<dyn RoleRepository>,
    pub rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
    pub rules: Arc<dyn RuleRepository>,
    pub shadows: Arc<dyn ShadowRepository>,
    pub telemetry: Arc<dyn TelemetryRepository>,
    pub users: Arc<dyn UserRepository>,
    pub zones: Arc<dyn ZoneRepository>,
}

pub struct RepositoryPorts {
    pub activity: Arc<dyn ActivityRepository>,
    pub analytics: Arc<dyn AnalyticsRepository>,
    pub api_keys: Arc<dyn ApiKeyRepository>,
    pub alerts: Arc<dyn AlertRepository>,
    pub audit: Arc<dyn AuditRepository>,
    pub bootstrap: Arc<dyn BootstrapRepository>,
    pub certificates: Arc<dyn CertificateRepository>,
    pub commands: Arc<dyn CommandRepository>,
    pub configuration: Arc<dyn DeviceConfigRepository>,
    pub dashboard: Arc<dyn DashboardReadRepository>,
    pub device_blueprints: Arc<dyn DeviceBlueprintRepository>,
    pub device_types: Arc<dyn DeviceTypeRepository>,
    pub devices: Arc<dyn DeviceRepository>,
    pub events: Arc<dyn DeviceEventRepository>,
    pub fleets: Arc<dyn FleetRepository>,
    pub firmware: Arc<dyn FirmwareRepository>,
    pub logs: Arc<dyn LogRepository>,
    pub metrics: Arc<dyn MetricsRepository>,
    pub outbox: Arc<dyn OutboxRepository>,
    pub roles: Arc<dyn RoleRepository>,
    pub rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
    pub rules: Arc<dyn RuleRepository>,
    pub shadows: Arc<dyn ShadowRepository>,
    pub telemetry: Arc<dyn TelemetryRepository>,
    pub users: Arc<dyn UserRepository>,
    pub zones: Arc<dyn ZoneRepository>,
}

impl RepositorySet {
    #[must_use]
    pub fn new(ports: RepositoryPorts) -> Self {
        Self {
            activity: ports.activity,
            analytics: ports.analytics,
            api_keys: ports.api_keys,
            alerts: ports.alerts,
            audit: ports.audit,
            bootstrap: ports.bootstrap,
            certificates: ports.certificates,
            commands: ports.commands,
            configuration: ports.configuration,
            dashboard: ports.dashboard,
            device_blueprints: ports.device_blueprints,
            device_types: ports.device_types,
            devices: ports.devices,
            events: ports.events,
            fleets: ports.fleets,
            firmware: ports.firmware,
            logs: ports.logs,
            metrics: ports.metrics,
            outbox: ports.outbox,
            roles: ports.roles,
            rule_zone_snapshots: ports.rule_zone_snapshots,
            rules: ports.rules,
            shadows: ports.shadows,
            telemetry: ports.telemetry,
            users: ports.users,
            zones: ports.zones,
        }
    }
}
