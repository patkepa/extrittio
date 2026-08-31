-- Enforce tenant consistency on every tenant-owned relationship. The original
-- foreign keys are retained so Diesel can continue to infer single-column
-- joins; these composite keys add the cross-tenant invariant at the database
-- boundary.

ALTER TABLE device_types ADD CONSTRAINT device_types_tenant_id_id_key UNIQUE (tenant_id, id);
ALTER TABLE fleets ADD CONSTRAINT fleets_tenant_id_id_key UNIQUE (tenant_id, id);
ALTER TABLE devices ADD CONSTRAINT devices_tenant_id_id_key UNIQUE (tenant_id, id);
ALTER TABLE firmware_updates ADD CONSTRAINT firmware_updates_tenant_id_id_key UNIQUE (tenant_id, id);
ALTER TABLE rules ADD CONSTRAINT rules_tenant_id_id_key UNIQUE (tenant_id, id);
ALTER TABLE zones ADD CONSTRAINT zones_tenant_id_id_key UNIQUE (tenant_id, id);
ALTER TABLE users ADD CONSTRAINT users_tenant_id_id_key UNIQUE (tenant_id, id);
ALTER TABLE roles ADD CONSTRAINT roles_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE devices
    ADD CONSTRAINT devices_tenant_device_type_fk
    FOREIGN KEY (tenant_id, device_type_id)
    REFERENCES device_types (tenant_id, id) NOT VALID,
    ADD CONSTRAINT devices_tenant_fleet_fk
    FOREIGN KEY (tenant_id, fleet_id)
    REFERENCES fleets (tenant_id, id) ON DELETE SET NULL (fleet_id) NOT VALID;

ALTER TABLE api_keys
    ADD CONSTRAINT api_keys_tenant_device_type_fk
    FOREIGN KEY (tenant_id, device_type_id)
    REFERENCES device_types (tenant_id, id) ON DELETE SET NULL (device_type_id) NOT VALID;

ALTER TABLE firmware_updates
    ADD CONSTRAINT firmware_updates_tenant_device_type_fk
    FOREIGN KEY (tenant_id, device_type_id)
    REFERENCES device_types (tenant_id, id) ON DELETE CASCADE NOT VALID;

ALTER TABLE firmware_blobs
    ADD CONSTRAINT firmware_blobs_tenant_update_fk
    FOREIGN KEY (tenant_id, firmware_update_id)
    REFERENCES firmware_updates (tenant_id, id) ON DELETE CASCADE NOT VALID;

ALTER TABLE ota_deployments
    ADD CONSTRAINT ota_deployments_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID,
    ADD CONSTRAINT ota_deployments_tenant_update_fk
    FOREIGN KEY (tenant_id, firmware_update_id)
    REFERENCES firmware_updates (tenant_id, id) ON DELETE CASCADE NOT VALID;

