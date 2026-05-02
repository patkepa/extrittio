use crate::auth::context::RequestContext;
use crate::error::AppError;

#[derive(Debug, Clone, Copy)]
pub enum Permission {
    ManageApiKeys,
}

pub fn require(ctx: &RequestContext, permission: Permission) -> Result<(), AppError> {
    match permission {
        Permission::ManageApiKeys => require_admin(ctx),
    }
}

fn require_admin(ctx: &RequestContext) -> Result<(), AppError> {
    if ctx.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden("Admin role required".into()))
    }
}
