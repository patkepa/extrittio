//! Host composition boundary for concrete database adapters.
//!
//! Legacy persistence modules consume only backend-neutral ports; construction
//! of extracted adapters is centralized here until the remaining repositories
//! move into their dedicated crates.

#[cfg(feature = "postgres")]
pub(crate) fn postgres_api_keys(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::ApiKeyRepository> {
    Arc::new(extrittio_backend_postgres::PostgresApiKeyRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_api_keys(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::ApiKeyRepository> {
    Arc::new(
        extrittio_backend_turso::TursoApiKeyRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(any(feature = "postgres", feature = "turso"))]
use std::sync::Arc;

#[cfg(any(feature = "postgres", feature = "turso"))]
use extrittio_backend_core::{
    RoleRepository, RuleZoneSnapshotRepository, UserRepository, ZoneRepository,
};

#[cfg(feature = "postgres")]
pub use extrittio_backend_postgres::{PostgresExecutor, PostgresPool, models, schema};

#[cfg(feature = "turso")]
mod turso;
#[cfg(feature = "turso")]
pub use turso::{LogicalArchiveInfo, TursoBackupInfo, TursoDatabase, TursoDatabaseInfo};

#[cfg(feature = "postgres")]
pub(crate) fn postgres_zones(
    pool: &crate::persistence::postgres::executor::PostgresPool,
) -> (Arc<dyn ZoneRepository>, Arc<dyn RuleZoneSnapshotRepository>) {
    let adapter =
        Arc::new(extrittio_backend_postgres::PostgresZoneRepository::from_pool(pool.clone()));
    (adapter.clone(), adapter)
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_roles(
    pool: &crate::persistence::postgres::executor::PostgresPool,
) -> Arc<dyn RoleRepository> {
    Arc::new(extrittio_backend_postgres::PostgresRoleRepository::from_pool(pool.clone()))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_users(
    pool: &crate::persistence::postgres::executor::PostgresPool,
) -> Arc<dyn UserRepository> {
    Arc::new(extrittio_backend_postgres::PostgresUserRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_zones(
    database: &crate::persistence::turso::TursoDatabase,
) -> (Arc<dyn ZoneRepository>, Arc<dyn RuleZoneSnapshotRepository>) {
    let adapter = Arc::new(extrittio_backend_turso::TursoZoneRepository::from_handles(
        database.shared_handles(),
    ));
    (adapter.clone(), adapter)
}

#[cfg(feature = "turso")]
pub(crate) fn turso_roles(
    database: &crate::persistence::turso::TursoDatabase,
) -> Arc<dyn RoleRepository> {
    Arc::new(extrittio_backend_turso::TursoRoleRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_users(
    database: &crate::persistence::turso::TursoDatabase,
) -> Arc<dyn UserRepository> {
    Arc::new(extrittio_backend_turso::TursoUserRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_ci_ingest(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::CiIngestRepository> {
    Arc::new(extrittio_backend_postgres::PostgresCiIngestRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_ci_ingest(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::CiIngestRepository> {
    Arc::new(
        extrittio_backend_turso::TursoCiIngestRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_certificates(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::certificates::CertificateRepository> {
    Arc::new(extrittio_backend_postgres::PostgresCertificateRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_certificates(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::certificates::CertificateRepository> {
    Arc::new(
        extrittio_backend_turso::TursoCertificateRepository::from_handles(
            database.shared_handles(),
        ),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_bootstrap(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::bootstrap::BootstrapRepository> {
    Arc::new(extrittio_backend_postgres::PostgresBootstrapRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_bootstrap(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::bootstrap::BootstrapRepository> {
    Arc::new(
        extrittio_backend_turso::TursoBootstrapRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_device_types(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::device_types::DeviceTypeRepository> {
    Arc::new(extrittio_backend_postgres::PostgresDeviceTypeRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_device_types(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::device_types::DeviceTypeRepository> {
    Arc::new(
        extrittio_backend_turso::TursoDeviceTypeRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_fleets(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::fleets::FleetRepository> {
    Arc::new(extrittio_backend_postgres::PostgresFleetRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_fleets(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::fleets::FleetRepository> {
    Arc::new(extrittio_backend_turso::TursoFleetRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_device_blueprints(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::device_blueprints::DeviceBlueprintRepository> {
    Arc::new(extrittio_backend_postgres::PostgresDeviceBlueprintRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_device_blueprints(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::device_blueprints::DeviceBlueprintRepository> {
    Arc::new(
        extrittio_backend_turso::TursoDeviceBlueprintRepository::from_handles(
            database.shared_handles(),
        ),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_devices(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::devices::DeviceRepository> {
    Arc::new(extrittio_backend_postgres::PostgresDeviceRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_devices(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::devices::DeviceRepository> {
    Arc::new(
        extrittio_backend_turso::TursoDeviceRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_rules(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::rules::RuleRepository> {
    Arc::new(extrittio_backend_postgres::PostgresRuleRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_rules(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::rules::RuleRepository> {
    Arc::new(extrittio_backend_turso::TursoRuleRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_outbox(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::outbox::OutboxRepository> {
    Arc::new(extrittio_backend_postgres::PostgresOutboxRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_outbox(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::outbox::OutboxRepository> {
    Arc::new(
        extrittio_backend_turso::TursoOutboxRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_alerts(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::alerts::AlertRepository> {
    Arc::new(extrittio_backend_postgres::PostgresAlertRepository::from_pool(pool.clone()))
}

#[cfg(feature = "turso")]
pub(crate) fn turso_alerts(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::alerts::AlertRepository> {
    Arc::new(extrittio_backend_turso::TursoAlertRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_shadows(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::shadows::ShadowRepository> {
    Arc::new(extrittio_backend_postgres::PostgresShadowRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_shadows(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::shadows::ShadowRepository> {
    Arc::new(
        extrittio_backend_turso::TursoShadowRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_configuration(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::configuration::DeviceConfigRepository> {
    Arc::new(extrittio_backend_postgres::PostgresConfigurationRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_configuration(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::configuration::DeviceConfigRepository> {
    Arc::new(
        extrittio_backend_turso::TursoConfigurationRepository::from_handles(
            database.shared_handles(),
        ),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_commands(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::commands::CommandRepository> {
    Arc::new(extrittio_backend_postgres::PostgresCommandRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_commands(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::commands::CommandRepository> {
    Arc::new(
        extrittio_backend_turso::TursoCommandRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_logs(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::logs::LogRepository> {
    Arc::new(extrittio_backend_postgres::PostgresLogRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_logs(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::logs::LogRepository> {
    Arc::new(extrittio_backend_turso::TursoLogRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_events(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::events::DeviceEventRepository> {
    Arc::new(extrittio_backend_postgres::PostgresEventRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_events(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::events::DeviceEventRepository> {
    Arc::new(extrittio_backend_turso::TursoEventRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_device_ingress(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::device_ingress::DeviceIngressRepository> {
    Arc::new(extrittio_backend_postgres::PostgresDeviceIngressRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_device_ingress(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::device_ingress::DeviceIngressRepository> {
    Arc::new(
        extrittio_backend_turso::TursoDeviceIngressRepository::from_handles(
            database.shared_handles(),
        ),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_telemetry(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::telemetry::TelemetryRepository> {
    Arc::new(extrittio_backend_postgres::PostgresTelemetryRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_telemetry(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::telemetry::TelemetryRepository> {
    Arc::new(
        extrittio_backend_turso::TursoTelemetryRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "turso")]
pub(crate) fn turso_firmware(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::firmware::FirmwareRepository> {
    Arc::new(
        extrittio_backend_turso::TursoFirmwareRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_firmware(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::firmware::FirmwareRepository> {
    Arc::new(extrittio_backend_postgres::PostgresFirmwareRepository::from_pool(pool.clone()))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_activity(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::activity::ActivityRepository> {
    Arc::new(extrittio_backend_postgres::PostgresActivityRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_activity(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::activity::ActivityRepository> {
    Arc::new(
        extrittio_backend_turso::TursoActivityRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_dashboard(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::dashboard::DashboardReadRepository> {
    Arc::new(extrittio_backend_postgres::PostgresDashboardRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_dashboard(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::dashboard::DashboardReadRepository> {
    Arc::new(
        extrittio_backend_turso::TursoDashboardRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_analytics(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::analytics::AnalyticsRepository> {
    Arc::new(extrittio_backend_postgres::PostgresAnalyticsRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_analytics(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::analytics::AnalyticsRepository> {
    Arc::new(
        extrittio_backend_turso::TursoAnalyticsRepository::from_handles(database.shared_handles()),
    )
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_audit(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::audit::AuditRepository> {
    Arc::new(extrittio_backend_postgres::PostgresAuditRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_audit(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::audit::AuditRepository> {
    Arc::new(extrittio_backend_turso::TursoAuditRepository::from_handles(
        database.shared_handles(),
    ))
}

#[cfg(feature = "postgres")]
pub(crate) fn postgres_metrics(
    pool: &PostgresPool,
) -> Arc<dyn extrittio_backend_core::metrics::MetricsRepository> {
    Arc::new(extrittio_backend_postgres::PostgresMetricsRepository::from_pool(pool.clone()))
}
#[cfg(feature = "turso")]
pub(crate) fn turso_metrics(
    database: &TursoDatabase,
) -> Arc<dyn extrittio_backend_core::metrics::MetricsRepository> {
    Arc::new(
        extrittio_backend_turso::TursoMetricsRepository::from_handles(database.shared_handles()),
    )
}
