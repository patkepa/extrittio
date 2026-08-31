-- Tenant-scoped RBAC. Existing role strings remain as a compatibility field,
-- but authorization moves to role_permissions through user_roles.

CREATE TABLE roles (
    id SERIAL PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT roles_tenant_id_name_key UNIQUE (tenant_id, name)
);

CREATE TABLE role_permissions (
    role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission TEXT NOT NULL,
    PRIMARY KEY (role_id, permission)
);

CREATE TABLE user_roles (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, role_id)
);

ALTER TABLE users
    ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE users
    ADD COLUMN permission_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE users
    ADD COLUMN last_login_at TIMESTAMPTZ;
ALTER TABLE users
    ALTER COLUMN role SET DEFAULT 'viewer';

CREATE INDEX idx_roles_tenant_id ON roles(tenant_id);
CREATE INDEX idx_role_permissions_permission ON role_permissions(permission);
CREATE INDEX idx_user_roles_tenant_user ON user_roles(tenant_id, user_id);
CREATE INDEX idx_user_roles_tenant_role ON user_roles(tenant_id, role_id);

INSERT INTO roles (tenant_id, name, description, is_system)
SELECT id, 'owner', 'Full tenant owner with all permissions and lockout protection.', TRUE
FROM organizations
ON CONFLICT (tenant_id, name) DO NOTHING;

INSERT INTO roles (tenant_id, name, description, is_system)
SELECT id, 'admin', 'Administrative access to tenant resources and security settings.', TRUE
FROM organizations
ON CONFLICT (tenant_id, name) DO NOTHING;

INSERT INTO roles (tenant_id, name, description, is_system)
SELECT id, 'operator', 'Operational access for device management workflows without security administration.', TRUE
FROM organizations
ON CONFLICT (tenant_id, name) DO NOTHING;

INSERT INTO roles (tenant_id, name, description, is_system)
SELECT id, 'viewer', 'Read-only operational visibility.', TRUE
FROM organizations
ON CONFLICT (tenant_id, name) DO NOTHING;

WITH grants(role_name, permission) AS (
    VALUES
        ('owner', 'api_keys.manage'),
        ('owner', 'alerts.manage'),
        ('owner', 'alerts.read'),
        ('owner', 'commands.read'),
        ('owner', 'commands.send'),
        ('owner', 'device_types.manage'),
        ('owner', 'device_types.read'),
        ('owner', 'devices.manage'),
        ('owner', 'devices.read'),
        ('owner', 'firmware.deploy'),
        ('owner', 'firmware.manage'),
        ('owner', 'firmware.read'),
        ('owner', 'fleets.manage'),
        ('owner', 'fleets.read'),
        ('owner', 'logs.read'),
        ('owner', 'roles.manage'),
        ('owner', 'roles.read'),
        ('owner', 'rules.manage'),
        ('owner', 'rules.read'),
        ('owner', 'server_metrics.read'),
        ('owner', 'shadows.manage'),
        ('owner', 'shadows.read'),
        ('owner', 'telemetry.read'),
        ('owner', 'users.manage'),
        ('owner', 'users.read'),
        ('owner', 'zones.manage'),
        ('owner', 'zones.read'),
        ('admin', 'api_keys.manage'),
        ('admin', 'alerts.manage'),
        ('admin', 'alerts.read'),
        ('admin', 'commands.read'),
        ('admin', 'commands.send'),
        ('admin', 'device_types.manage'),
        ('admin', 'device_types.read'),
        ('admin', 'devices.manage'),
        ('admin', 'devices.read'),
        ('admin', 'firmware.deploy'),
        ('admin', 'firmware.manage'),
        ('admin', 'firmware.read'),
        ('admin', 'fleets.manage'),
        ('admin', 'fleets.read'),
        ('admin', 'logs.read'),
        ('admin', 'roles.manage'),
        ('admin', 'roles.read'),
        ('admin', 'rules.manage'),
        ('admin', 'rules.read'),
        ('admin', 'server_metrics.read'),
        ('admin', 'shadows.manage'),
        ('admin', 'shadows.read'),
        ('admin', 'telemetry.read'),
        ('admin', 'users.manage'),
        ('admin', 'users.read'),
        ('admin', 'zones.manage'),
        ('admin', 'zones.read'),
        ('operator', 'alerts.manage'),
        ('operator', 'alerts.read'),
        ('operator', 'commands.read'),
        ('operator', 'commands.send'),
        ('operator', 'device_types.read'),
        ('operator', 'devices.read'),
        ('operator', 'firmware.read'),
        ('operator', 'fleets.read'),
        ('operator', 'logs.read'),
        ('operator', 'rules.read'),
        ('operator', 'shadows.manage'),
        ('operator', 'shadows.read'),
        ('operator', 'telemetry.read'),
        ('operator', 'zones.read'),
        ('viewer', 'alerts.read'),
        ('viewer', 'commands.read'),
        ('viewer', 'device_types.read'),
        ('viewer', 'devices.read'),
        ('viewer', 'firmware.read'),
        ('viewer', 'fleets.read'),
        ('viewer', 'logs.read'),
        ('viewer', 'rules.read'),
        ('viewer', 'shadows.read'),
        ('viewer', 'telemetry.read'),
        ('viewer', 'zones.read')
)
INSERT INTO role_permissions (role_id, permission)
SELECT roles.id, grants.permission
FROM roles
JOIN grants ON grants.role_name = roles.name
ON CONFLICT (role_id, permission) DO NOTHING;

INSERT INTO user_roles (tenant_id, user_id, role_id)
SELECT
    users.tenant_id,
    users.id,
    roles.id
FROM users
JOIN roles
    ON roles.tenant_id = users.tenant_id
    AND roles.name = CASE
        WHEN users.role = 'viewer' THEN 'viewer'
        WHEN users.role = 'operator' THEN 'operator'
        ELSE 'owner'
    END
ON CONFLICT (user_id, role_id) DO NOTHING;
