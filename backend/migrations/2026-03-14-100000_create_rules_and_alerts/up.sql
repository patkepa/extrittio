CREATE TABLE rules (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    trigger_type TEXT NOT NULL CHECK (trigger_type IN ('telemetry', 'device_status')),
    target_type TEXT NOT NULL CHECK (target_type IN ('global', 'device_type', 'fleet', 'device')),
    target_id TEXT,
    cooldown_seconds INTEGER NOT NULL DEFAULT 300,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE rule_conditions (
    id TEXT PRIMARY KEY NOT NULL,
    rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    operator TEXT NOT NULL CHECK (operator IN ('gt', 'gte', 'lt', 'lte', 'eq', 'neq')),
    value TEXT NOT NULL,
    condition_group INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE rule_actions (
    id TEXT PRIMARY KEY NOT NULL,
    rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL CHECK (action_type IN ('alert', 'webhook', 'command')),
    config TEXT NOT NULL
);

CREATE TABLE alerts (
    id TEXT PRIMARY KEY NOT NULL,
    rule_id TEXT REFERENCES rules(id) ON DELETE SET NULL,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'acknowledged', 'resolved')),
    message TEXT NOT NULL,
    triggered_value TEXT,
    resolved_at TIMESTAMP,
    acknowledged_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE rule_cooldowns (
    rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    last_fired_at TIMESTAMP NOT NULL,
    PRIMARY KEY (rule_id, device_id)
);

CREATE INDEX idx_rule_conditions_rule_id ON rule_conditions(rule_id);
CREATE INDEX idx_rule_actions_rule_id ON rule_actions(rule_id);
CREATE INDEX idx_alerts_status_created ON alerts(status, created_at);
CREATE INDEX idx_alerts_device_id ON alerts(device_id);
CREATE INDEX idx_alerts_rule_id ON alerts(rule_id);
