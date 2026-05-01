// Repository functions for dashboard

use diesel::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::BigInt;

#[derive(QueryableByName, Debug)]
pub struct DashboardCounts {
    #[diesel(sql_type = BigInt)]
    pub total_devices: i64,
    #[diesel(sql_type = BigInt)]
    pub online_devices: i64,
    #[diesel(sql_type = BigInt)]
    pub offline_devices: i64,
    #[diesel(sql_type = BigInt)]
    pub total_messages: i64,
}

pub fn get_dashboard_counts(
    conn: &mut PgConnection,
) -> Result<DashboardCounts, diesel::result::Error> {
    diesel::sql_query(
        "SELECT \
            (SELECT COUNT(*) FROM devices) AS total_devices, \
            (SELECT COUNT(*) FROM devices WHERE status = 'online') AS online_devices, \
            (SELECT COUNT(*) FROM devices WHERE status = 'offline') AS offline_devices, \
            (SELECT COUNT(*) FROM telemetry) AS total_messages",
    )
    .get_result(conn)
}
