use diesel::Connection;
use diesel::prelude::*;
use crate::db::models::{ApiKey, NewApiKey};
use crate::db::schema::api_keys;

pub fn insert_api_key(
    conn: &mut SqliteConnection,
    new_key: &NewApiKey,
) -> Result<ApiKey, diesel::result::Error> {
    conn.transaction(|conn| {
        diesel::insert_into(api_keys::table)
            .values(new_key)
            .execute(conn)?;

        api_keys::table
            .order(api_keys::id.desc())
            .select(ApiKey::as_select())
            .first(conn)
    })
}

pub fn find_api_key_by_hash(
    conn: &mut SqliteConnection,
    hash: &str,
) -> Result<Option<ApiKey>, diesel::result::Error> {
    api_keys::table
        .filter(api_keys::key_hash.eq(hash))
        .select(ApiKey::as_select())
        .first(conn)
        .optional()
}

pub fn list_api_keys(
    conn: &mut SqliteConnection,
) -> Result<Vec<ApiKey>, diesel::result::Error> {
    api_keys::table
        .order(api_keys::created_at.desc())
        .select(ApiKey::as_select())
        .load(conn)
}

pub fn delete_api_key(
    conn: &mut SqliteConnection,
    key_id: i32,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(api_keys::table.filter(api_keys::id.eq(key_id)))
        .execute(conn)
}

pub fn update_last_used(
    conn: &mut SqliteConnection,
    key_id: i32,
) -> Result<usize, diesel::result::Error> {
    diesel::update(api_keys::table.filter(api_keys::id.eq(key_id)))
        .set(api_keys::last_used_at.eq(diesel::dsl::now))
        .execute(conn)
}
