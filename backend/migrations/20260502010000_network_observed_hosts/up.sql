CREATE TABLE IF NOT EXISTS network_observed_hosts (
    id BIGSERIAL PRIMARY KEY,
    analyzer_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
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
    UNIQUE(analyzer_device_id, host_key)
);

CREATE INDEX IF NOT EXISTS idx_network_observed_hosts_analyzer_last_seen
ON network_observed_hosts(analyzer_device_id, last_seen_at DESC);

CREATE INDEX IF NOT EXISTS idx_network_observed_hosts_last_seen
ON network_observed_hosts(last_seen_at);
