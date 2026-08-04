CREATE TABLE IF NOT EXISTS telemetry_rollups_hourly (
    tenant_id TEXT NOT NULL REFERENCES organizations(id),
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    bucket_start TIMESTAMPTZ NOT NULL,
    sample_count BIGINT NOT NULL,
    avg_temperature REAL,
    min_temperature REAL,
    max_temperature REAL,
    avg_humidity REAL,
    min_humidity REAL,
    max_humidity REAL,
    avg_battery_level REAL,
    min_battery_level REAL,
    max_battery_level REAL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, device_id, bucket_start)
);

CREATE INDEX IF NOT EXISTS idx_telemetry_rollups_hourly_tenant_bucket
    ON telemetry_rollups_hourly(tenant_id, bucket_start DESC);

CREATE INDEX IF NOT EXISTS idx_telemetry_tenant_received
    ON telemetry(tenant_id, received_at);
