use diesel::PgConnection;

use crate::db::models::NewAuditEvent;
use crate::error::AppError;
use crate::repositories::audit_repo;

pub fn record(conn: &mut PgConnection, event: NewAuditEvent) -> Result<(), AppError> {
    audit_repo::insert(conn, &event)?;
    Ok(())
}
