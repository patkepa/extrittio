DROP FUNCTION IF EXISTS drop_telemetry_partitions_older_than(TIMESTAMPTZ);
DROP FUNCTION IF EXISTS ensure_telemetry_partitions(INTEGER);

ALTER TABLE telemetry RENAME TO telemetry_partitioned;

CREATE TABLE telemetry (
    id BIGINT NOT NULL DEFAULT nextval('telemetry_id_seq'),
    device_id TEXT NOT NULL,
    payload BYTEA NOT NULL,
    temperature REAL,
    humidity REAL,
    battery_level REAL,
    custom_json JSONB,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    speed REAL,
    altitude REAL,
    heading REAL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    tenant_id TEXT NOT NULL DEFAULT 'default'
);

INSERT INTO telemetry SELECT * FROM telemetry_partitioned;

ALTER SEQUENCE telemetry_id_seq OWNED BY NONE;
DROP TABLE telemetry_partitioned CASCADE;
ALTER SEQUENCE telemetry_id_seq OWNED BY telemetry.id;

ALTER TABLE telemetry ADD PRIMARY KEY (id);
ALTER TABLE telemetry
    ADD CONSTRAINT telemetry_device_fk
        FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE,
    ADD CONSTRAINT telemetry_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES organizations(id),
    ADD CONSTRAINT telemetry_tenant_device_fk
        FOREIGN KEY (tenant_id, device_id)
        REFERENCES devices(tenant_id, id) ON DELETE CASCADE;

CREATE INDEX idx_telemetry_device_received
    ON telemetry(device_id, received_at DESC);
CREATE INDEX idx_telemetry_tenant_device_received
    ON telemetry(tenant_id, device_id, received_at DESC);
CREATE INDEX idx_telemetry_tenant_received
    ON telemetry(tenant_id, received_at DESC);
CREATE INDEX idx_telemetry_received_brin
    ON telemetry USING BRIN(received_at) WITH (pages_per_range = 32);

DROP TABLE device_latest_state;
