// Repository functions for dashboard

use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::schema::{devices, telemetry};

#[derive(Debug)]
pub struct DashboardCounts {
    pub total_devices: i64,
    pub online_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}

pub fn get_dashboard_counts(
    conn: &mut PgConnection,
    tenant_id: &str,
) -> Result<DashboardCounts, diesel::result::Error> {
    let total_devices = devices::table
        .filter(devices::tenant_id.eq(tenant_id))
        .count()
        .get_result(conn)?;
    let online_devices = devices::table
        .filter(devices::tenant_id.eq(tenant_id))
        .filter(devices::status.eq("online"))
        .count()
        .get_result(conn)?;
    let offline_devices = devices::table
        .filter(devices::tenant_id.eq(tenant_id))
        .filter(devices::status.eq("offline"))
        .count()
        .get_result(conn)?;
    let total_messages = telemetry::table
        .filter(telemetry::tenant_id.eq(tenant_id))
        .count()
        .get_result(conn)?;

    Ok(DashboardCounts {
        total_devices,
        online_devices,
        offline_devices,
        total_messages,
    })
}
