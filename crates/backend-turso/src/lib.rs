#![forbid(unsafe_code)]

mod firmware;
pub use firmware::TursoFirmwareRepository;
pub mod api_keys;
mod fleets;
pub use fleets::TursoFleetRepository;
mod device_types;
pub use device_types::TursoDeviceTypeRepository;
mod bootstrap;
pub use bootstrap::TursoBootstrapRepository;
mod certificates;
pub use certificates::TursoCertificateRepository;
mod ci_ingest;
pub use ci_ingest::TursoCiIngestRepository;
mod database;
pub use api_keys::TursoApiKeyRepository;
mod error;
mod lifecycle;
mod maintenance;
mod migrations;
pub mod roles;
mod row;
pub mod users;
mod zones;

pub use database::{TursoConnectionHandles, TursoDatabase};
pub use lifecycle::TursoLifecycleError;
pub use maintenance::{LogicalArchiveInfo, TursoBackupInfo, TursoDatabaseInfo};
pub use migrations::LATEST_SCHEMA_VERSION;
pub use roles::TursoRoleRepository;
pub use users::TursoUserRepository;
pub use zones::TursoZoneRepository;

mod device_blueprints;
pub use device_blueprints::TursoDeviceBlueprintRepository;

mod devices;
pub use devices::TursoDeviceRepository;

mod rules;
pub use rules::TursoRuleRepository;

mod outbox;
pub use outbox::TursoOutboxRepository;

mod alerts;
pub use alerts::TursoAlertRepository;

mod rule_runtime;

mod shadows;
pub use shadows::TursoShadowRepository;

mod configuration;
pub use configuration::TursoConfigurationRepository;

mod commands;
pub use commands::TursoCommandRepository;

mod logs;
pub use logs::TursoLogRepository;

mod events;
pub use events::TursoEventRepository;

mod device_ingress;
pub use device_ingress::TursoDeviceIngressRepository;

mod telemetry;
pub use telemetry::TursoTelemetryRepository;

mod activity;
pub use activity::TursoActivityRepository;

mod dashboard;
pub use dashboard::TursoDashboardRepository;

mod analytics;
pub use analytics::TursoAnalyticsRepository;

mod audit;
pub use audit::TursoAuditRepository;

mod metrics;
pub use metrics::TursoMetricsRepository;
