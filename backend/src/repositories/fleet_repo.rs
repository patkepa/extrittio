// Repository functions for fleets

use diesel::SqliteConnection;
use diesel::prelude::*;

use crate::db::models::{Fleet, NewFleet};
use crate::db::schema::{devices, fleets};

type FleetDeviceCounts = Vec<(Option<i32>, i64)>;

pub fn list_fleets(
    conn: &mut SqliteConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Fleet>, FleetDeviceCounts, i64), diesel::result::Error> {
    let total: i64 = fleets::table.count().get_result(conn)?;

    let all_fleets: Vec<Fleet> = fleets::table
        .select(Fleet::as_select())
        .order(fleets::name.asc())
        .limit(limit)
        .offset(offset)
        .load(conn)?;

    let counts: Vec<(Option<i32>, i64)> = devices::table
        .group_by(devices::fleet_id)
        .select((devices::fleet_id, diesel::dsl::count(devices::id)))
        .load(conn)?;

    Ok((all_fleets, counts, total))
}

pub fn insert_fleet(
    conn: &mut SqliteConnection,
    fleet: &NewFleet,
) -> Result<Fleet, diesel::result::Error> {
    diesel::insert_into(fleets::table)
        .values(fleet)
        .execute(conn)?;

    // Query by the exact name just inserted to avoid returning the wrong row
    // under concurrent inserts.
    fleets::table
        .filter(fleets::name.eq(&fleet.name))
        .select(Fleet::as_select())
        .first(conn)
}

pub fn delete_fleet(conn: &mut SqliteConnection, id: i32) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(fleets::table.find(id)).execute(conn)?;
    Ok(rows > 0)
}
