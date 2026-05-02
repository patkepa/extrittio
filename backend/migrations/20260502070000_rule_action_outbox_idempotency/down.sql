DROP INDEX IF EXISTS idx_rule_action_outbox_idempotency_active;

ALTER TABLE rule_action_outbox
    DROP COLUMN IF EXISTS idempotency_key;
