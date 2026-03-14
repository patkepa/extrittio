use diesel::SqliteConnection;

use crate::error::AppError;
use crate::repositories::dashboard_repo;

pub struct DashboardStats {
    pub total_devices: i64,
    pub active_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}

pub fn get_stats(conn: &mut SqliteConnection) -> Result<DashboardStats, AppError> {
    Ok(DashboardStats {
        total_devices: dashboard_repo::get_total_devices(conn)?,
        active_devices: dashboard_repo::get_online_devices(conn)?,
        offline_devices: dashboard_repo::get_offline_devices(conn)?,
        total_messages: dashboard_repo::get_total_firmware(conn)?,
    })
}
