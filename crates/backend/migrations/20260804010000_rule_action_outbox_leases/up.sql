-- Support efficient recovery of events whose processing worker disappeared.
CREATE INDEX idx_rule_action_outbox_processing_lease
    ON rule_action_outbox (locked_at, created_at)
    WHERE status = 'processing';
