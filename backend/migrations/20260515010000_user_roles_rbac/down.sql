DROP INDEX IF EXISTS idx_user_roles_tenant_role;
DROP INDEX IF EXISTS idx_user_roles_tenant_user;
DROP INDEX IF EXISTS idx_role_permissions_permission;
DROP INDEX IF EXISTS idx_roles_tenant_id;

DROP TABLE IF EXISTS user_roles;
DROP TABLE IF EXISTS role_permissions;
DROP TABLE IF EXISTS roles;

ALTER TABLE users
    ALTER COLUMN role SET DEFAULT 'admin';
ALTER TABLE users
    DROP COLUMN IF EXISTS last_login_at;
ALTER TABLE users
    DROP COLUMN IF EXISTS permission_version;
ALTER TABLE users
    DROP COLUMN IF EXISTS is_active;
