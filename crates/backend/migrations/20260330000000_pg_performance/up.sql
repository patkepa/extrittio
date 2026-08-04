-- ==========================================================================
-- Missing B-tree indexes
-- ==========================================================================
CREATE INDEX idx_device_certificates_device_id ON device_certificates(device_id);
CREATE INDEX idx_api_keys_device_type_id ON api_keys(device_type_id);
CREATE INDEX idx_rule_cooldowns_last_fired_at ON rule_cooldowns(last_fired_at);

-- ==========================================================================
-- BRIN indexes for time-series tables (append-only, naturally ordered)
-- ==========================================================================
CREATE INDEX idx_telemetry_received_brin ON telemetry USING BRIN(received_at) WITH (pages_per_range = 32);
CREATE INDEX idx_device_logs_created_brin ON device_logs USING BRIN(created_at) WITH (pages_per_range = 32);
CREATE INDEX idx_server_metrics_recorded_brin ON server_metrics USING BRIN(recorded_at) WITH (pages_per_range = 32);
CREATE INDEX idx_app_metrics_recorded_brin ON app_metrics USING BRIN(recorded_at) WITH (pages_per_range = 32);

-- ==========================================================================
-- Partial indexes (smaller, faster, only index rows that matter)
-- ==========================================================================
CREATE INDEX idx_alerts_active ON alerts(device_id, created_at) WHERE status != 'resolved';
CREATE INDEX idx_commands_pending ON command_history(created_at) WHERE status IN ('sent', 'delivered');
CREATE INDEX idx_devices_online_last_seen ON devices(last_seen) WHERE status != 'offline';

-- ==========================================================================
-- pg_trgm for indexed ILIKE/LIKE wildcard search
-- ==========================================================================
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX idx_devices_name_trgm ON devices USING GIN(name gin_trgm_ops);

-- ==========================================================================
-- TEXT -> JSONB column migrations
-- Drop existing text defaults first so ALTER TYPE can succeed,
-- then set jsonb-typed defaults.
-- ==========================================================================
ALTER TABLE device_shadows ALTER COLUMN desired DROP DEFAULT;
ALTER TABLE device_shadows ALTER COLUMN reported DROP DEFAULT;
ALTER TABLE device_shadows ALTER COLUMN delta DROP DEFAULT;

ALTER TABLE device_shadows ALTER COLUMN desired TYPE JSONB USING desired::jsonb;
ALTER TABLE device_shadows ALTER COLUMN reported TYPE JSONB USING reported::jsonb;
ALTER TABLE device_shadows ALTER COLUMN delta TYPE JSONB USING delta::jsonb;

ALTER TABLE device_shadows ALTER COLUMN desired SET DEFAULT '{}'::jsonb;
ALTER TABLE device_shadows ALTER COLUMN reported SET DEFAULT '{}'::jsonb;
ALTER TABLE device_shadows ALTER COLUMN delta SET DEFAULT '{}'::jsonb;

ALTER TABLE device_configs ALTER COLUMN config DROP DEFAULT;
ALTER TABLE device_configs ALTER COLUMN config TYPE JSONB USING config::jsonb;
ALTER TABLE device_configs ALTER COLUMN config SET DEFAULT '{}'::jsonb;

ALTER TABLE telemetry ALTER COLUMN custom_json TYPE JSONB USING custom_json::jsonb;

ALTER TABLE zones ALTER COLUMN geometry_json TYPE JSONB USING geometry_json::jsonb;

ALTER TABLE rule_actions ALTER COLUMN config TYPE JSONB USING config::jsonb;
