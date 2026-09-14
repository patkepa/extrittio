-- IDs are already globally unique; these tenant-qualified keys let the new
-- runtime table enforce tenant ownership on both of its parent references.
CREATE UNIQUE INDEX rule_zone_entries_rules_tenant_ref ON rules(tenant_id,id);
CREATE UNIQUE INDEX rule_zone_entries_devices_tenant_ref ON devices(tenant_id,id);
CREATE TABLE rule_zone_entries (
    tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    rule_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    entered_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id, rule_id, device_id),
    FOREIGN KEY (tenant_id,rule_id) REFERENCES rules(tenant_id,id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id,device_id) REFERENCES devices(tenant_id,id) ON DELETE CASCADE
);
CREATE INDEX rule_zone_entries_device ON rule_zone_entries(tenant_id,device_id);
