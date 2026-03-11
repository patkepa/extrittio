// Repository functions for fleets

use diesel::prelude::*;
use diesel::SqliteConnection;

use crate::db::models::{Fleet, NewFleet};
use crate::db::schema::{devices, fleets};

pub fn list_fleets(
    conn: &mut SqliteConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Fleet>, Vec<(Option<i32>, i64)>, i64), diesel::result::Error> {
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

    fleets::table
        .order(fleets::id.desc())
        .select(Fleet::as_select())
        .first(conn)
}

pub fn delete_fleet(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(fleets::table.find(id)).execute(conn)?;
    Ok(rows > 0)
}
