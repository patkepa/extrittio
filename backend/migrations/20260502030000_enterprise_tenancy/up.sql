-- Add the enterprise tenancy boundary without changing existing application
-- behavior. All current data is assigned to the built-in default organization.

CREATE TABLE organizations (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO organizations (id, name)
VALUES ('default', 'Default Organization');

ALTER TABLE users
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE api_keys
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE device_types
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE fleets
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE devices
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE firmware_updates
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE command_history
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE telemetry
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE device_logs
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE device_shadows
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE device_configs
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE device_certificates
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE firmware_blobs
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE ota_deployments
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE rules
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE rule_actions
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE rule_conditions
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE rule_cooldowns
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE alerts
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE zones
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);
ALTER TABLE network_observed_hosts
    ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES organizations(id);

CREATE INDEX idx_users_tenant_id ON users(tenant_id);
CREATE INDEX idx_api_keys_tenant_id ON api_keys(tenant_id);
CREATE INDEX idx_device_types_tenant_id ON device_types(tenant_id);
CREATE INDEX idx_fleets_tenant_id ON fleets(tenant_id);
CREATE INDEX idx_devices_tenant_id ON devices(tenant_id);
CREATE INDEX idx_devices_tenant_status ON devices(tenant_id, status);
CREATE INDEX idx_devices_tenant_fleet ON devices(tenant_id, fleet_id);
CREATE INDEX idx_firmware_updates_tenant_id ON firmware_updates(tenant_id);
CREATE INDEX idx_command_history_tenant_device_created
    ON command_history(tenant_id, device_id, created_at);
CREATE INDEX idx_telemetry_tenant_device_received
    ON telemetry(tenant_id, device_id, received_at);
CREATE INDEX idx_device_logs_tenant_device_created
    ON device_logs(tenant_id, device_id, created_at);
CREATE INDEX idx_ota_deployments_tenant_device_id
    ON ota_deployments(tenant_id, device_id);
CREATE INDEX idx_rules_tenant_target ON rules(tenant_id, target_type, target_id);
CREATE INDEX idx_alerts_tenant_device_status ON alerts(tenant_id, device_id, status);
CREATE INDEX idx_zones_tenant_id ON zones(tenant_id);
CREATE INDEX idx_network_observed_hosts_tenant_analyzer
    ON network_observed_hosts(tenant_id, analyzer_device_id);
