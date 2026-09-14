pub const DEFAULT_TENANT_ID: &str = "default";

/// Compatibility name for the core-owned tenant identifier. Keeping one
/// concrete type across host authentication, application use cases, and
/// storage ports prevents adapters from translating tenant scope by string.
pub use extrittio_backend_core::{TenantId, TenantIdError};

pub use extrittio_backend_core::{DeviceIdentity, DeviceIdentityError};
