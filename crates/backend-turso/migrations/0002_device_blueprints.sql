CREATE TABLE device_blueprints (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  blueprint_key TEXT NOT NULL, name TEXT NOT NULL, description TEXT,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  UNIQUE (tenant_id, id), UNIQUE (tenant_id, blueprint_key)
);
CREATE TABLE device_blueprint_drafts (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, blueprint_id TEXT NOT NULL,
  document TEXT NOT NULL CHECK(json_valid(document)), created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  UNIQUE (tenant_id, blueprint_id),
  FOREIGN KEY (tenant_id, blueprint_id) REFERENCES device_blueprints(tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE device_blueprint_revisions (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, blueprint_id TEXT NOT NULL,
  revision INTEGER NOT NULL CHECK(revision > 0), document TEXT NOT NULL CHECK(json_valid(document)),
  document_hash TEXT NOT NULL CHECK(length(document_hash) = 64),
  compatibility TEXT NOT NULL CHECK(json_valid(compatibility)), created_at INTEGER NOT NULL,
  UNIQUE (tenant_id, id), UNIQUE (tenant_id, blueprint_id, revision),
  FOREIGN KEY (tenant_id, blueprint_id) REFERENCES device_blueprints(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_device_blueprints_tenant_name ON device_blueprints(tenant_id, name, id);
CREATE INDEX idx_device_blueprint_revisions_latest
  ON device_blueprint_revisions(tenant_id, blueprint_id, revision DESC);
INSERT OR IGNORE INTO role_permissions(role_id, permission)
SELECT id, 'device_blueprints.read' FROM roles WHERE name IN ('owner', 'admin', 'operator', 'viewer');
INSERT OR IGNORE INTO role_permissions(role_id, permission)
SELECT id, 'device_blueprints.manage' FROM roles WHERE name IN ('owner', 'admin');
