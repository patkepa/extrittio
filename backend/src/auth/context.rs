use super::Claims;
use crate::tenancy::TenantId;

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
        Self {
            user_id: claims.sub,
            username: claims.username,
            role: claims.role,
            tenant_id: None,
            scopes: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}
