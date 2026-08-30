UPDATE rules SET target_type = 'global', target_id = NULL WHERE target_type = 'blueprint';
ALTER TABLE rules DROP CONSTRAINT rules_target_type_check;
ALTER TABLE rules ADD CONSTRAINT rules_target_type_check
    CHECK (target_type IN ('global', 'device_type', 'fleet', 'device'));
