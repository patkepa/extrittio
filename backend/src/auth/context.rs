use super::Claims;
use crate::tenancy::{DEFAULT_TENANT_ID, TenantId};

/// Authenticated request metadata used by application services and policy
/// checks. Tenant data is optional until the schema becomes tenant-aware.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub user_id: i32,
    pub username: String,
    pub role: String,
    pub tenant_id: Option<TenantId>,
    pub scopes: Vec<String>,
}

impl RequestContext {
    #[must_use]
    pub fn from_claims(claims: Claims) -> Self {
        let tenant_id = claims
            .tenant_id
            .unwrap_or_else(|| DEFAULT_TENANT_ID.to_string());
        let tenant_id = TenantId::new(tenant_id)
            .unwrap_or_else(|_| TenantId::new(DEFAULT_TENANT_ID).expect("valid tenant id"));

        Self {
            user_id: claims.sub,
            username: claims.username,
            role: claims.role,
            tenant_id: Some(tenant_id),
            scopes: claims.scopes,
        }
    }

    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    #[must_use]
    pub fn tenant_id_str(&self) -> &str {
        self.tenant_id
            .as_ref()
            .map_or(DEFAULT_TENANT_ID, TenantId::as_str)
    }
}
