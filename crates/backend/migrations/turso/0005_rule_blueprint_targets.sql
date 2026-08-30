-- Turso stores rule target kinds as unconstrained text. Index the new
-- application-level `blueprint` target without changing the table shape.
CREATE INDEX IF NOT EXISTS idx_rules_blueprint_targets
    ON rules (tenant_id, target_id)
    WHERE target_type = 'blueprint';
