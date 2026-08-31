//! Temporary PostgreSQL rule-snapshot bridge.
//!
//! Tenant CRUD moved to `extrittio-backend-postgres`; this system-scoped read
//! remains until the rules slice consumes `RuleZoneSnapshotRepository`.

use crate::db::models::Zone;
use crate::db::schema::zones;
use diesel::PgConnection;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::Text;

pub fn list_all_zones(conn: &mut PgConnection) -> QueryResult<Vec<Zone>> {
    zones::table
        .order((
            sql::<Text>(r#"tenant_id COLLATE "C""#).asc(),
            sql::<Text>(r#"name COLLATE "C""#).asc(),
            sql::<Text>(r#"id COLLATE "C""#).asc(),
        ))
        .select(Zone::as_select())
        .load(conn)
}
