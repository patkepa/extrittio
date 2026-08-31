//! Compatibility re-exports for the core-owned persistence failure contract.
//!
//! Keeping this module lets legacy host repositories migrate independently
//! without maintaining a second, nominally incompatible error taxonomy.

pub use extrittio_backend_core::{ConstraintName, PersistenceError};
