//! PostgreSQL persistence adapters for the backend application core.
#![forbid(unsafe_code)]

mod firmware;
mod firmware_sql;
pub use firmware::PostgresFirmwareRepository;

pub mod api_keys;
mod fleets;
pub use fleets::PostgresFleetRepository;
mod bootstrap;
pub use bootstrap::PostgresBootstrapRepository;
mod certificates;
pub use certificates::PostgresCertificateRepository;
mod ci_ingest;
pub use ci_ingest::PostgresCiIngestRepository;
pub mod executor;
pub use api_keys::PostgresApiKeyRepository;
pub mod migrations;
#[allow(dead_code)]
mod models;
pub mod roles;
mod schema;
pub mod users;
pub mod zones;

mod error;
mod lifecycle;
pub use lifecycle::{PostgresLifecycle, PostgresLifecycleError};

pub use executor::{PostgresExecutor, PostgresPool};
pub use migrations::{MIGRATIONS, PostgresMigrationError, run_pending_migrations};
pub use roles::PostgresRoleRepository;
pub use users::PostgresUserRepository;
pub use zones::PostgresZoneRepository;

mod device_blueprints;
pub use device_blueprints::PostgresDeviceBlueprintRepository;

mod devices;
pub use devices::PostgresDeviceRepository;

mod rules;
pub use rules::PostgresRuleRepository;
mod rules_sql;

mod outbox;
pub use outbox::PostgresOutboxRepository;
mod outbox_sql;

mod alerts;
pub use alerts::PostgresAlertRepository;
mod alerts_sql;

mod rule_runtime;

mod shadows;
pub use shadows::PostgresShadowRepository;

mod configuration;
pub use configuration::PostgresConfigurationRepository;

mod commands;
pub use commands::PostgresCommandRepository;

mod logs;
pub use logs::PostgresLogRepository;

mod events;
pub use events::PostgresEventRepository;

mod device_ingress;
pub use device_ingress::PostgresDeviceIngressRepository;

mod activity;
pub use activity::PostgresActivityRepository;

mod dashboard;
pub use dashboard::PostgresDashboardRepository;

mod analytics;
pub use analytics::PostgresAnalyticsRepository;

mod audit;
pub use audit::PostgresAuditRepository;
mod audit_sql;

mod metrics;
pub use metrics::PostgresMetricsRepository;
mod metrics_sql;