ALTER TABLE telemetry
    ADD CONSTRAINT telemetry_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE telemetry_rollups_hourly
    ADD CONSTRAINT telemetry_rollups_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE device_logs
    ADD CONSTRAINT device_logs_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE device_shadows
    ADD CONSTRAINT device_shadows_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE device_configs
    ADD CONSTRAINT device_configs_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE device_certificates
    ADD CONSTRAINT device_certificates_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE command_history
    ADD CONSTRAINT command_history_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE network_observed_hosts
    ADD CONSTRAINT network_hosts_tenant_device_fk
    FOREIGN KEY (tenant_id, analyzer_device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;

ALTER TABLE rule_actions
    ADD CONSTRAINT rule_actions_tenant_rule_fk
    FOREIGN KEY (tenant_id, rule_id)
    REFERENCES rules (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE rule_conditions
    ADD CONSTRAINT rule_conditions_tenant_rule_fk
    FOREIGN KEY (tenant_id, rule_id)
    REFERENCES rules (tenant_id, id) ON DELETE CASCADE NOT VALID,
    ADD CONSTRAINT rule_conditions_tenant_zone_fk
    FOREIGN KEY (tenant_id, zone_id)
    REFERENCES zones (tenant_id, id) NOT VALID;
ALTER TABLE rule_cooldowns
    ADD CONSTRAINT rule_cooldowns_tenant_rule_fk
    FOREIGN KEY (tenant_id, rule_id)
    REFERENCES rules (tenant_id, id) ON DELETE CASCADE NOT VALID,
    ADD CONSTRAINT rule_cooldowns_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;
ALTER TABLE alerts
    ADD CONSTRAINT alerts_tenant_rule_fk
    FOREIGN KEY (tenant_id, rule_id)
    REFERENCES rules (tenant_id, id) ON DELETE SET NULL (rule_id) NOT VALID,
    ADD CONSTRAINT alerts_tenant_device_fk
    FOREIGN KEY (tenant_id, device_id)
    REFERENCES devices (tenant_id, id) ON DELETE CASCADE NOT VALID;

ALTER TABLE user_roles
    ADD CONSTRAINT user_roles_tenant_user_fk
    FOREIGN KEY (tenant_id, user_id)
    REFERENCES users (tenant_id, id) ON DELETE CASCADE NOT VALID,
    ADD CONSTRAINT user_roles_tenant_role_fk
    FOREIGN KEY (tenant_id, role_id)
    REFERENCES roles (tenant_id, id) ON DELETE CASCADE NOT VALID;

-- Validate separately so production deployments can add each constraint with
-- a short initial lock while still rejecting all new inconsistent writes.
ALTER TABLE devices VALIDATE CONSTRAINT devices_tenant_device_type_fk;
ALTER TABLE devices VALIDATE CONSTRAINT devices_tenant_fleet_fk;
ALTER TABLE api_keys VALIDATE CONSTRAINT api_keys_tenant_device_type_fk;
ALTER TABLE firmware_updates VALIDATE CONSTRAINT firmware_updates_tenant_device_type_fk;
ALTER TABLE firmware_blobs VALIDATE CONSTRAINT firmware_blobs_tenant_update_fk;
ALTER TABLE ota_deployments VALIDATE CONSTRAINT ota_deployments_tenant_device_fk;
ALTER TABLE ota_deployments VALIDATE CONSTRAINT ota_deployments_tenant_update_fk;
ALTER TABLE telemetry VALIDATE CONSTRAINT telemetry_tenant_device_fk;
ALTER TABLE telemetry_rollups_hourly VALIDATE CONSTRAINT telemetry_rollups_tenant_device_fk;
ALTER TABLE device_logs VALIDATE CONSTRAINT device_logs_tenant_device_fk;
ALTER TABLE device_shadows VALIDATE CONSTRAINT device_shadows_tenant_device_fk;
ALTER TABLE device_configs VALIDATE CONSTRAINT device_configs_tenant_device_fk;
ALTER TABLE device_certificates VALIDATE CONSTRAINT device_certificates_tenant_device_fk;
ALTER TABLE command_history VALIDATE CONSTRAINT command_history_tenant_device_fk;
ALTER TABLE network_observed_hosts VALIDATE CONSTRAINT network_hosts_tenant_device_fk;
ALTER TABLE rule_actions VALIDATE CONSTRAINT rule_actions_tenant_rule_fk;
ALTER TABLE rule_conditions VALIDATE CONSTRAINT rule_conditions_tenant_rule_fk;
ALTER TABLE rule_conditions VALIDATE CONSTRAINT rule_conditions_tenant_zone_fk;
ALTER TABLE rule_cooldowns VALIDATE CONSTRAINT rule_cooldowns_tenant_rule_fk;
ALTER TABLE rule_cooldowns VALIDATE CONSTRAINT rule_cooldowns_tenant_device_fk;
ALTER TABLE alerts VALIDATE CONSTRAINT alerts_tenant_rule_fk;
ALTER TABLE alerts VALIDATE CONSTRAINT alerts_tenant_device_fk;
ALTER TABLE user_roles VALIDATE CONSTRAINT user_roles_tenant_user_fk;
ALTER TABLE user_roles VALIDATE CONSTRAINT user_roles_tenant_role_fk;

CREATE INDEX idx_devices_tenant_device_type ON devices (tenant_id, device_type_id);
CREATE INDEX idx_api_keys_tenant_device_type ON api_keys (tenant_id, device_type_id);
CREATE INDEX idx_firmware_blobs_tenant_update ON firmware_blobs (tenant_id, firmware_update_id);
CREATE INDEX idx_ota_deployments_tenant_update ON ota_deployments (tenant_id, firmware_update_id);
CREATE INDEX idx_device_certificates_tenant_device ON device_certificates (tenant_id, device_id);
CREATE INDEX idx_device_shadows_tenant_device ON device_shadows (tenant_id, device_id);
CREATE INDEX idx_device_configs_tenant_device ON device_configs (tenant_id, device_id);
CREATE INDEX idx_rule_actions_tenant_rule ON rule_actions (tenant_id, rule_id);
CREATE INDEX idx_rule_conditions_tenant_rule ON rule_conditions (tenant_id, rule_id);
CREATE INDEX idx_rule_conditions_tenant_zone ON rule_conditions (tenant_id, zone_id);
CREATE INDEX idx_rule_cooldowns_tenant_device ON rule_cooldowns (tenant_id, device_id);

ALTER TABLE devices DROP CONSTRAINT IF EXISTS devices_name_unique;
ALTER TABLE devices
    ADD CONSTRAINT devices_tenant_name_key UNIQUE (tenant_id, name);
