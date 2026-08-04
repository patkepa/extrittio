use diesel::Connection;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{NewRole, NewRolePermission, NewUserRole, Role, User, UserRole};
use crate::db::schema::{role_permissions, roles, user_roles, users};

pub fn list_roles(conn: &mut PgConnection, tenant_id: &str) -> QueryResult<Vec<Role>> {
    roles::table
        .filter(roles::tenant_id.eq(tenant_id))
        .select(Role::as_select())
        .order((roles::is_system.desc(), roles::name.asc()))
        .load(conn)
}

pub fn find_role(conn: &mut PgConnection, tenant_id: &str, id: i32) -> QueryResult<Role> {
    roles::table
        .filter(roles::tenant_id.eq(tenant_id))
        .filter(roles::id.eq(id))
        .select(Role::as_select())
        .first(conn)
}

pub fn find_role_by_name(
    conn: &mut PgConnection,
    tenant_id: &str,
    name: &str,
) -> QueryResult<Role> {
    roles::table
        .filter(roles::tenant_id.eq(tenant_id))
        .filter(roles::name.eq(name))
        .select(Role::as_select())
        .first(conn)
}

pub fn find_roles_by_ids(
    conn: &mut PgConnection,
    tenant_id: &str,
    ids: &[i32],
) -> QueryResult<Vec<Role>> {
    roles::table
        .filter(roles::tenant_id.eq(tenant_id))
        .filter(roles::id.eq_any(ids))
        .select(Role::as_select())
        .order(roles::name.asc())
        .load(conn)
}

pub fn insert_role(conn: &mut PgConnection, new_role: &NewRole) -> QueryResult<Role> {
    diesel::insert_into(roles::table)
        .values(new_role)
        .returning(Role::as_returning())
        .get_result(conn)
}

pub fn update_role(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: i32,
    name: &str,
    description: Option<&str>,
) -> QueryResult<Role> {
    diesel::update(
        roles::table
            .filter(roles::tenant_id.eq(tenant_id))
            .filter(roles::id.eq(id)),
    )
    .set((
        roles::name.eq(name),
        roles::description.eq(description),
        roles::updated_at.eq(diesel::dsl::now),
    ))
    .returning(Role::as_returning())
    .get_result(conn)
}

pub fn delete_role(conn: &mut PgConnection, tenant_id: &str, id: i32) -> QueryResult<bool> {
    let rows = diesel::delete(
        roles::table
            .filter(roles::tenant_id.eq(tenant_id))
            .filter(roles::id.eq(id)),
    )
    .execute(conn)?;
    Ok(rows > 0)
}

pub fn permissions_for_role(conn: &mut PgConnection, role_id: i32) -> QueryResult<Vec<String>> {
    role_permissions::table
        .filter(role_permissions::role_id.eq(role_id))
        .select(role_permissions::permission)
        .order(role_permissions::permission.asc())
        .load(conn)
}

pub fn permissions_for_user(
    conn: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
) -> QueryResult<Vec<String>> {
    let role_ids: Vec<i32> = user_roles::table
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::user_id.eq(user_id))
        .select(user_roles::role_id)
        .load(conn)?;

    if role_ids.is_empty() {
        return Ok(Vec::new());
    }

    role_permissions::table
        .filter(role_permissions::role_id.eq_any(role_ids))
        .select(role_permissions::permission)
        .distinct()
        .order(role_permissions::permission.asc())
        .load(conn)
}

pub fn roles_for_user(
    conn: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
) -> QueryResult<Vec<Role>> {
    user_roles::table
        .inner_join(roles::table)
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::user_id.eq(user_id))
        .select(Role::as_select())
        .order(roles::name.asc())
        .load(conn)
}

