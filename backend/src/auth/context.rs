use super::Claims;
use super::policy::Permission;
use crate::tenancy::{DEFAULT_TENANT_ID, TenantId};
use std::collections::HashSet;

/// Authenticated request metadata used by application services and policy
/// checks. Tenant data is optional until the schema becomes tenant-aware.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub user_id: i32,
    pub username: String,
    pub role: String,
    pub tenant_id: Option<TenantId>,
    pub scopes: Vec<String>,
    pub permissions: HashSet<Permission>,
}

impl RequestContext {
    #[must_use]
    pub fn from_claims(claims: Claims) -> Self {
        let tenant_id = claims
            .tenant_id
            .unwrap_or_else(|| DEFAULT_TENANT_ID.to_string());
        let tenant_id = TenantId::new(tenant_id)
            .unwrap_or_else(|_| TenantId::new(DEFAULT_TENANT_ID).expect("valid tenant id"));

        let permissions = Permission::from_keys(&claims.scopes);
        let permissions =
            if permissions.is_empty() && matches!(claims.role.as_str(), "admin" | "owner") {
                Permission::all().iter().copied().collect()
            } else {
                permissions
            };

        Self {
            user_id: claims.sub,
            username: claims.username,
            role: claims.role,
            tenant_id: Some(tenant_id),
            scopes: claims.scopes,
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
        matches!(self.role.as_str(), "admin" | "owner")
    }

    #[must_use]
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission) || self.is_admin()
    }

    #[must_use]
    pub fn tenant_id_str(&self) -> &str {
        self.tenant_id
            .as_ref()
            .map_or(DEFAULT_TENANT_ID, TenantId::as_str)
    }
}
