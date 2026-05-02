use crate::auth::context::RequestContext;
use crate::error::AppError;

#[derive(Debug, Clone, Copy)]
pub enum Permission {
    ManageApiKeys,
    ManageFirmware,
    ManageUsers,
    ReadFirmware,
    ReadUsers,
}

pub fn require(ctx: &RequestContext, permission: Permission) -> Result<(), AppError> {
    match permission {
        Permission::ReadFirmware => require_authenticated(ctx),
        Permission::ManageApiKeys | Permission::ManageFirmware => require_admin(ctx),
        Permission::ManageUsers | Permission::ReadUsers => require_admin(ctx),
    }
}

fn require_authenticated(ctx: &RequestContext) -> Result<(), AppError> {
    if ctx.user_id > 0 {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

fn require_admin(ctx: &RequestContext) -> Result<(), AppError> {
    if ctx.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden("Admin role required".into()))
    }
}
