use std::sync::Arc;

use crate::persistence::{DatabaseRuntime, RepositorySet};
use executor::PostgresPool;

mod activity;
mod analytics;
mod audit;
mod dashboard;
mod devices;
mod events;
pub mod executor;
mod firmware;
pub(crate) mod lifecycle;
mod logs;
mod metrics;
mod outbox;
mod telemetry;

/// Shared PostgreSQL adapter object. It implements multiple domain ports while
/// owning one executor/pool boundary.
#[derive(Clone)]
pub struct PostgresAdapter {
    executor: executor::PostgresExecutor,
}

impl PostgresAdapter {
    #[must_use]
    pub fn new(pool: PostgresPool) -> Self {
        Self {
            executor: executor::PostgresExecutor::new(pool),
        }
    }
}

#[must_use]
pub fn create_repositories(pool: PostgresPool) -> RepositorySet {
    build_repositories(pool)
}

#[must_use]
pub fn create_runtime(pool: PostgresPool) -> DatabaseRuntime {
    let lifecycle = lifecycle::PostgresLifecycle::new(pool.clone());
    DatabaseRuntime::postgres(build_repositories(pool), lifecycle)
}

fn build_repositories(pool: PostgresPool) -> RepositorySet {
    let (zones, rule_zone_snapshots) = crate::database::postgres_zones(&pool);
    let roles = crate::database::postgres_roles(&pool);
    let users = crate::database::postgres_users(&pool);
    let api_keys = crate::database::postgres_api_keys(&pool);
    let commands = crate::database::postgres_commands(&pool);
    let configuration = crate::database::postgres_configuration(&pool);
    let shadows = crate::database::postgres_shadows(&pool);
    let fleets = crate::database::postgres_fleets(&pool);
    let device_blueprints = crate::database::postgres_device_blueprints(&pool);
    let alerts = crate::database::postgres_alerts(&pool);
    let outbox = crate::database::postgres_outbox(&pool);
    let rules = crate::database::postgres_rules(&pool);
    let devices = crate::database::postgres_devices(&pool);
    let device_types = crate::database::postgres_device_types(&pool);
    let bootstrap = crate::database::postgres_bootstrap(&pool);
    let certificates = crate::database::postgres_certificates(&pool);
    let ci_ingest = crate::database::postgres_ci_ingest(&pool);
    let adapter = Arc::new(PostgresAdapter::new(pool));
    RepositorySet {
        activity: adapter.clone(),
        analytics: adapter.clone(),
        api_keys,
        ci_ingest,
        alerts,
        audit: adapter.clone(),
        bootstrap,
        certificates,
        commands,
        configuration,
        dashboard: adapter.clone(),
        device_blueprints,
        device_types,
        devices,
        device_ingress: adapter.clone(),
        events: adapter.clone(),
        fleets,
        firmware: adapter.clone(),
        logs: adapter.clone(),
        metrics: adapter.clone(),
        outbox,
        roles,
        rule_zone_snapshots,
        rules,
        shadows,
        telemetry: adapter.clone(),
        users,
        zones,
    }
}
