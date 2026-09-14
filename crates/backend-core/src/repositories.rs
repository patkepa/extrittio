use std::sync::Arc;

use crate::{RoleRepository, UserRepository, ZoneRepository};

/// Named business-port dependencies used to construct a [`RepositorySet`].
///
/// Adapters construct this value after they have created their concrete
/// repositories. Database connection, migration, health, backup, and other
/// lifecycle capabilities intentionally do not belong here.
pub struct RepositorySetInput {
    pub api_keys: Arc<dyn crate::ApiKeyRepository>,
    pub device_blueprints: Arc<dyn crate::device_blueprints::DeviceBlueprintRepository>,
    pub devices: Arc<dyn crate::devices::DeviceRepository>,
    pub fleets: Arc<dyn crate::fleets::FleetRepository>,
    pub device_types: Arc<dyn crate::device_types::DeviceTypeRepository>,
    pub ci_ingest: Arc<dyn crate::CiIngestRepository>,
    pub certificates: Arc<dyn crate::certificates::CertificateRepository>,
    pub dashboard: Arc<dyn crate::dashboard::DashboardReadRepository>,
    pub activity: Arc<dyn crate::activity::ActivityRepository>,
    pub firmware: Arc<dyn crate::firmware::FirmwareRepository>,
    pub telemetry: Arc<dyn crate::telemetry::TelemetryRepository>,
    pub events: Arc<dyn crate::events::DeviceEventRepository>,
    pub logs: Arc<dyn crate::logs::LogRepository>,
    pub commands: Arc<dyn crate::commands::CommandRepository>,
    pub configuration: Arc<dyn crate::configuration::DeviceConfigRepository>,
    pub shadows: Arc<dyn crate::shadows::ShadowRepository>,
    pub alerts: Arc<dyn crate::alerts::AlertRepository>,
    pub outbox: Arc<dyn crate::outbox::OutboxRepository>,
    pub rules: Arc<dyn crate::rules::RuleRepository>,
    pub roles: Arc<dyn RoleRepository>,
    pub users: Arc<dyn UserRepository>,
    pub zones: Arc<dyn ZoneRepository>,
}

/// Complete set of business persistence ports available to the application.
///
/// Fields stay private so transports and runtime code cannot use this as a
/// service locator. Only application modules decompose the set into the narrow
/// use-case façades they own.
#[derive(Clone)]
pub struct RepositorySet {
    api_keys: Arc<dyn crate::ApiKeyRepository>,
    device_blueprints: Arc<dyn crate::device_blueprints::DeviceBlueprintRepository>,
    devices: Arc<dyn crate::devices::DeviceRepository>,
    fleets: Arc<dyn crate::fleets::FleetRepository>,
    device_types: Arc<dyn crate::device_types::DeviceTypeRepository>,
    ci_ingest: Arc<dyn crate::CiIngestRepository>,
    certificates: Arc<dyn crate::certificates::CertificateRepository>,
    dashboard: Arc<dyn crate::dashboard::DashboardReadRepository>,
    activity: Arc<dyn crate::activity::ActivityRepository>,
    firmware: Arc<dyn crate::firmware::FirmwareRepository>,
    telemetry: Arc<dyn crate::telemetry::TelemetryRepository>,
    events: Arc<dyn crate::events::DeviceEventRepository>,
    logs: Arc<dyn crate::logs::LogRepository>,
    commands: Arc<dyn crate::commands::CommandRepository>,
    configuration: Arc<dyn crate::configuration::DeviceConfigRepository>,
    shadows: Arc<dyn crate::shadows::ShadowRepository>,
    alerts: Arc<dyn crate::alerts::AlertRepository>,
    outbox: Arc<dyn crate::outbox::OutboxRepository>,
    rules: Arc<dyn crate::rules::RuleRepository>,
    roles: Arc<dyn RoleRepository>,
    users: Arc<dyn UserRepository>,
    zones: Arc<dyn ZoneRepository>,
}

impl RepositorySet {
    #[must_use]
    pub fn new(input: RepositorySetInput) -> Self {
        Self {
            api_keys: input.api_keys,
            device_blueprints: input.device_blueprints,
            devices: input.devices,
            fleets: input.fleets,
            device_types: input.device_types,
            ci_ingest: input.ci_ingest,
            certificates: input.certificates,
            dashboard: input.dashboard,
            activity: input.activity,
            firmware: input.firmware,
            telemetry: input.telemetry,
            events: input.events,
            logs: input.logs,
            commands: input.commands,
            configuration: input.configuration,
            shadows: input.shadows,
            alerts: input.alerts,
            outbox: input.outbox,
            rules: input.rules,
            roles: input.roles,
            users: input.users,
            zones: input.zones,
        }
    }

    pub(crate) fn into_parts(self) -> RepositorySetParts {
        RepositorySetParts {
            api_keys: self.api_keys,
            device_blueprints: self.device_blueprints,
            devices: self.devices,
            fleets: self.fleets,
            device_types: self.device_types,
            ci_ingest: self.ci_ingest,
            certificates: self.certificates,
            dashboard: self.dashboard,
            activity: self.activity,
            firmware: self.firmware,
            telemetry: self.telemetry,
            events: self.events,
            logs: self.logs,
            commands: self.commands,
            configuration: self.configuration,
            shadows: self.shadows,
            alerts: self.alerts,
            outbox: self.outbox,
            rules: self.rules,
            roles: self.roles,
            users: self.users,
            zones: self.zones,
        }
    }
}

pub(crate) struct RepositorySetParts {
    pub(crate) api_keys: Arc<dyn crate::ApiKeyRepository>,
    pub(crate) device_blueprints: Arc<dyn crate::device_blueprints::DeviceBlueprintRepository>,
    pub(crate) devices: Arc<dyn crate::devices::DeviceRepository>,
    pub(crate) fleets: Arc<dyn crate::fleets::FleetRepository>,
    pub(crate) device_types: Arc<dyn crate::device_types::DeviceTypeRepository>,
    pub(crate) ci_ingest: Arc<dyn crate::CiIngestRepository>,
    pub(crate) certificates: Arc<dyn crate::certificates::CertificateRepository>,
    pub(crate) dashboard: Arc<dyn crate::dashboard::DashboardReadRepository>,
    pub(crate) activity: Arc<dyn crate::activity::ActivityRepository>,
    pub(crate) firmware: Arc<dyn crate::firmware::FirmwareRepository>,
    pub(crate) telemetry: Arc<dyn crate::telemetry::TelemetryRepository>,
    pub(crate) events: Arc<dyn crate::events::DeviceEventRepository>,
    pub(crate) logs: Arc<dyn crate::logs::LogRepository>,
    pub(crate) commands: Arc<dyn crate::commands::CommandRepository>,
    pub(crate) configuration: Arc<dyn crate::configuration::DeviceConfigRepository>,
    pub(crate) shadows: Arc<dyn crate::shadows::ShadowRepository>,
    pub(crate) alerts: Arc<dyn crate::alerts::AlertRepository>,
    pub(crate) outbox: Arc<dyn crate::outbox::OutboxRepository>,
    pub(crate) rules: Arc<dyn crate::rules::RuleRepository>,
    pub(crate) roles: Arc<dyn RoleRepository>,
    pub(crate) users: Arc<dyn UserRepository>,
    pub(crate) zones: Arc<dyn ZoneRepository>,
}