pub fn replace_role_permissions(
    conn: &mut PgConnection,
    role_id: i32,
    permissions: &[String],
) -> QueryResult<()> {
    conn.transaction(|conn| {
        diesel::delete(role_permissions::table.filter(role_permissions::role_id.eq(role_id)))
            .execute(conn)?;

        if !permissions.is_empty() {
            let rows: Vec<NewRolePermission> = permissions
                .iter()
                .map(|permission| NewRolePermission {
                    role_id,
                    permission: permission.clone(),
                })
                .collect();

            diesel::insert_into(role_permissions::table)
                .values(&rows)
                .execute(conn)?;
        }

        Ok(())
    })
}

pub fn set_user_roles(
    conn: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
    role_ids: &[i32],
) -> QueryResult<Vec<Role>> {
    conn.transaction(|conn| {
        let roles_for_tenant = find_roles_by_ids(conn, tenant_id, role_ids)?;
        let unique_count = role_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len();

        if roles_for_tenant.len() != unique_count {
            return Err(diesel::result::Error::NotFound);
        }

        diesel::delete(
            user_roles::table
                .filter(user_roles::tenant_id.eq(tenant_id))
                .filter(user_roles::user_id.eq(user_id)),
        )
        .execute(conn)?;

        if !roles_for_tenant.is_empty() {
            let rows: Vec<NewUserRole> = roles_for_tenant
                .iter()
                .map(|role| NewUserRole {
                    user_id,
                    role_id: role.id,
                    tenant_id: tenant_id.to_string(),
                })
                .collect();

            diesel::insert_into(user_roles::table)
                .values(&rows)
                .execute(conn)?;

            let primary_role = roles_for_tenant
                .iter()
                .find(|role| role.name == "owner")
                .or_else(|| roles_for_tenant.iter().find(|role| role.name == "admin"))
                .unwrap_or(&roles_for_tenant[0]);

            let user = users::table
                .filter(users::tenant_id.eq(tenant_id))
                .filter(users::id.eq(user_id))
                .select(User::as_select())
                .first::<User>(conn)?;

            diesel::update(
                users::table
                    .filter(users::tenant_id.eq(tenant_id))
                    .filter(users::id.eq(user_id)),
            )
            .set((
                users::role.eq(primary_role.name.clone()),
                users::permission_version.eq(user.permission_version + 1),
            ))
            .execute(conn)?;
        }

        Ok(roles_for_tenant)
    })
}

pub fn user_has_role_name(
    conn: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
    role_name: &str,
) -> QueryResult<bool> {
    let count: i64 = user_roles::table
        .inner_join(roles::table)
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::user_id.eq(user_id))
        .filter(roles::name.eq(role_name))
        .count()
        .get_result(conn)?;
    Ok(count > 0)
}

pub fn count_users_with_role_name(
    conn: &mut PgConnection,
    tenant_id: &str,
    role_name: &str,
) -> QueryResult<i64> {
    user_roles::table
        .inner_join(roles::table)
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(roles::name.eq(role_name))
        .select(user_roles::user_id)
        .distinct()
        .count()
        .get_result(conn)
}

pub fn count_users_for_role(
    conn: &mut PgConnection,
    tenant_id: &str,
    role_id: i32,
) -> QueryResult<i64> {
    user_roles::table
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::role_id.eq(role_id))
        .count()
        .get_result(conn)
}

pub fn bump_permission_versions_for_role(
    conn: &mut PgConnection,
    tenant_id: &str,
    role_id: i32,
) -> QueryResult<usize> {
    let user_ids = user_roles::table
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::role_id.eq(role_id))
        .select(user_roles::user_id);

    diesel::update(
        users::table
            .filter(users::tenant_id.eq(tenant_id))
            .filter(users::id.eq_any(user_ids)),
    )
    .set(users::permission_version.eq(users::permission_version + 1))
    .execute(conn)
}

#[allow(dead_code)]
pub fn list_user_roles(
    conn: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
) -> QueryResult<Vec<UserRole>> {
    user_roles::table
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::user_id.eq(user_id))
        .select(UserRole::as_select())
        .load(conn)
}
