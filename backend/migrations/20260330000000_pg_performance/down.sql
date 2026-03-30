-- Revert JSONB -> TEXT
ALTER TABLE rule_actions ALTER COLUMN config TYPE TEXT USING config::text;

ALTER TABLE zones ALTER COLUMN geometry_json TYPE TEXT USING geometry_json::text;

ALTER TABLE telemetry ALTER COLUMN custom_json TYPE TEXT USING custom_json::text;

ALTER TABLE device_configs ALTER COLUMN config TYPE TEXT USING config::text;
ALTER TABLE device_configs ALTER COLUMN config SET DEFAULT '{}';

ALTER TABLE device_shadows ALTER COLUMN delta TYPE TEXT USING delta::text;
ALTER TABLE device_shadows ALTER COLUMN reported TYPE TEXT USING reported::text;
ALTER TABLE device_shadows ALTER COLUMN desired TYPE TEXT USING desired::text;
ALTER TABLE device_shadows ALTER COLUMN desired SET DEFAULT '{}';
ALTER TABLE device_shadows ALTER COLUMN reported SET DEFAULT '{}';
ALTER TABLE device_shadows ALTER COLUMN delta SET DEFAULT '{}';

-- Drop pg_trgm
DROP INDEX IF EXISTS idx_devices_name_trgm;
DROP EXTENSION IF EXISTS pg_trgm;

-- Drop partial indexes
DROP INDEX IF EXISTS idx_devices_online_last_seen;
DROP INDEX IF EXISTS idx_commands_pending;
DROP INDEX IF EXISTS idx_alerts_active;

-- Drop BRIN indexes
DROP INDEX IF EXISTS idx_app_metrics_recorded_brin;
DROP INDEX IF EXISTS idx_server_metrics_recorded_brin;
DROP INDEX IF EXISTS idx_device_logs_created_brin;
DROP INDEX IF EXISTS idx_telemetry_received_brin;

-- Drop B-tree indexes
DROP INDEX IF EXISTS idx_rule_cooldowns_last_fired_at;
DROP INDEX IF EXISTS idx_api_keys_device_type_id;
DROP INDEX IF EXISTS idx_device_certificates_device_id;
