ALTER TABLE rule_action_outbox
    ADD COLUMN IF NOT EXISTS idempotency_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_rule_action_outbox_idempotency_active
    ON rule_action_outbox(tenant_id, event_type, idempotency_key)
    WHERE idempotency_key IS NOT NULL
      AND status IN ('pending', 'processing', 'failed');
