use diesel::PgConnection;
use std::collections::HashMap;

use crate::db::models::{ApiKey, NewApiKey};
use crate::error::AppError;
use crate::repositories::{api_key_repo, device_type_repo};

/// Insert a new API key.
pub fn create(conn: &mut PgConnection, new_key: &NewApiKey) -> Result<ApiKey, AppError> {
    Ok(api_key_repo::insert_api_key(conn, new_key)?)
}

/// List all API keys with their device type names resolved.
/// Returns tuples of (ApiKey, Option<device_type_name>).
pub fn list_with_type_names(
    conn: &mut PgConnection,
) -> Result<Vec<(ApiKey, Option<String>)>, AppError> {
    let keys = api_key_repo::list_api_keys(conn)?;
    let all_device_types = device_type_repo::list_all_device_types(conn)?;
    let dt_map: HashMap<i32, String> = all_device_types
        .into_iter()
        .map(|dt| (dt.id, dt.name))
        .collect();

    let results = keys
        .into_iter()
        .map(|k| {
            let dt_name = k.device_type_id.and_then(|id| dt_map.get(&id).cloned());
            (k, dt_name)
        })
        .collect();

    Ok(results)
}

/// Delete an API key by ID. Returns error if not found.
pub fn delete(conn: &mut PgConnection, id: i32) -> Result<(), AppError> {
    let deleted = api_key_repo::delete_api_key(conn, id)?;
    if deleted == 0 {
        return Err(AppError::NotFound("API key not found".into()));
    }
    Ok(())
}
