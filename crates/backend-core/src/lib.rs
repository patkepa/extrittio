#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

pub mod application;
pub mod context;
pub mod error;
pub mod ports;
pub mod zones;

pub use application::{Application, CreateZone, ZoneApplication, ZoneUpdate};
pub use context::{Actor, Permission, PermissionSet, TenantContext, TenantId, TenantIdError};
pub use error::{ApplicationError, ConstraintName, PersistenceError};
pub use zones::{
    DeleteZoneOutcome, NewZone, RuleZoneSnapshotRepository, Zone, ZonePatch, ZoneRepository,
};
