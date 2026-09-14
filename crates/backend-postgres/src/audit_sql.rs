use diesel::PgConnection;
use diesel::prelude::*;

use crate::models::{AuditEvent, NewAuditEvent};
use crate::schema::audit_events;

pub fn insert(conn: &mut PgConnection, event: &NewAuditEvent) -> QueryResult<usize> {
    diesel::insert_into(audit_events::table)
        .values(event)
        .execute(conn)
}

pub fn list(
    conn: &mut PgConnection,
    tenant_id: &str,
    limit: i64,
    offset: i64,
) -> QueryResult<Vec<AuditEvent>> {
    audit_events::table
        .filter(audit_events::tenant_id.eq(tenant_id))
        .order((
            audit_events::occurred_at.desc(),
            diesel::dsl::sql::<diesel::sql_types::Text>("id COLLATE \"C\"").desc(),
        ))
        .limit(limit)
        .offset(offset)
        .select(AuditEvent::as_select())
        .load(conn)
}
