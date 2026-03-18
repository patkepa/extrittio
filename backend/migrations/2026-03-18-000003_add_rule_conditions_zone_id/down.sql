PRAGMA foreign_keys = OFF;

CREATE TABLE rule_conditions_backup (
    id TEXT PRIMARY KEY NOT NULL,
    rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    operator TEXT NOT NULL,
    value TEXT NOT NULL,
    condition_group INTEGER NOT NULL DEFAULT 0
);

INSERT INTO rule_conditions_backup (id, rule_id, field, operator, value, condition_group)
SELECT id, rule_id, field, operator, value, condition_group FROM rule_conditions;

DROP TABLE rule_conditions;
ALTER TABLE rule_conditions_backup RENAME TO rule_conditions;

PRAGMA foreign_keys = ON;
