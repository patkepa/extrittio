#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use extrittio_backend_core::{PersistenceError, RuleZoneSnapshotRepository, ZoneRepository};

pub mod roles;
pub mod users;
pub mod zones;

pub use roles::{RoleContractHarness, UserVersionFixture};
pub use users::UserContractHarness;

/// Test-only lifecycle used by shared semantic contract suites.
#[async_trait]
pub trait ContractHarness: Send + Sync {
    async fn reset(&self) -> Result<(), PersistenceError>;
    fn zones(&self) -> Arc<dyn ZoneRepository>;
    fn rule_zone_snapshots(&self) -> Arc<dyn RuleZoneSnapshotRepository>;

    /// Test-only fixture hook used to prove the in-use delete outcome.
    async fn reference_zone(&self, tenant_id: &str, zone_id: &str) -> Result<(), PersistenceError>;
}
