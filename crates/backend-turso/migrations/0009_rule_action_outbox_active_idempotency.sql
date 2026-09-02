ALTER TABLE rule_action_outbox RENAME TO rule_action_outbox_legacy;

CREATE TABLE rule_action_outbox (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  idempotency_key TEXT,
  payload TEXT NOT NULL CHECK(json_valid(payload)),
  status TEXT NOT NULL,
  attempts INTEGER NOT NULL,
  max_attempts INTEGER NOT NULL,
  available_at INTEGER NOT NULL,
  locked_at INTEGER,
  locked_by TEXT,
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

INSERT INTO rule_action_outbox (
  id, tenant_id, event_type, aggregate_type, aggregate_id, idempotency_key,
  payload, status, attempts, max_attempts, available_at, locked_at, locked_by,
  last_error, created_at, updated_at
)
SELECT
  id, tenant_id, event_type, aggregate_type, aggregate_id, idempotency_key,
  payload, status, attempts, max_attempts, available_at, locked_at, locked_by,
  last_error, created_at, updated_at
FROM rule_action_outbox_legacy;

DROP TABLE rule_action_outbox_legacy;

CREATE INDEX outbox_claim
  ON rule_action_outbox(status, available_at, created_at, id);

CREATE UNIQUE INDEX rule_action_outbox_idempotency_active
  ON rule_action_outbox(tenant_id, event_type, idempotency_key)
  WHERE idempotency_key IS NOT NULL
    AND status IN ('pending', 'processing', 'failed');
