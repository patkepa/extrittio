use std::sync::Arc;

use crate::persistence::{BackendDescriptor, Persistence, PersistencePorts};
use crate::state::DbPool;

mod alerts;
mod api_keys;
mod audit;
mod bootstrap;
mod certificates;
mod commands;
mod configuration;
mod dashboard;
mod device_types;
mod devices;
pub mod executor;
mod firmware;
mod fleets;
mod logs;
mod roles;
mod shadows;
mod telemetry;
mod users;
mod zones;

/// Shared PostgreSQL adapter object. It implements multiple domain ports while
/// owning one executor/pool boundary.
#[derive(Clone)]
pub struct PostgresAdapter {
    executor: executor::PostgresExecutor,
}

impl PostgresAdapter {
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        Self {
            executor: executor::PostgresExecutor::new(pool),
        }
    }
}

#[must_use]
pub fn create_persistence(pool: DbPool) -> Persistence {
    let adapter = Arc::new(PostgresAdapter::new(pool));
    Persistence::new(
        BackendDescriptor::postgres(),
        PersistencePorts {
            api_keys: adapter.clone(),
            alerts: adapter.clone(),
            audit: adapter.clone(),
            bootstrap: adapter.clone(),
            certificates: adapter.clone(),
            commands: adapter.clone(),
            configuration: adapter.clone(),
            dashboard: adapter.clone(),
            device_types: adapter.clone(),
            devices: adapter.clone(),
            fleets: adapter.clone(),
            firmware: adapter.clone(),
            logs: adapter.clone(),
            roles: adapter.clone(),
            shadows: adapter.clone(),
            telemetry: adapter.clone(),
            users: adapter.clone(),
            zones: adapter,
        },
    )
}
