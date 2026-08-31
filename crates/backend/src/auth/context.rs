use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use super::Claims;
use super::policy::Permission;
use crate::tenancy::{DEFAULT_TENANT_ID, TenantId, TenantIdError};

static LEGACY_MISSING_TENANT_CLAIM_COUNT: AtomicU64 = AtomicU64::new(0);

/// A signed user JWT after its tenant claim has crossed the compatibility
/// boundary. Keeping this type crate-private prevents unvalidated claims from
/// being mistaken for an authenticated request context.
#[derive(Debug, Clone)]
pub(crate) struct MappedUserClaims {
    claims: Claims,
    tenant_id: TenantId,
}

impl MappedUserClaims {
    #[must_use]
    pub(crate) fn claims(&self) -> &Claims {
        &self.claims
    }

    #[must_use]
    pub(crate) fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub(crate) fn into_parts(self) -> (Claims, TenantId) {
        (self.claims, self.tenant_id)
    }
}

/// Error raised while mapping a cryptographically validated user JWT into a
/// tenant-scoped identity.
#[derive(Debug, thiserror::Error)]
pub enum UserClaimsContextError {
    #[error("invalid tenant claim: {0}")]
    InvalidTenant(#[source] TenantIdError),
}

/// The one compatibility boundary for signed user JWT tenant claims.
///
/// Tokens issued before tenant claims were introduced remain valid and map to
/// the default tenant. A claim that is present but invalid always fails closed.
pub(crate) fn map_validated_user_claims(
    claims: Claims,
) -> Result<MappedUserClaims, UserClaimsContextError> {
    let tenant_id = match claims.tenant_id.as_deref() {
        Some(value) => {
            TenantId::new(value.to_string()).map_err(UserClaimsContextError::InvalidTenant)?
        }
        None => {
            let compatibility_hit = LEGACY_MISSING_TENANT_CLAIM_COUNT
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1);
            tracing::warn!(
                user_id = claims.sub,
                compatibility_hit,
                "security.legacy_jwt_missing_tenant_claim"
            );
            TenantId::new(DEFAULT_TENANT_ID).expect("default tenant ID must be valid")
        }
    };

    Ok(MappedUserClaims { claims, tenant_id })
}

/// Process-local compatibility counter for observability and removal gating.
#[must_use]
pub fn legacy_missing_tenant_claim_count() -> u64 {
    LEGACY_MISSING_TENANT_CLAIM_COUNT.load(Ordering::Relaxed)
}

/// Authenticated request metadata used by application services and policy
/// checks. A request context can only exist with a validated tenant identity.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub user_id: i32,
    pub username: String,
    pub role: String,
    tenant_id: TenantId,
    pub scopes: Vec<String>,
    pub permissions: HashSet<Permission>,
}

impl RequestContext {
    /// Build a context directly from signed claims.
    ///
    /// Runtime authentication replaces claim permissions with current
    /// repository permissions; this constructor remains useful for focused
    /// policy and handler tests.
    pub fn from_claims(claims: Claims) -> Result<Self, UserClaimsContextError> {
        let mapped = map_validated_user_claims(claims)?;
        let (claims, tenant_id) = mapped.into_parts();
        let permissions = Permission::from_keys(&claims.scopes);
        let permissions =
            if permissions.is_empty() && matches!(claims.role.as_str(), "admin" | "owner") {
                Permission::all().iter().copied().collect()
            } else {
                permissions
            };

        Ok(Self {
            user_id: claims.sub,
            username: claims.username,
            role: claims.role,
            tenant_id,
            scopes: claims.scopes,
            permissions,
        })
    }

    pub(crate) fn authenticated(
        user_id: i32,
        username: String,
        role: String,
        tenant_id: TenantId,
        scopes: Vec<String>,
        permissions: HashSet<Permission>,
    ) -> Self {
        Self {
            user_id,
            username,
            role,
            tenant_id,
            scopes,
            permissions,
        }
    }

    #[must_use]
    pub fn with_permissions(mut self, permissions: HashSet<Permission>) -> Self {
        self.permissions = permissions;
        self.scopes = self
            .permissions
            .iter()
            .map(|permission| permission.key().to_string())
            .collect();
        self
    }

    #[must_use]
    pub fn is_admin(&self) -> bool {
        Permission::all()
            .iter()
            .all(|permission| self.permissions.contains(permission))
    }

    #[must_use]
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions
            .iter()
            .any(|held_permission| super::policy::satisfies(*held_permission, permission))
    }

    #[must_use]
    pub fn tenant_id_str(&self) -> &str {
        self.tenant_id.as_str()
    }

    /// Tenant identity required by tenant-scoped persistence ports.
    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    /// Convert the host authentication context into the transport-independent
    /// identity accepted by extracted application use cases.
    #[must_use]
    pub fn tenant_context(&self) -> extrittio_backend_core::TenantContext {
        let actor = extrittio_backend_core::Actor::User {
            id: self.user_id,
            username: self.username.clone(),
            role: self.role.clone(),
        };
        let permissions = extrittio_backend_core::PermissionSet::from_keys(
            self.permissions.iter().map(|permission| permission.key()),
        );

        extrittio_backend_core::TenantContext::new(self.tenant_id.clone(), actor, permissions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(tenant_id: Option<&str>) -> Claims {
        Claims {
            sub: 7,
            username: "legacy-user".to_string(),
            role: "viewer".to_string(),
            tenant_id: tenant_id.map(str::to_string),
            scopes: vec!["devices:read".to_string()],
            permission_version: 1,
            exp: usize::MAX,
        }
    }

    #[test]
    fn valid_tenant_claim_maps_to_the_exact_tenant() {
        let mapped = map_validated_user_claims(claims(Some("tenant-a"))).unwrap();

        assert_eq!(mapped.tenant_id().as_str(), "tenant-a");
    }

    #[test]
    fn absent_legacy_tenant_claim_uses_default_and_increments_counter() {
        let before = legacy_missing_tenant_claim_count();

        let mapped = map_validated_user_claims(claims(None)).unwrap();

        assert_eq!(mapped.tenant_id().as_str(), "default");
        assert!(legacy_missing_tenant_claim_count() > before);
    }

    #[test]
    fn present_empty_or_whitespace_tenant_claim_fails_closed() {
        for tenant_id in ["", " ", "\t\n"] {
            assert!(matches!(
                map_validated_user_claims(claims(Some(tenant_id))),
                Err(UserClaimsContextError::InvalidTenant(_))
            ));
        }
    }

    #[test]
    fn request_context_always_contains_a_tenant() {
        let context = RequestContext::from_claims(claims(Some("tenant-a"))).unwrap();

        assert_eq!(context.tenant_id_str(), "tenant-a");
    }

    #[test]
    fn core_context_conversion_maps_actor_tenant_and_permission_keys() {
        let mut claims = claims(Some("tenant-a"));
        claims.scopes = vec!["zones.manage".to_string()];
        let context = RequestContext::from_claims(claims).unwrap();

        let core = context.tenant_context();

        assert_eq!(core.tenant_id().as_str(), "tenant-a");
        assert_eq!(
            core.actor(),
            &extrittio_backend_core::Actor::User {
                id: 7,
                username: "legacy-user".to_string(),
                role: "viewer".to_string(),
            }
        );
        assert!(
            core.permissions()
                .contains(extrittio_backend_core::Permission::ManageZones)
        );
        assert!(
            core.permissions()
                .contains(extrittio_backend_core::Permission::ReadZones)
        );
    }
}
