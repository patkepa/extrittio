// Repository functions for zones

use chrono::Utc;
use diesel::PgConnection;
use diesel::prelude::*;
use serde_json::Value as JsonValue;

use crate::db::models::{NewZone, UpdateZone, Zone};
use crate::db::schema::zones;

pub fn list_zones(conn: &mut PgConnection, tenant_id: &str) -> QueryResult<Vec<Zone>> {
    zones::table
        .filter(zones::tenant_id.eq(tenant_id))
        .order(zones::created_at.desc())
        .select(Zone::as_select())
        .load(conn)
}

pub fn list_all_zones(conn: &mut PgConnection) -> QueryResult<Vec<Zone>> {
    zones::table
        .order(zones::created_at.desc())
        .select(Zone::as_select())
        .load(conn)
}

pub fn get_zone(conn: &mut PgConnection, tenant_id: &str, zone_id: &str) -> QueryResult<Zone> {
    zones::table
        .filter(zones::tenant_id.eq(tenant_id))
        .filter(zones::id.eq(zone_id))
        .select(Zone::as_select())
        .first(conn)
}

pub fn insert_zone(
    conn: &mut PgConnection,
    tenant_id: &str,
    new_zone: &NewZone,
) -> QueryResult<Zone> {
    diesel::insert_into(zones::table)
        .values(new_zone)
        .execute(conn)?;
    zones::table
        .filter(zones::tenant_id.eq(tenant_id))
        .filter(zones::id.eq(&new_zone.id))
        .select(Zone::as_select())
        .first(conn)
}

pub fn update_zone(
    conn: &mut PgConnection,
    tenant_id: &str,
    zone_id: &str,
    name: Option<String>,
    description: Option<String>,
    geometry_type: Option<String>,
    geometry_json: Option<JsonValue>,
    color: Option<String>,
) -> QueryResult<Zone> {
    let now = Utc::now().naive_utc();
    let changeset = UpdateZone {
        name,
        description,
        geometry_type,
        geometry_json,
        color,
        updated_at: Some(now),
    };

    diesel::update(
        zones::table
            .filter(zones::tenant_id.eq(tenant_id))
            .filter(zones::id.eq(zone_id)),
    )
    .set(&changeset)
    .execute(conn)?;

    zones::table
        .filter(zones::tenant_id.eq(tenant_id))
        .filter(zones::id.eq(zone_id))
        .select(Zone::as_select())
        .first(conn)
}

pub fn delete_zone(conn: &mut PgConnection, tenant_id: &str, zone_id: &str) -> QueryResult<usize> {
    diesel::delete(
        zones::table
            .filter(zones::tenant_id.eq(tenant_id))
            .filter(zones::id.eq(zone_id)),
    )
    .execute(conn)
}
