use std::sync::Arc;

use crate::persistence::{DatabaseRuntime, RepositoryPorts, RepositorySet};
use executor::PostgresPool;

mod activity;
mod alerts;
mod analytics;
mod api_keys;
mod audit;
mod bootstrap;
mod certificates;
mod commands;
mod configuration;
mod dashboard;
mod device_blueprints;
mod device_types;
mod devices;
mod events;
pub mod executor;
mod firmware;
mod fleets;
pub(crate) mod lifecycle;
mod logs;
mod metrics;
mod outbox;
mod rules;
mod shadows;
mod telemetry;
mod users;

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
    let adapter = Arc::new(PostgresAdapter::new(pool));
    RepositorySet::new(RepositoryPorts {
        activity: adapter.clone(),
        analytics: adapter.clone(),
        api_keys: adapter.clone(),
        alerts: adapter.clone(),
        audit: adapter.clone(),
        bootstrap: adapter.clone(),
        certificates: adapter.clone(),
        commands: adapter.clone(),
        configuration: adapter.clone(),
        dashboard: adapter.clone(),
        device_blueprints: adapter.clone(),
        device_types: adapter.clone(),
        devices: adapter.clone(),
        events: adapter.clone(),
        fleets: adapter.clone(),
        firmware: adapter.clone(),
        logs: adapter.clone(),
        metrics: adapter.clone(),
        outbox: adapter.clone(),
        roles,
        rule_zone_snapshots,
        rules: adapter.clone(),
        shadows: adapter.clone(),
        telemetry: adapter.clone(),
        users: adapter.clone(),
        zones,
    })
}
