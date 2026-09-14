CREATE TABLE rule_cooldown_resets (
    tenant_id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    reset_at INTEGER NOT NULL,
    PRIMARY KEY (tenant_id,rule_id,device_id),
    FOREIGN KEY (tenant_id,rule_id) REFERENCES rules(tenant_id,id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id,device_id) REFERENCES devices(tenant_id,id) ON DELETE CASCADE
);
CREATE INDEX rule_cooldown_resets_device ON rule_cooldown_resets(tenant_id,device_id);
