-- Keep raw telemetry scalable and make the current device state independent of
-- raw-data retention. The migration preserves existing IDs and timestamps.

CREATE TABLE device_latest_state (
    tenant_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    telemetry_id BIGINT NOT NULL,
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
    received_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, device_id),
    CONSTRAINT device_latest_state_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES organizations(id) ON DELETE CASCADE,
    CONSTRAINT device_latest_state_tenant_device_fk
        FOREIGN KEY (tenant_id, device_id)
        REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);

INSERT INTO device_latest_state (
    tenant_id, device_id, telemetry_id, payload, temperature, humidity,
    battery_level, custom_json, latitude, longitude, speed, altitude, heading,
    received_at
)
SELECT DISTINCT ON (tenant_id, device_id)
    tenant_id, device_id, id, payload, temperature, humidity, battery_level,
    custom_json, latitude, longitude, speed, altitude, heading, received_at
FROM telemetry
ORDER BY tenant_id, device_id, received_at DESC, id DESC;

ALTER TABLE telemetry RENAME TO telemetry_unpartitioned;
ALTER TABLE telemetry_unpartitioned
    RENAME CONSTRAINT telemetry_pkey TO telemetry_unpartitioned_pkey;

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
    tenant_id TEXT NOT NULL DEFAULT 'default',
    PRIMARY KEY (id, received_at),
    CONSTRAINT telemetry_device_fk
        FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE,
    CONSTRAINT telemetry_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES organizations(id),
    CONSTRAINT telemetry_tenant_device_fk
        FOREIGN KEY (tenant_id, device_id)
        REFERENCES devices(tenant_id, id) ON DELETE CASCADE
) PARTITION BY RANGE (received_at);

DO $$
DECLARE
    partition_start DATE;
    partition_end DATE;
    cursor_month DATE;
    partition_name TEXT;
BEGIN
    SELECT COALESCE(
        date_trunc('month', min(received_at))::date,
        date_trunc('month', now())::date
    ) INTO partition_start
    FROM telemetry_unpartitioned;

    partition_end := (date_trunc('month', now()) + interval '4 months')::date;
    cursor_month := partition_start;

    WHILE cursor_month < partition_end LOOP
        partition_name := 'telemetry_y' || to_char(cursor_month, 'YYYYMM');
        EXECUTE format(
            'CREATE TABLE %I PARTITION OF telemetry FOR VALUES FROM (%L) TO (%L)',
            partition_name,
            cursor_month,
            cursor_month + interval '1 month'
        );
        cursor_month := (cursor_month + interval '1 month')::date;
    END LOOP;
END
$$;

CREATE TABLE telemetry_default PARTITION OF telemetry DEFAULT;

INSERT INTO telemetry
SELECT * FROM telemetry_unpartitioned;

ALTER SEQUENCE telemetry_id_seq OWNED BY NONE;
DROP TABLE telemetry_unpartitioned;
ALTER SEQUENCE telemetry_id_seq OWNED BY telemetry.id;

CREATE INDEX idx_telemetry_device_received
    ON telemetry(device_id, received_at DESC);
CREATE INDEX idx_telemetry_tenant_device_received
    ON telemetry(tenant_id, device_id, received_at DESC);
CREATE INDEX idx_telemetry_tenant_received
    ON telemetry(tenant_id, received_at DESC);
CREATE INDEX idx_telemetry_received_brin
    ON telemetry USING BRIN(received_at) WITH (pages_per_range = 32);

-- Create missing future partitions. Rows that arrived in the default partition
-- during an extended scheduler outage are moved into the new partition safely.
CREATE OR REPLACE FUNCTION ensure_telemetry_partitions(months_ahead INTEGER DEFAULT 3)
RETURNS INTEGER
LANGUAGE plpgsql
AS $$
DECLARE
    cursor_month DATE := date_trunc('month', now())::date;
    final_month DATE := (date_trunc('month', now()) + make_interval(months => months_ahead))::date;
    next_month DATE;
    partition_name TEXT;
    created_count INTEGER := 0;
BEGIN
    IF months_ahead < 1 OR months_ahead > 24 THEN
        RAISE EXCEPTION 'months_ahead must be between 1 and 24';
    END IF;

    CREATE TEMP TABLE IF NOT EXISTS telemetry_partition_buffer
        ON COMMIT DROP AS SELECT * FROM telemetry WITH NO DATA;

    WHILE cursor_month <= final_month LOOP
        next_month := (cursor_month + interval '1 month')::date;
        partition_name := 'telemetry_y' || to_char(cursor_month, 'YYYYMM');

        IF to_regclass(partition_name) IS NULL THEN
            TRUNCATE telemetry_partition_buffer;
            EXECUTE
                'WITH moved AS (
                    DELETE FROM telemetry_default
                    WHERE received_at >= $1 AND received_at < $2
                    RETURNING *
                )
                INSERT INTO telemetry_partition_buffer SELECT * FROM moved'
            USING cursor_month, next_month;

            EXECUTE format(
                'CREATE TABLE %I PARTITION OF telemetry FOR VALUES FROM (%L) TO (%L)',
                partition_name,
                cursor_month,
                next_month
            );
            INSERT INTO telemetry SELECT * FROM telemetry_partition_buffer;
            created_count := created_count + 1;
        END IF;

        cursor_month := next_month;
    END LOOP;

    RETURN created_count;
END
$$;

CREATE OR REPLACE FUNCTION drop_telemetry_partitions_older_than(cutoff TIMESTAMPTZ)
RETURNS INTEGER
LANGUAGE plpgsql
AS $$
DECLARE
    partition_record RECORD;
    partition_month DATE;
    dropped_count INTEGER := 0;
BEGIN
    FOR partition_record IN
        SELECT child.relname AS partition_name
        FROM pg_inherits
        JOIN pg_class parent ON pg_inherits.inhparent = parent.oid
        JOIN pg_class child ON pg_inherits.inhrelid = child.oid
        JOIN pg_namespace namespace ON child.relnamespace = namespace.oid
        WHERE parent.oid = 'telemetry'::regclass
          AND namespace.nspname = current_schema()
          AND child.relname ~ '^telemetry_y[0-9]{6}$'
    LOOP
        partition_month := to_date(substring(partition_record.partition_name FROM 12), 'YYYYMM');
        IF partition_month + interval '1 month' <= cutoff THEN
            EXECUTE format('DROP TABLE %I', partition_record.partition_name);
            dropped_count := dropped_count + 1;
        END IF;
    END LOOP;

    RETURN dropped_count;
END
$$;
