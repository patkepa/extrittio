-- Extrittio PostgreSQL Schema (consolidated from 27 SQLite migrations)

-- ==========================================================================
-- Core lookup tables
-- ==========================================================================

CREATE TABLE device_types (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE fleets (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ==========================================================================
-- Devices
-- ==========================================================================

CREATE TABLE devices (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    device_type_id INTEGER NOT NULL REFERENCES device_types(id),
    fleet_id INTEGER REFERENCES fleets(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'offline',
    firmware TEXT NOT NULL DEFAULT '',
    last_seen TIMESTAMPTZ,
    uptime_seconds INTEGER NOT NULL DEFAULT 0,
    latest_latitude DOUBLE PRECISION,
    latest_longitude DOUBLE PRECISION,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_devices_device_type_id ON devices(device_type_id);
CREATE INDEX idx_devices_fleet_id ON devices(fleet_id);
CREATE INDEX idx_devices_status ON devices(status);
CREATE INDEX idx_devices_last_seen ON devices(last_seen);

-- ==========================================================================
-- Telemetry
-- ==========================================================================

CREATE TABLE telemetry (
    id BIGSERIAL PRIMARY KEY,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    payload BYTEA NOT NULL,
    temperature REAL,
    humidity REAL,
    battery_level REAL,
    custom_json TEXT,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    speed REAL,
    altitude REAL,
    heading REAL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_telemetry_device_received ON telemetry(device_id, received_at);

-- ==========================================================================
-- Device Shadows
-- ==========================================================================

CREATE TABLE device_shadows (
    device_id TEXT PRIMARY KEY NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    desired TEXT NOT NULL DEFAULT '{}',
    reported TEXT NOT NULL DEFAULT '{}',
    delta TEXT NOT NULL DEFAULT '{}',
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ==========================================================================
-- Device Configs
-- ==========================================================================

CREATE TABLE device_configs (
    device_id TEXT PRIMARY KEY NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    config TEXT NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ==========================================================================
-- Users & Auth
-- ==========================================================================

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'admin',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE server_config (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE api_keys (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,
    device_type_id INTEGER REFERENCES device_types(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

-- ==========================================================================
-- Firmware & OTA
-- ==========================================================================

CREATE TABLE firmware_updates (
    id SERIAL PRIMARY KEY,
    device_type_id INTEGER NOT NULL REFERENCES device_types(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    url TEXT NOT NULL,
    description TEXT,
    sha256 TEXT,
    commit_sha TEXT,
    branch TEXT,
    ci_run_url TEXT,
    build_timestamp TIMESTAMPTZ,
    changelog TEXT,
    source TEXT NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(device_type_id, version)
);

CREATE INDEX idx_firmware_updates_device_type_id ON firmware_updates(device_type_id);

CREATE TABLE firmware_blobs (
    firmware_update_id INTEGER PRIMARY KEY NOT NULL REFERENCES firmware_updates(id) ON DELETE CASCADE,
    data BYTEA NOT NULL,
    size INTEGER NOT NULL,
    filename TEXT NOT NULL
);

CREATE TABLE ota_deployments (
    id SERIAL PRIMARY KEY,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    firmware_update_id INTEGER NOT NULL REFERENCES firmware_updates(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    initiated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_ota_deployments_device_id ON ota_deployments(device_id);
CREATE INDEX idx_ota_deployments_firmware_update_id ON ota_deployments(firmware_update_id);

-- ==========================================================================
-- Device Logs
-- ==========================================================================

CREATE TABLE device_logs (
    id BIGSERIAL PRIMARY KEY,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    level TEXT NOT NULL DEFAULT 'INFO',
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_device_logs_device_id ON device_logs(device_id);
CREATE INDEX idx_device_logs_device_created ON device_logs(device_id, created_at);

-- ==========================================================================
-- Command History
-- ==========================================================================

CREATE TABLE command_history (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    command TEXT NOT NULL,
    params TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'sent',
    response_payload TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_command_history_device_id ON command_history(device_id);
CREATE INDEX idx_command_history_device_created ON command_history(device_id, created_at);
CREATE INDEX idx_command_history_status ON command_history(status);

-- ==========================================================================
-- Certificates
-- ==========================================================================

CREATE TABLE ca_certificates (
    id SERIAL PRIMARY KEY,
    private_key_pem TEXT NOT NULL,
    certificate_pem TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE device_certificates (
    id SERIAL PRIMARY KEY,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    private_key_pem TEXT NOT NULL,
    certificate_pem TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ==========================================================================
-- Server & App Metrics
-- ==========================================================================

CREATE TABLE server_metrics (
    id BIGSERIAL PRIMARY KEY,
    cpu_usage_percent REAL NOT NULL,
    memory_used_bytes BIGINT NOT NULL,
    memory_total_bytes BIGINT NOT NULL,
    disk_used_bytes BIGINT NOT NULL,
    disk_total_bytes BIGINT NOT NULL,
    network_rx_bytes_delta BIGINT NOT NULL,
    network_tx_bytes_delta BIGINT NOT NULL,
    load_avg_1m REAL NOT NULL,
    load_avg_5m REAL NOT NULL,
    load_avg_15m REAL NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_server_metrics_recorded_at ON server_metrics(recorded_at);

CREATE TABLE app_metrics (
    id BIGSERIAL PRIMARY KEY,
    request_count INTEGER NOT NULL,
    error_count INTEGER NOT NULL,
    avg_latency_ms REAL NOT NULL,
    p95_latency_ms REAL NOT NULL,
    db_pool_active INTEGER NOT NULL,
    db_pool_idle INTEGER NOT NULL,
    zenoh_messages_in INTEGER NOT NULL,
    zenoh_messages_out INTEGER NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_app_metrics_recorded_at ON app_metrics(recorded_at);

-- ==========================================================================
-- Rules & Alerts
-- ==========================================================================

CREATE TABLE zones (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    geometry_type TEXT NOT NULL,
    geometry_json TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#4A90D9',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE rules (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    trigger_type TEXT NOT NULL CHECK (trigger_type IN ('telemetry', 'device_status')),
    target_type TEXT NOT NULL CHECK (target_type IN ('global', 'device_type', 'fleet', 'device')),
    target_id TEXT,
    cooldown_seconds INTEGER NOT NULL DEFAULT 300,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE rule_conditions (
    id TEXT PRIMARY KEY NOT NULL,
    rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    operator TEXT NOT NULL CHECK (operator IN ('gt', 'gte', 'lt', 'lte', 'eq', 'neq')),
    value TEXT NOT NULL,
    condition_group INTEGER NOT NULL DEFAULT 0,
    zone_id TEXT REFERENCES zones(id)
);

CREATE INDEX idx_rule_conditions_rule_id ON rule_conditions(rule_id);
CREATE INDEX idx_rule_conditions_zone_id ON rule_conditions(zone_id);

CREATE TABLE rule_actions (
    id TEXT PRIMARY KEY NOT NULL,
    rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL CHECK (action_type IN ('alert', 'webhook', 'command')),
    config TEXT NOT NULL
);

CREATE INDEX idx_rule_actions_rule_id ON rule_actions(rule_id);

CREATE TABLE alerts (
    id TEXT PRIMARY KEY NOT NULL,
    rule_id TEXT REFERENCES rules(id) ON DELETE SET NULL,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'acknowledged', 'resolved')),
    message TEXT NOT NULL,
    triggered_value TEXT,
    resolved_at TIMESTAMPTZ,
    acknowledged_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_alerts_status_created ON alerts(status, created_at);
CREATE INDEX idx_alerts_device_id ON alerts(device_id);
CREATE INDEX idx_alerts_rule_id ON alerts(rule_id);

CREATE TABLE rule_cooldowns (
    rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    last_fired_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (rule_id, device_id)
);

-- ==========================================================================
-- Seed data
-- ==========================================================================

INSERT INTO device_types (name) VALUES ('default');
INSERT INTO device_types (name) VALUES ('mac-device');
