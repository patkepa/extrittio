CREATE TABLE device_blueprints (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    blueprint_key TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, blueprint_key)
);

CREATE TABLE device_blueprint_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    blueprint_id TEXT NOT NULL,
    document JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, blueprint_id),
    FOREIGN KEY (tenant_id, blueprint_id)
        REFERENCES device_blueprints(tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE device_blueprint_revisions (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    blueprint_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    document JSONB NOT NULL,
    document_hash TEXT NOT NULL CHECK (length(document_hash) = 64),
    compatibility JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, blueprint_id, revision),
    FOREIGN KEY (tenant_id, blueprint_id)
        REFERENCES device_blueprints(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_device_blueprints_tenant_name
    ON device_blueprints (tenant_id, name, id);
CREATE INDEX idx_device_blueprint_revisions_latest
    ON device_blueprint_revisions (tenant_id, blueprint_id, revision DESC);

INSERT INTO role_permissions (role_id, permission)
SELECT roles.id, permissions.permission
FROM roles
CROSS JOIN LATERAL (
    VALUES
        ('device_blueprints.read'),
        ('device_blueprints.manage')
) AS permissions(permission)
WHERE roles.name IN ('owner', 'admin')
ON CONFLICT DO NOTHING;

INSERT INTO role_permissions (role_id, permission)
SELECT roles.id, 'device_blueprints.read'
FROM roles
WHERE roles.name IN ('operator', 'viewer')
ON CONFLICT DO NOTHING;
