use std::sync::Arc;

use crate::persistence::{DatabaseComposition, DatabaseRuntime};
use extrittio_backend_core::RepositorySetInput;

use crate::database::TursoDatabase;

#[must_use]
pub(crate) fn create_runtime(database: Arc<TursoDatabase>) -> DatabaseComposition {
    let repositories = build_repositories(database.clone());
    DatabaseComposition::new(repositories, DatabaseRuntime::turso(database))
}

fn build_repositories(database: Arc<TursoDatabase>) -> RepositorySetInput {
    let zones = crate::database::turso_zones(&database);
    let roles = crate::database::turso_roles(&database);
    let users = crate::database::turso_users(&database);
    let api_keys = crate::database::turso_api_keys(&database);
    let telemetry = crate::database::turso_telemetry(&database);
    let device_ingress = crate::database::turso_device_ingress(&database);
    let events = crate::database::turso_events(&database);
    let logs = crate::database::turso_logs(&database);
    let commands = crate::database::turso_commands(&database);
    let configuration = crate::database::turso_configuration(&database);
    let shadows = crate::database::turso_shadows(&database);
    let fleets = crate::database::turso_fleets(&database);
    let device_blueprints = crate::database::turso_device_blueprints(&database);
    let alerts = crate::database::turso_alerts(&database);
    let outbox = crate::database::turso_outbox(&database);
    let rules = crate::database::turso_rules(&database);
    let devices = crate::database::turso_devices(&database);
    let device_types = crate::database::turso_device_types(&database);
    let bootstrap = crate::database::turso_bootstrap(&database);
    let certificates = crate::database::turso_certificates(&database);
    let metrics = crate::database::turso_metrics(&database);
    let audit = crate::database::turso_audit(&database);
    let analytics = crate::database::turso_analytics(&database);
    let dashboard = crate::database::turso_dashboard(&database);
    let activity = crate::database::turso_activity(&database);
    let firmware = crate::database::turso_firmware(&database);
    let ci_ingest = crate::database::turso_ci_ingest(&database);
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
        device_types,
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
        telemetry,
        users,
        zones,
    }
}
