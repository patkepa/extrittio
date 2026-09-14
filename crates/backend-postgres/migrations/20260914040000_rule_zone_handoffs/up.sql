CREATE TABLE rule_zone_handoffs (
    tenant_id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    live_seen BOOLEAN NOT NULL,
    legacy_created_at TIMESTAMPTZ,
    legacy_event_id TEXT,
    PRIMARY KEY (tenant_id,rule_id,device_id),
    FOREIGN KEY (tenant_id,rule_id) REFERENCES rules(tenant_id,id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id,device_id) REFERENCES devices(tenant_id,id) ON DELETE CASCADE
);
CREATE INDEX rule_zone_handoffs_device ON rule_zone_handoffs(tenant_id,device_id);
-- Existing persisted entries were written by live transaction evaluation.
INSERT INTO rule_zone_handoffs(tenant_id,rule_id,device_id,live_seen)
SELECT tenant_id,rule_id,device_id,TRUE FROM rule_zone_entries;
