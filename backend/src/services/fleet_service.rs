use diesel::SqliteConnection;
use std::collections::HashMap;

use crate::db::models::{Fleet, NewFleet};
use crate::error::AppError;
use crate::repositories::fleet_repo;

pub struct FleetWithCount {
    pub fleet: Fleet,
    pub device_count: i64,
}

pub fn list(
    conn: &mut SqliteConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FleetWithCount>, i64), AppError> {
    let (fleets, counts, total) = fleet_repo::list_fleets(conn, limit, offset)?;

    let count_map: HashMap<i32, i64> = counts
        .into_iter()
        .filter_map(|(fleet_id, count)| fleet_id.map(|fid| (fid, count)))
        .collect();

    let enriched = fleets
        .into_iter()
        .map(|f| {
            let device_count = count_map.get(&f.id).copied().unwrap_or(0);
            FleetWithCount {
                fleet: f,
                device_count,
            }
        })
        .collect();

    Ok((enriched, total))
}

pub fn create(conn: &mut SqliteConnection, name: &str) -> Result<Fleet, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("Fleet name must not be empty".into()));
    }
    Ok(fleet_repo::insert_fleet(
        conn,
        &NewFleet {
            name: name.to_string(),
        },
    )?)
}

pub fn rename(conn: &mut SqliteConnection, id: i32, new_name: &str) -> Result<Fleet, AppError> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("Fleet name must not be empty".into()));
    }
    fleet_repo::update_fleet_name(conn, id, trimmed)?
        .ok_or_else(|| AppError::NotFound(format!("Fleet {id} not found")))
}

pub fn delete(conn: &mut SqliteConnection, id: i32) -> Result<(), AppError> {
    let deleted = fleet_repo::delete_fleet(conn, id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Fleet {id} not found")));
    }
    Ok(())
}
