//! Pure device-blueprint validation and contract compilation for Extrittio.
//!
//! This crate is the boundary between device-specific declarations and the
//! generic platform. It deliberately contains no database, transport, HTTP,
//! tenant-authorization, async-runtime, or worker dependencies.

mod canonical;
mod compatibility;
mod compiler;
mod error;
mod model;
mod schema;
mod validation;

pub use compatibility::{
    Compatibility, CompatibilityChange, CompatibilityChangeKind, compare_blueprints,
};
pub use compiler::{
    BlueprintCompiler, CompileContext, CompiledCommand, CompiledConfiguration, CompiledContract,
    CompiledContractDocument, CompiledField, CompiledRoute, CompiledRuntime, CompiledSchema,
    CompiledStream, CompiledTransport, ContractHash, ResolvedTransport,
};
pub use error::{ContractError, ValidationIssue};
pub use model::*;
pub use schema::{SchemaProfile, validate_instance};
pub use validation::{ValidatedBlueprint, validate_blueprint};

/// The first supported blueprint document API.
pub const BLUEPRINT_API_VERSION: &str = "extrittio.io/v1alpha1";

/// The only accepted top-level kind for a blueprint document.
pub const BLUEPRINT_KIND: &str = "DeviceBlueprint";

/// Contract-runtime API implemented by this version of the compiler.
pub const CONTRACT_API_VERSION: u32 = 1;
