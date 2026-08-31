ALTER TABLE devices DROP CONSTRAINT IF EXISTS devices_tenant_name_key;
ALTER TABLE devices ADD CONSTRAINT devices_name_unique UNIQUE (name);

DROP INDEX IF EXISTS idx_rule_cooldowns_tenant_device;
DROP INDEX IF EXISTS idx_rule_conditions_tenant_zone;
DROP INDEX IF EXISTS idx_rule_conditions_tenant_rule;
DROP INDEX IF EXISTS idx_rule_actions_tenant_rule;
DROP INDEX IF EXISTS idx_device_configs_tenant_device;
DROP INDEX IF EXISTS idx_device_shadows_tenant_device;
DROP INDEX IF EXISTS idx_device_certificates_tenant_device;
DROP INDEX IF EXISTS idx_ota_deployments_tenant_update;
DROP INDEX IF EXISTS idx_firmware_blobs_tenant_update;
DROP INDEX IF EXISTS idx_api_keys_tenant_device_type;
DROP INDEX IF EXISTS idx_devices_tenant_device_type;

ALTER TABLE user_roles
    DROP CONSTRAINT IF EXISTS user_roles_tenant_role_fk,
    DROP CONSTRAINT IF EXISTS user_roles_tenant_user_fk;
ALTER TABLE alerts
    DROP CONSTRAINT IF EXISTS alerts_tenant_device_fk,
    DROP CONSTRAINT IF EXISTS alerts_tenant_rule_fk;
ALTER TABLE rule_cooldowns
    DROP CONSTRAINT IF EXISTS rule_cooldowns_tenant_device_fk,
    DROP CONSTRAINT IF EXISTS rule_cooldowns_tenant_rule_fk;
ALTER TABLE rule_conditions
    DROP CONSTRAINT IF EXISTS rule_conditions_tenant_zone_fk,
    DROP CONSTRAINT IF EXISTS rule_conditions_tenant_rule_fk;
ALTER TABLE rule_actions DROP CONSTRAINT IF EXISTS rule_actions_tenant_rule_fk;
ALTER TABLE network_observed_hosts DROP CONSTRAINT IF EXISTS network_hosts_tenant_device_fk;
ALTER TABLE command_history DROP CONSTRAINT IF EXISTS command_history_tenant_device_fk;
ALTER TABLE device_certificates DROP CONSTRAINT IF EXISTS device_certificates_tenant_device_fk;
ALTER TABLE device_configs DROP CONSTRAINT IF EXISTS device_configs_tenant_device_fk;
ALTER TABLE device_shadows DROP CONSTRAINT IF EXISTS device_shadows_tenant_device_fk;
ALTER TABLE device_logs DROP CONSTRAINT IF EXISTS device_logs_tenant_device_fk;
ALTER TABLE telemetry_rollups_hourly DROP CONSTRAINT IF EXISTS telemetry_rollups_tenant_device_fk;
ALTER TABLE telemetry DROP CONSTRAINT IF EXISTS telemetry_tenant_device_fk;
ALTER TABLE ota_deployments
    DROP CONSTRAINT IF EXISTS ota_deployments_tenant_update_fk,
    DROP CONSTRAINT IF EXISTS ota_deployments_tenant_device_fk;
ALTER TABLE firmware_blobs DROP CONSTRAINT IF EXISTS firmware_blobs_tenant_update_fk;
ALTER TABLE firmware_updates DROP CONSTRAINT IF EXISTS firmware_updates_tenant_device_type_fk;
ALTER TABLE api_keys DROP CONSTRAINT IF EXISTS api_keys_tenant_device_type_fk;
ALTER TABLE devices
    DROP CONSTRAINT IF EXISTS devices_tenant_fleet_fk,
    DROP CONSTRAINT IF EXISTS devices_tenant_device_type_fk;

ALTER TABLE roles DROP CONSTRAINT IF EXISTS roles_tenant_id_id_key;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_tenant_id_id_key;
ALTER TABLE zones DROP CONSTRAINT IF EXISTS zones_tenant_id_id_key;
ALTER TABLE rules DROP CONSTRAINT IF EXISTS rules_tenant_id_id_key;
ALTER TABLE firmware_updates DROP CONSTRAINT IF EXISTS firmware_updates_tenant_id_id_key;
ALTER TABLE devices DROP CONSTRAINT IF EXISTS devices_tenant_id_id_key;
ALTER TABLE fleets DROP CONSTRAINT IF EXISTS fleets_tenant_id_id_key;
ALTER TABLE device_types DROP CONSTRAINT IF EXISTS device_types_tenant_id_id_key;
