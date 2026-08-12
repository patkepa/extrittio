CREATE TABLE audit_events (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    actor_type TEXT NOT NULL,
    actor_id TEXT,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    outcome TEXT NOT NULL,
    request_id TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_events_tenant_occurred
    ON audit_events (tenant_id, occurred_at DESC);
CREATE INDEX idx_audit_events_tenant_resource
    ON audit_events (tenant_id, resource_type, resource_id, occurred_at DESC);
CREATE INDEX idx_audit_events_request_id ON audit_events (request_id);
