ALTER TABLE devices
ADD COLUMN IF NOT EXISTS declared_connections JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE INDEX IF NOT EXISTS idx_devices_declared_connections_gin
ON devices USING GIN (declared_connections);

INSERT INTO device_types (name)
VALUES ('network-analyzer')
ON CONFLICT (name) DO NOTHING;
