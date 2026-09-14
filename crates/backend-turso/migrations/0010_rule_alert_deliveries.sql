-- Retain delivery identity after alert resolution/retention. The outbox row
-- owns receipt lifetime; alert_id deliberately has no cascading foreign key.
CREATE TABLE rule_alert_deliveries (
    delivery_id TEXT PRIMARY KEY REFERENCES rule_action_outbox(id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL,
    alert_id TEXT NOT NULL
);
