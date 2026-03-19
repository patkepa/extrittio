// Zone service — business logic for zone management

use diesel::prelude::*;
use diesel::SqliteConnection;
use uuid::Uuid;

use crate::db::models::{NewZone, Zone};
use crate::error::AppError;
use crate::repositories::zone_repo;

pub fn list_zones(conn: &mut SqliteConnection) -> Result<Vec<Zone>, AppError> {
    Ok(zone_repo::list_zones(conn)?)
}

pub fn get_zone(conn: &mut SqliteConnection, zone_id: &str) -> Result<Zone, AppError> {
    zone_repo::get_zone(conn, zone_id).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Zone '{zone_id}' not found"))
        }
        other => AppError::Database(other),
    })
}

pub fn create_zone(
    conn: &mut SqliteConnection,
    name: String,
    description: String,
    geometry_type: String,
    geometry_json: String,
    color: String,
) -> Result<Zone, AppError> {
    let zone_id = Uuid::new_v4().to_string();

    let new_zone = NewZone {
        id: zone_id,
        name,
        description,
        geometry_type,
        geometry_json,
        color,
    };

    Ok(zone_repo::insert_zone(conn, &new_zone)?)
}

pub fn update_zone(
    conn: &mut SqliteConnection,
    zone_id: &str,
    name: Option<String>,
    description: Option<String>,
    geometry_type: Option<String>,
    geometry_json: Option<String>,
    color: Option<String>,
) -> Result<Zone, AppError> {
    // Verify zone exists first
    zone_repo::get_zone(conn, zone_id).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Zone '{zone_id}' not found"))
        }
        other => AppError::Database(other),
    })?;

    Ok(zone_repo::update_zone(
        conn,
        zone_id,
        name,
        description,
        geometry_type,
        geometry_json,
        color,
    )?)
}

pub fn delete_zone(conn: &mut SqliteConnection, zone_id: &str) -> Result<(), AppError> {
    // Verify zone exists first
    zone_repo::get_zone(conn, zone_id).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Zone '{zone_id}' not found"))
        }
        other => AppError::Database(other),
    })?;

    // Check if any rule_conditions reference this zone
    use crate::db::schema::rule_conditions;
    let count: i64 = rule_conditions::table
        .filter(rule_conditions::zone_id.eq(zone_id))
        .count()
        .get_result(conn)?;
    if count > 0 {
        return Err(AppError::Conflict(
            "Cannot delete zone: referenced by rules".into(),
        ));
    }

    let rows = zone_repo::delete_zone(conn, zone_id)?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("Zone '{zone_id}' not found")));
    }

    Ok(())
}
