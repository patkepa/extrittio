ALTER TABLE rule_conditions ADD COLUMN zone_id TEXT REFERENCES zones(id);
CREATE INDEX idx_rule_conditions_zone_id ON rule_conditions(zone_id);
