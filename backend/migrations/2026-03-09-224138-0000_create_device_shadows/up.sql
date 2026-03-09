CREATE TABLE device_shadows (
    device_id TEXT PRIMARY KEY NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    desired TEXT NOT NULL DEFAULT '{}',
    reported TEXT NOT NULL DEFAULT '{}',
    delta TEXT NOT NULL DEFAULT '{}',
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Backfill shadows for existing devices
INSERT INTO device_shadows (device_id)
SELECT id FROM devices;
