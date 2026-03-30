// Repository functions for dashboard

use diesel::PgConnection;
use diesel::dsl::count_star;
use diesel::prelude::*;

use crate::db::schema::{devices, telemetry};

pub fn get_total_devices(conn: &mut PgConnection) -> Result<i64, diesel::result::Error> {
    devices::table.select(count_star()).first(conn)
}

pub fn get_online_devices(conn: &mut PgConnection) -> Result<i64, diesel::result::Error> {
    devices::table
        .filter(devices::status.eq("online"))
        .select(count_star())
        .first(conn)
}

pub fn get_offline_devices(conn: &mut PgConnection) -> Result<i64, diesel::result::Error> {
    devices::table
        .filter(devices::status.eq("offline"))
        .select(count_star())
        .first(conn)
}

pub fn get_total_messages(conn: &mut PgConnection) -> Result<i64, diesel::result::Error> {
    telemetry::table.select(count_star()).first(conn)
}
