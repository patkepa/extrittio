ALTER TABLE rules DROP CONSTRAINT rules_target_type_check;
ALTER TABLE rules ADD CONSTRAINT rules_target_type_check
    CHECK (target_type IN ('global', 'blueprint', 'device_type', 'fleet', 'device'));
