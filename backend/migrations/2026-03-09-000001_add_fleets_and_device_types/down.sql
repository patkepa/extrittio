CREATE TABLE devices_old (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    device_type TEXT NOT NULL DEFAULT 'default',
    status TEXT NOT NULL DEFAULT 'offline',
    firmware TEXT NOT NULL DEFAULT '',
    location TEXT NOT NULL DEFAULT '',
    last_seen TIMESTAMP,
    uptime_seconds INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO devices_old (id, name, device_type, status, firmware, location,
                         last_seen, uptime_seconds, created_at, updated_at)
SELECT d.id, d.name, COALESCE(dt.name, 'default'), d.status, d.firmware, d.location,
       d.last_seen, d.uptime_seconds, d.created_at, d.updated_at
FROM devices d
LEFT JOIN device_types dt ON dt.id = d.device_type_id;

-- Recreate telemetry to point at old devices table
CREATE TABLE telemetry_old (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    device_id TEXT NOT NULL REFERENCES devices_old(id) ON DELETE CASCADE,
    payload BLOB NOT NULL,
    temperature FLOAT,
    humidity FLOAT,
    battery_level FLOAT,
    custom_json TEXT,
    received_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO telemetry_old SELECT * FROM telemetry;
DROP TABLE telemetry;
DROP TABLE devices;

ALTER TABLE devices_old RENAME TO devices;
ALTER TABLE telemetry_old RENAME TO telemetry;

CREATE INDEX idx_telemetry_device_id ON telemetry(device_id);
CREATE INDEX idx_telemetry_received_at ON telemetry(received_at);

DROP TABLE fleets;
DROP TABLE device_types;
