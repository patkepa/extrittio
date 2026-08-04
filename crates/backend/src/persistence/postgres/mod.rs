use std::sync::Arc;

use crate::persistence::{BackendDescriptor, Persistence};
use crate::state::DbPool;

mod api_keys;
mod bootstrap;
mod configuration;
mod dashboard;
mod device_types;
pub mod executor;
mod fleets;
mod roles;
mod shadows;
mod users;

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
        adapter.clone(),
        adapter.clone(),
        adapter.clone(),
        adapter.clone(),
        adapter.clone(),
        adapter.clone(),
        adapter.clone(),
        adapter.clone(),
        adapter,
    )
}
