//! PostgreSQL persistence adapters for the backend application core.
#![forbid(unsafe_code)]

mod firmware;
mod firmware_sql;
pub use firmware::PostgresFirmwareRepository;

pub mod api_keys;
mod fleets;
pub use fleets::PostgresFleetRepository;
mod device_types;
pub use device_types::PostgresDeviceTypeRepository;
mod bootstrap;
pub use bootstrap::PostgresBootstrapRepository;
mod certificates;
pub use certificates::PostgresCertificateRepository;
mod ci_ingest;
pub use ci_ingest::PostgresCiIngestRepository;
pub mod executor;
pub use api_keys::PostgresApiKeyRepository;
pub mod migrations;
#[cfg(feature = "migration-bridge")]
#[doc(hidden)]
pub mod models;
#[cfg(not(feature = "migration-bridge"))]
#[allow(dead_code)]
mod models;
pub mod roles;
#[cfg(feature = "migration-bridge")]
#[doc(hidden)]
pub mod schema;
#[cfg(not(feature = "migration-bridge"))]
mod schema;
pub mod users;
pub mod zones;

mod error;

pub use executor::{PostgresExecutor, PostgresPool};
pub use migrations::{MIGRATIONS, PostgresMigrationError, run_pending_migrations};
pub use roles::PostgresRoleRepository;
pub use users::PostgresUserRepository;
pub use zones::PostgresZoneRepository;

/// Adapter name used by the runtime composition root. The walking skeleton has
/// one repository today; later slices can replace this alias with a façade
/// without changing the core port implementation.
pub type PostgresAdapter = PostgresZoneRepository;

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
#[cfg(feature = "migration-bridge")]
#[doc(hidden)]
pub use rule_runtime::evaluate_rules_in_transaction;

#[cfg(feature = "migration-bridge")]
#[doc(hidden)]
pub use outbox::enqueue_actions_in_transaction;

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

mod telemetry;
pub use telemetry::PostgresTelemetryRepository;
mod telemetry_sql;

mod activity;
pub use activity::PostgresActivityRepository;

mod dashboard;
pub use dashboard::PostgresDashboardRepository;

mod analytics;
pub use analytics::PostgresAnalyticsRepository;

mod audit;
pub use audit::PostgresAuditRepository;
mod audit_sql;
