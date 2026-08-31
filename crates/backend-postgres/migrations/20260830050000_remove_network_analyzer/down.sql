INSERT INTO device_types (tenant_id, name, icon, color_hex)
SELECT id, 'network-analyzer', 'antenna', '#36CFC9'
FROM organizations
ON CONFLICT (tenant_id, name) DO NOTHING;

CREATE TABLE network_observed_hosts (
    id BIGSERIAL PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    analyzer_device_id TEXT NOT NULL,
    host_key TEXT NOT NULL,
    label TEXT NOT NULL,
    address TEXT,
    device_type TEXT,
    source TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, analyzer_device_id, host_key),
    FOREIGN KEY (tenant_id, analyzer_device_id)
        REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_network_observed_hosts_analyzer_last_seen
    ON network_observed_hosts(analyzer_device_id, last_seen_at DESC);
CREATE INDEX idx_network_observed_hosts_last_seen
    ON network_observed_hosts(last_seen_at);
CREATE INDEX idx_network_observed_hosts_tenant_analyzer
    ON network_observed_hosts(tenant_id, analyzer_device_id);
