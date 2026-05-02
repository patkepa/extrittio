// Repository functions for users

use diesel::Connection;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{NewUser, User};
use crate::db::schema::users;

pub fn list_users(
    conn: &mut PgConnection,
    tenant_id: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<User>, i64), diesel::result::Error> {
    let total: i64 = users::table
        .filter(users::tenant_id.eq(tenant_id))
        .count()
        .get_result(conn)?;

    let results = users::table
        .filter(users::tenant_id.eq(tenant_id))
        .select(User::as_select())
        .order(users::username.asc())
        .limit(limit)
        .offset(offset)
        .load(conn)?;

    Ok((results, total))
}

pub fn find_user_by_username(
    conn: &mut PgConnection,
    tenant_id: &str,
    username: &str,
) -> Result<User, diesel::result::Error> {
    users::table
        .filter(users::tenant_id.eq(tenant_id))
        .filter(users::username.eq(username))
        .select(User::as_select())
        .first(conn)
}

pub fn insert_user(conn: &mut PgConnection, user: &NewUser) -> Result<User, diesel::result::Error> {
    conn.transaction(|conn| {
        diesel::insert_into(users::table)
            .values(user)
            .execute(conn)?;

        users::table
            .filter(users::username.eq(&user.username))
            .select(User::as_select())
            .first(conn)
    })
}

pub fn delete_user(conn: &mut PgConnection, id: i32) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(users::table.find(id)).execute(conn)?;
    Ok(rows > 0)
}

pub fn delete_user_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: i32,
) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(
        users::table
            .filter(users::tenant_id.eq(tenant_id))
            .filter(users::id.eq(id)),
    )
    .execute(conn)?;
    Ok(rows > 0)
}

pub fn update_password(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: i32,
    hash: &str,
) -> Result<(), diesel::result::Error> {
    diesel::update(
        users::table
            .filter(users::tenant_id.eq(tenant_id))
            .filter(users::id.eq(id)),
    )
    .set(users::password_hash.eq(hash))
    .execute(conn)?;
    Ok(())
}

pub fn find_user_by_id(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: i32,
) -> Result<User, diesel::result::Error> {
    users::table
        .filter(users::tenant_id.eq(tenant_id))
        .filter(users::id.eq(id))
        .select(User::as_select())
        .first(conn)
}
