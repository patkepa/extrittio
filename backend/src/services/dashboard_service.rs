use diesel::PgConnection;

use crate::error::AppError;
use crate::repositories::dashboard_repo;

pub struct DashboardStats {
    pub total_devices: i64,
    pub active_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}

pub fn get_stats(conn: &mut PgConnection) -> Result<DashboardStats, AppError> {
    let counts = dashboard_repo::get_dashboard_counts(conn)?;
    Ok(DashboardStats {
        total_devices: counts.total_devices,
        active_devices: counts.online_devices,
        offline_devices: counts.offline_devices,
        total_messages: counts.total_messages,
    })
}
