use crate::database::PostgresPool;
use crate::persistence::{DatabaseComposition, DatabaseRuntime};
use extrittio_backend_core::RepositorySetInput;

#[must_use]
pub(crate) fn create_runtime(pool: PostgresPool) -> DatabaseComposition {
    let lifecycle = crate::database::PostgresLifecycle::new(pool.clone());
    DatabaseComposition::new(
        build_repositories(pool),
        DatabaseRuntime::postgres(lifecycle),
    )
}

fn build_repositories(pool: PostgresPool) -> RepositorySetInput {
    let zones = crate::database::postgres_zones(&pool);
    let roles = crate::database::postgres_roles(&pool);
    let users = crate::database::postgres_users(&pool);
    let api_keys = crate::database::postgres_api_keys(&pool);
    let device_ingress = crate::database::postgres_device_ingress(&pool);
    let events = crate::database::postgres_events(&pool);
    let logs = crate::database::postgres_logs(&pool);
    let commands = crate::database::postgres_commands(&pool);
    let configuration = crate::database::postgres_configuration(&pool);
    let shadows = crate::database::postgres_shadows(&pool);
    let fleets = crate::database::postgres_fleets(&pool);
    let device_blueprints = crate::database::postgres_device_blueprints(&pool);
    let alerts = crate::database::postgres_alerts(&pool);
    let outbox = crate::database::postgres_outbox(&pool);
    let rules = crate::database::postgres_rules(&pool);
    let devices = crate::database::postgres_devices(&pool);
    let bootstrap = crate::database::postgres_bootstrap(&pool);
    let certificates = crate::database::postgres_certificates(&pool);
    let metrics = crate::database::postgres_metrics(&pool);
    let audit = crate::database::postgres_audit(&pool);
    let analytics = crate::database::postgres_analytics(&pool);
    let dashboard = crate::database::postgres_dashboard(&pool);
    let activity = crate::database::postgres_activity(&pool);
    let firmware = crate::database::postgres_firmware(&pool);
    let ci_ingest = crate::database::postgres_ci_ingest(&pool);
    RepositorySetInput {
        activity,
        analytics,
        api_keys,
        ci_ingest,
        alerts,
        audit,
        bootstrap,
        certificates,
        commands,
        configuration,
        dashboard,
        device_blueprints,
        devices,
        device_ingress,
        events,
        fleets,
        firmware,
        logs,
        metrics,
        outbox,
        roles,
        rules,
        shadows,
        users,
        zones,
    }
}
