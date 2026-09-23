-- Fresh blueprint-only schema. No legacy upgrade or data conversion path.
CREATE TABLE organizations (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
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

CREATE TABLE server_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE fleets (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL,
  created_at INTEGER NOT NULL, UNIQUE (tenant_id, name), UNIQUE (tenant_id, id)
);
CREATE TABLE devices (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL,
  fleet_id INTEGER, status TEXT NOT NULL, firmware TEXT NOT NULL,
  last_seen INTEGER, uptime_seconds INTEGER NOT NULL DEFAULT 0, declared_connections TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(declared_connections)),
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE (tenant_id, id),
  FOREIGN KEY (tenant_id, fleet_id) REFERENCES fleets(tenant_id, id)
);
CREATE TABLE users (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), username TEXT NOT NULL,
  password_hash TEXT NOT NULL, role TEXT NOT NULL, is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)),
  auth_epoch TEXT NOT NULL UNIQUE CHECK(length(auth_epoch) > 0),
  permission_version INTEGER NOT NULL DEFAULT 1, last_login_at INTEGER, created_at INTEGER NOT NULL,
  UNIQUE (tenant_id, username), UNIQUE (tenant_id, id)
);
CREATE TABLE roles (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL,
  description TEXT, is_system INTEGER NOT NULL DEFAULT 0 CHECK(is_system IN (0,1)),
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE (tenant_id, name), UNIQUE (tenant_id, id)
);
CREATE TABLE role_permissions (
  role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE, permission TEXT NOT NULL,
  PRIMARY KEY (role_id, permission)
);
CREATE TABLE user_roles (
  user_id INTEGER NOT NULL, role_id INTEGER NOT NULL, tenant_id TEXT NOT NULL,
  created_at INTEGER NOT NULL, PRIMARY KEY (user_id, role_id),
  FOREIGN KEY (tenant_id, user_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, role_id) REFERENCES roles(tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE api_keys (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL,
  key_hash TEXT NOT NULL, key_prefix TEXT NOT NULL, blueprint_id TEXT, created_at INTEGER NOT NULL,
  last_used_at INTEGER, UNIQUE (tenant_id, name), UNIQUE (key_hash),
  FOREIGN KEY (tenant_id, blueprint_id) REFERENCES device_blueprints(tenant_id, id)
);
CREATE TABLE ca_certificates (
  id INTEGER PRIMARY KEY, private_key_pem TEXT NOT NULL, certificate_pem TEXT NOT NULL, created_at INTEGER NOT NULL
);
CREATE TABLE device_certificates (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, private_key_pem TEXT NOT NULL,
  certificate_pem TEXT NOT NULL, fingerprint TEXT NOT NULL, expires_at INTEGER NOT NULL, created_at INTEGER NOT NULL,
  UNIQUE (tenant_id, fingerprint), FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE device_configs (
  tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, config TEXT NOT NULL CHECK(json_valid(config)),
  updated_at INTEGER NOT NULL, PRIMARY KEY (tenant_id, device_id),
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE device_shadows (
  tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, desired TEXT NOT NULL CHECK(json_valid(desired)),
  reported TEXT NOT NULL CHECK(json_valid(reported)), delta TEXT NOT NULL CHECK(json_valid(delta)),
  version INTEGER NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY (tenant_id, device_id),
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE device_logs (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, level TEXT NOT NULL,
  message TEXT NOT NULL, created_at INTEGER NOT NULL,
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX device_logs_device_time ON device_logs(tenant_id, device_id, created_at DESC, id DESC);
CREATE TABLE command_history (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, command TEXT NOT NULL, params TEXT NOT NULL,
  status TEXT NOT NULL, response_payload TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE firmware_updates (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, blueprint_revision_id TEXT NOT NULL, version TEXT NOT NULL,
  url TEXT NOT NULL, description TEXT, sha256 TEXT, commit_sha TEXT, branch TEXT, ci_run_url TEXT,
  build_timestamp INTEGER, changelog TEXT, source TEXT NOT NULL, created_at INTEGER NOT NULL,
  compatibility TEXT NOT NULL CHECK(json_valid(compatibility)), update_strategy TEXT,
  UNIQUE (tenant_id, id), UNIQUE (tenant_id, blueprint_revision_id, version),
  FOREIGN KEY (tenant_id, blueprint_revision_id) REFERENCES device_blueprint_revisions(tenant_id, id)
);
CREATE TABLE firmware_blobs (
  firmware_update_id INTEGER PRIMARY KEY,
  tenant_id TEXT NOT NULL, size INTEGER NOT NULL CHECK(size >= 0), filename TEXT NOT NULL, storage_key TEXT NOT NULL,
  storage_backend TEXT NOT NULL CHECK(storage_backend <> 'database'), created_at INTEGER NOT NULL,
  FOREIGN KEY (tenant_id, firmware_update_id) REFERENCES firmware_updates(tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE ota_deployments (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, firmware_update_id INTEGER NOT NULL,
  status TEXT NOT NULL, error_message TEXT, initiated_at INTEGER NOT NULL, completed_at INTEGER,
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id),
  FOREIGN KEY (tenant_id, firmware_update_id) REFERENCES firmware_updates(tenant_id, id)
);
CREATE TABLE zones (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL, description TEXT NOT NULL,
  geometry_type TEXT NOT NULL, geometry_json TEXT NOT NULL CHECK(json_valid(geometry_json)), color TEXT NOT NULL,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE (tenant_id, name), UNIQUE (tenant_id, id)
);
CREATE TABLE rules (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL, description TEXT,
  enabled INTEGER NOT NULL CHECK(enabled IN (0,1)), trigger_type TEXT NOT NULL, target_type TEXT NOT NULL CHECK(target_type IN ('global','device','fleet','blueprint')),
  target_id TEXT, cooldown_seconds INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  UNIQUE (tenant_id, name), UNIQUE (tenant_id, id)
);
CREATE TABLE rule_conditions (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, rule_id TEXT NOT NULL, field TEXT NOT NULL,
  blueprint_id TEXT, blueprint_revision_id TEXT, operator TEXT NOT NULL,
  value TEXT NOT NULL, condition_group INTEGER NOT NULL, zone_id TEXT,
  FOREIGN KEY (tenant_id, rule_id) REFERENCES rules(tenant_id, id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, zone_id) REFERENCES zones(tenant_id, id)
);
CREATE TABLE rule_actions (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, rule_id TEXT NOT NULL, action_type TEXT NOT NULL,
  config TEXT NOT NULL CHECK(json_valid(config)),
  FOREIGN KEY (tenant_id, rule_id) REFERENCES rules(tenant_id, id) ON DELETE CASCADE
);
CREATE TABLE rule_cooldowns (
  tenant_id TEXT NOT NULL, rule_id TEXT NOT NULL, device_id TEXT NOT NULL, last_fired_at INTEGER NOT NULL,
  PRIMARY KEY (tenant_id, rule_id, device_id)
);
CREATE TABLE alerts (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, rule_id TEXT, device_id TEXT NOT NULL, severity TEXT NOT NULL,
  status TEXT NOT NULL, message TEXT NOT NULL, triggered_value TEXT, resolved_at INTEGER, acknowledged_at INTEGER,
  created_at INTEGER NOT NULL, FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id),
  FOREIGN KEY (tenant_id, rule_id) REFERENCES rules(tenant_id, id)
);
CREATE TABLE rule_action_outbox (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, event_type TEXT NOT NULL, aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL, idempotency_key TEXT, payload TEXT NOT NULL CHECK(json_valid(payload)), status TEXT NOT NULL,
  attempts INTEGER NOT NULL, max_attempts INTEGER NOT NULL, available_at INTEGER NOT NULL, locked_at INTEGER,
  locked_by TEXT, last_error TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE INDEX outbox_claim ON rule_action_outbox(status, available_at, created_at, id);
CREATE TABLE audit_events (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), actor_type TEXT NOT NULL, actor_id TEXT,
  action TEXT NOT NULL, resource_type TEXT NOT NULL, resource_id TEXT, outcome TEXT NOT NULL, request_id TEXT NOT NULL,
  metadata TEXT NOT NULL CHECK(json_valid(metadata)), occurred_at INTEGER NOT NULL
);
CREATE TABLE server_metrics (
  id INTEGER PRIMARY KEY, cpu_usage_percent REAL NOT NULL, memory_used_bytes INTEGER NOT NULL,
  memory_total_bytes INTEGER NOT NULL, disk_used_bytes INTEGER NOT NULL, disk_total_bytes INTEGER NOT NULL,
  network_rx_bytes_delta INTEGER NOT NULL, network_tx_bytes_delta INTEGER NOT NULL, load_avg_1m REAL NOT NULL,
  load_avg_5m REAL NOT NULL, load_avg_15m REAL NOT NULL, recorded_at INTEGER NOT NULL
);
CREATE TABLE app_metrics (
  id INTEGER PRIMARY KEY, request_count INTEGER NOT NULL, error_count INTEGER NOT NULL, avg_latency_ms REAL NOT NULL,
  p95_latency_ms REAL NOT NULL, db_pool_active INTEGER NOT NULL, db_pool_idle INTEGER NOT NULL,
  zenoh_messages_in INTEGER NOT NULL, zenoh_messages_out INTEGER NOT NULL, recorded_at INTEGER NOT NULL
);
INSERT INTO organizations (id, name, created_at, updated_at)
VALUES ('default', 'Default', CAST(unixepoch('subsec') * 1000000 AS INTEGER), CAST(unixepoch('subsec') * 1000000 AS INTEGER));
INSERT INTO roles (tenant_id, name, description, is_system, created_at, updated_at) VALUES
('default', 'owner', 'Full tenant owner with all permissions and lockout protection.', 1, CAST(unixepoch('subsec') * 1000000 AS INTEGER), CAST(unixepoch('subsec') * 1000000 AS INTEGER)),
('default', 'admin', 'Administrative access to tenant resources and security settings.', 1, CAST(unixepoch('subsec') * 1000000 AS INTEGER), CAST(unixepoch('subsec') * 1000000 AS INTEGER)),
('default', 'operator', 'Operational access without security administration.', 1, CAST(unixepoch('subsec') * 1000000 AS INTEGER), CAST(unixepoch('subsec') * 1000000 AS INTEGER)),
('default', 'viewer', 'Read-only operational visibility.', 1, CAST(unixepoch('subsec') * 1000000 AS INTEGER), CAST(unixepoch('subsec') * 1000000 AS INTEGER));
WITH grants(role_name, permission) AS (VALUES
('owner','api_keys.manage'),('owner','alerts.manage'),('owner','alerts.read'),('owner','commands.read'),('owner','commands.send'),
('owner','device_blueprints.manage'),('owner','device_blueprints.read'),('owner','devices.manage'),('owner','devices.read'),
('owner','firmware.deploy'),('owner','firmware.manage'),('owner','firmware.read'),('owner','fleets.manage'),('owner','fleets.read'),
('owner','logs.read'),('owner','roles.manage'),('owner','roles.read'),('owner','rules.manage'),('owner','rules.read'),
('owner','server_metrics.read'),('owner','shadows.manage'),('owner','shadows.read'),('owner','telemetry.read'),
('owner','users.manage'),('owner','users.read'),('owner','zones.manage'),('owner','zones.read'),
('admin','api_keys.manage'),('admin','alerts.manage'),('admin','alerts.read'),('admin','commands.read'),('admin','commands.send'),
('admin','device_blueprints.manage'),('admin','device_blueprints.read'),('admin','devices.manage'),('admin','devices.read'),
('admin','firmware.deploy'),('admin','firmware.manage'),('admin','firmware.read'),('admin','fleets.manage'),('admin','fleets.read'),
('admin','logs.read'),('admin','roles.manage'),('admin','roles.read'),('admin','rules.manage'),('admin','rules.read'),
('admin','server_metrics.read'),('admin','shadows.manage'),('admin','shadows.read'),('admin','telemetry.read'),
('admin','users.manage'),('admin','users.read'),('admin','zones.manage'),('admin','zones.read'),
('operator','alerts.manage'),('operator','alerts.read'),('operator','commands.read'),('operator','commands.send'),
('operator','device_blueprints.read'),('operator','devices.read'),('operator','firmware.read'),('operator','fleets.read'),
('operator','logs.read'),('operator','rules.read'),('operator','shadows.manage'),('operator','shadows.read'),
('operator','telemetry.read'),('operator','zones.read'),
('viewer','alerts.read'),('viewer','commands.read'),('viewer','device_blueprints.read'),('viewer','devices.read'),
('viewer','firmware.read'),('viewer','fleets.read'),('viewer','logs.read'),('viewer','rules.read'),
('viewer','shadows.read'),('viewer','telemetry.read'),('viewer','zones.read'))
INSERT INTO role_permissions(role_id,permission)
SELECT roles.id,grants.permission FROM roles JOIN grants ON grants.role_name=roles.name
WHERE roles.tenant_id='default';

CREATE TABLE device_contracts (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL, blueprint_revision_id TEXT NOT NULL,
  document TEXT NOT NULL CHECK(json_valid(document)),
  contract_hash TEXT NOT NULL CHECK(length(contract_hash) = 64), created_at INTEGER NOT NULL,
  UNIQUE (tenant_id, id), UNIQUE (tenant_id, device_id, id), UNIQUE (tenant_id, device_id, contract_hash),
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, blueprint_revision_id) REFERENCES device_blueprint_revisions(tenant_id, id)
);
CREATE TABLE device_contract_assignments (
  tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL, desired_contract_id TEXT NOT NULL, active_contract_id TEXT,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'converged', 'failed')),
  acknowledged_at INTEGER, error TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  PRIMARY KEY (tenant_id, device_id),
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, device_id, desired_contract_id) REFERENCES device_contracts(tenant_id, device_id, id),
  FOREIGN KEY (tenant_id, device_id, active_contract_id) REFERENCES device_contracts(tenant_id, device_id, id)
);
CREATE INDEX idx_device_contracts_device_created
  ON device_contracts(tenant_id, device_id, created_at DESC);
CREATE INDEX idx_device_contract_assignments_status
  ON device_contract_assignments(tenant_id, status, updated_at);

CREATE TABLE device_events (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL, contract_id TEXT NOT NULL, route_key TEXT NOT NULL,
  occurred_at INTEGER NOT NULL, received_at INTEGER NOT NULL,
  payload TEXT NOT NULL CHECK(json_valid(payload)), UNIQUE(tenant_id, device_id, id),
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, device_id, contract_id) REFERENCES device_contracts(tenant_id, device_id, id)
);
CREATE TABLE device_event_receipts (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL, occurred_at INTEGER NOT NULL, received_at INTEGER NOT NULL,
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_device_event_receipts_time ON device_event_receipts(occurred_at);
CREATE TABLE device_metric_samples (
  event_id TEXT NOT NULL,
  tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL, stream_key TEXT NOT NULL, field_path TEXT NOT NULL,
  value_type TEXT NOT NULL CHECK(value_type IN ('float64','int64','string','boolean','json')),
  value_double REAL, value_int INTEGER, value_text TEXT, value_bool INTEGER,
  value_json TEXT CHECK(value_json IS NULL OR json_valid(value_json)), occurred_at INTEGER NOT NULL,
  PRIMARY KEY(event_id, stream_key, field_path),
  FOREIGN KEY (tenant_id, device_id, event_id) REFERENCES device_events(tenant_id, device_id, id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_device_events_device_time
  ON device_events(tenant_id, device_id, occurred_at DESC);
CREATE INDEX idx_device_events_retention ON device_events(occurred_at);
CREATE INDEX idx_device_metric_samples_query
  ON device_metric_samples(tenant_id, device_id, stream_key, field_path, occurred_at DESC);

CREATE TABLE device_metric_rollups_hourly (
  tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL,
  blueprint_revision_id TEXT NOT NULL,
  stream_key TEXT NOT NULL,
  field_path TEXT NOT NULL,
  bucket_start INTEGER NOT NULL,
  sample_count INTEGER NOT NULL CHECK(sample_count > 0),
  value_sum REAL NOT NULL,
  value_min REAL NOT NULL,
  value_max REAL NOT NULL,
  latest_value REAL NOT NULL,
  latest_at INTEGER NOT NULL,
  latest_event_id TEXT NOT NULL,
  PRIMARY KEY(tenant_id, device_id, blueprint_revision_id, stream_key, field_path, bucket_start),
  FOREIGN KEY(tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE,
  FOREIGN KEY(tenant_id, blueprint_revision_id) REFERENCES device_blueprint_revisions(tenant_id, id)
);
CREATE INDEX idx_device_metric_rollups_query
  ON device_metric_rollups_hourly(tenant_id, device_id, stream_key, field_path, bucket_start DESC);
CREATE INDEX idx_device_metric_rollups_retention
  ON device_metric_rollups_hourly(bucket_start);
CREATE TABLE device_metric_retention_state (
  id INTEGER PRIMARY KEY CHECK(id = 1),
  raw_retained_since INTEGER NOT NULL,
  rollup_retained_since INTEGER NOT NULL
);
INSERT INTO device_metric_retention_state VALUES (1, -62135596800000000, -62135596800000000);

-- Turso stores rule target kinds as unconstrained text. Index the new
-- application-level `blueprint` target without changing the table shape.
CREATE INDEX IF NOT EXISTS idx_rules_blueprint_targets
    ON rules (tenant_id, target_id)
    WHERE target_type = 'blueprint';

CREATE UNIQUE INDEX rule_action_outbox_idempotency_active
  ON rule_action_outbox(tenant_id, event_type, idempotency_key)
  WHERE idempotency_key IS NOT NULL
    AND status IN ('pending', 'processing', 'failed');

-- Retain delivery identity after alert resolution/retention. The outbox row
-- owns receipt lifetime; alert_id deliberately has no cascading foreign key.
CREATE TABLE rule_alert_deliveries (
    delivery_id TEXT PRIMARY KEY REFERENCES rule_action_outbox(id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL,
    alert_id TEXT NOT NULL
);

CREATE TABLE rule_zone_entries (
    tenant_id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    entered_at INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, rule_id, device_id),
    FOREIGN KEY (tenant_id,rule_id) REFERENCES rules(tenant_id,id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id,device_id) REFERENCES devices(tenant_id,id) ON DELETE CASCADE
);
CREATE INDEX rule_zone_entries_device ON rule_zone_entries(tenant_id,device_id);

CREATE INDEX api_keys_blueprint ON api_keys(tenant_id, blueprint_id);
CREATE INDEX firmware_updates_revision ON firmware_updates(tenant_id, blueprint_revision_id, created_at DESC, id DESC);
CREATE INDEX ota_deployments_device ON ota_deployments(tenant_id, device_id, initiated_at DESC, id DESC);
