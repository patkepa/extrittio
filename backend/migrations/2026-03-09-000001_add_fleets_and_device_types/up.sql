-- 1. Create lookup tables
CREATE TABLE device_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE fleets (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 2. Seed the mandatory default device type
INSERT INTO device_types (name) VALUES ('default');

-- 3. Recreate devices with FK columns (SQLite cannot ALTER to add FK constraints)
CREATE TABLE devices_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    device_type_id INTEGER NOT NULL DEFAULT 1 REFERENCES device_types(id),
    fleet_id INTEGER REFERENCES fleets(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'offline',
    firmware TEXT NOT NULL DEFAULT '',
    location TEXT NOT NULL DEFAULT '',
    last_seen TIMESTAMP,
    uptime_seconds INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO devices_new (id, name, device_type_id, fleet_id, status, firmware,
                         location, last_seen, uptime_seconds, created_at, updated_at)
SELECT id, name, 1, NULL, status, firmware,
       location, last_seen, uptime_seconds, created_at, updated_at
FROM devices;

DROP TABLE devices;
ALTER TABLE devices_new RENAME TO devices;

-- 4. Recreate telemetry to preserve FK to devices with cascade delete
CREATE TABLE telemetry_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    payload BLOB NOT NULL,
    temperature FLOAT,
    humidity FLOAT,
    battery_level FLOAT,
    custom_json TEXT,
    received_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO telemetry_new SELECT * FROM telemetry;
DROP TABLE telemetry;
ALTER TABLE telemetry_new RENAME TO telemetry;

-- 5. Restore indexes
CREATE INDEX idx_devices_device_type_id ON devices(device_type_id);
CREATE INDEX idx_devices_fleet_id ON devices(fleet_id);
CREATE INDEX idx_telemetry_device_id ON telemetry(device_id);
CREATE INDEX idx_telemetry_received_at ON telemetry(received_at);
