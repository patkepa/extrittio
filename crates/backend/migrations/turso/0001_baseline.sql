CREATE TABLE organizations (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE server_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE device_types (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL,
  icon TEXT NOT NULL, color_hex TEXT NOT NULL, created_at INTEGER NOT NULL,
  UNIQUE (tenant_id, name), UNIQUE (tenant_id, id)
);
CREATE TABLE fleets (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL,
  created_at INTEGER NOT NULL, UNIQUE (tenant_id, name), UNIQUE (tenant_id, id)
);
CREATE TABLE devices (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL,
  device_type_id INTEGER NOT NULL, fleet_id INTEGER, status TEXT NOT NULL, firmware TEXT NOT NULL,
  last_seen INTEGER, uptime_seconds INTEGER NOT NULL DEFAULT 0, latest_latitude REAL,
  latest_longitude REAL, declared_connections TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(declared_connections)),
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE (tenant_id, id),
  FOREIGN KEY (tenant_id, device_type_id) REFERENCES device_types(tenant_id, id),
  FOREIGN KEY (tenant_id, fleet_id) REFERENCES fleets(tenant_id, id)
);
CREATE TABLE users (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), username TEXT NOT NULL,
  password_hash TEXT NOT NULL, role TEXT NOT NULL, is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)),
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
  key_hash TEXT NOT NULL, key_prefix TEXT NOT NULL, device_type_id INTEGER, created_at INTEGER NOT NULL,
  last_used_at INTEGER, UNIQUE (tenant_id, name), UNIQUE (key_hash),
  FOREIGN KEY (tenant_id, device_type_id) REFERENCES device_types(tenant_id, id)
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
CREATE TABLE telemetry (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, payload BLOB NOT NULL,
  temperature REAL, humidity REAL, battery_level REAL, custom_json TEXT CHECK(custom_json IS NULL OR json_valid(custom_json)),
  latitude REAL, longitude REAL, speed REAL, altitude REAL, heading REAL, received_at INTEGER NOT NULL,
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX telemetry_device_time ON telemetry(tenant_id, device_id, received_at DESC, id DESC);
CREATE TABLE telemetry_rollups_hourly (
  tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, bucket_start INTEGER NOT NULL, sample_count INTEGER NOT NULL,
  avg_temperature REAL, min_temperature REAL, max_temperature REAL, avg_humidity REAL, min_humidity REAL,
  max_humidity REAL, avg_battery_level REAL, min_battery_level REAL, max_battery_level REAL,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY (tenant_id, device_id, bucket_start)
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
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, device_type_id INTEGER NOT NULL, version TEXT NOT NULL,
  url TEXT NOT NULL, description TEXT, sha256 TEXT, commit_sha TEXT, branch TEXT, ci_run_url TEXT,
  build_timestamp INTEGER, changelog TEXT, source TEXT NOT NULL, created_at INTEGER NOT NULL,
  UNIQUE (tenant_id, device_type_id, version),
  FOREIGN KEY (tenant_id, device_type_id) REFERENCES device_types(tenant_id, id)
);
CREATE TABLE firmware_blobs (
  firmware_update_id INTEGER PRIMARY KEY REFERENCES firmware_updates(id) ON DELETE CASCADE,
  tenant_id TEXT NOT NULL, data BLOB, size INTEGER NOT NULL, filename TEXT NOT NULL, storage_key TEXT,
  storage_backend TEXT NOT NULL, created_at INTEGER NOT NULL
);
CREATE TABLE ota_deployments (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, device_id TEXT NOT NULL, firmware_update_id INTEGER NOT NULL,
  status TEXT NOT NULL, error_message TEXT, initiated_at INTEGER NOT NULL, completed_at INTEGER,
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id),
  FOREIGN KEY (firmware_update_id) REFERENCES firmware_updates(id)
);
CREATE TABLE zones (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL, description TEXT NOT NULL,
  geometry_type TEXT NOT NULL, geometry_json TEXT NOT NULL CHECK(json_valid(geometry_json)), color TEXT NOT NULL,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE (tenant_id, name), UNIQUE (tenant_id, id)
);
CREATE TABLE rules (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), name TEXT NOT NULL, description TEXT,
  enabled INTEGER NOT NULL CHECK(enabled IN (0,1)), trigger_type TEXT NOT NULL, target_type TEXT NOT NULL,
  target_id TEXT, cooldown_seconds INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  UNIQUE (tenant_id, name), UNIQUE (tenant_id, id)
);
CREATE TABLE rule_conditions (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, rule_id TEXT NOT NULL, field TEXT NOT NULL, operator TEXT NOT NULL,
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
  locked_by TEXT, last_error TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  UNIQUE (tenant_id, idempotency_key)
);
CREATE INDEX outbox_claim ON rule_action_outbox(status, available_at, created_at, id);
CREATE TABLE audit_events (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id), actor_type TEXT NOT NULL, actor_id TEXT,
  action TEXT NOT NULL, resource_type TEXT NOT NULL, resource_id TEXT, outcome TEXT NOT NULL, request_id TEXT NOT NULL,
  metadata TEXT NOT NULL CHECK(json_valid(metadata)), occurred_at INTEGER NOT NULL
);
CREATE TABLE network_observed_hosts (
  id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, analyzer_device_id TEXT NOT NULL, host_key TEXT NOT NULL,
  label TEXT NOT NULL, address TEXT, device_type TEXT, source TEXT, status TEXT NOT NULL, first_seen_at INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  UNIQUE (tenant_id, analyzer_device_id, host_key),
  FOREIGN KEY (tenant_id, analyzer_device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
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
