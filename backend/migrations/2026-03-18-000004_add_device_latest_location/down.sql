PRAGMA foreign_keys = OFF;

CREATE TABLE devices_backup (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    device_type_id INTEGER NOT NULL REFERENCES device_types(id),
    fleet_id INTEGER REFERENCES fleets(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'offline',
    firmware TEXT NOT NULL DEFAULT '',
    last_seen TIMESTAMP,
    uptime_seconds INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO devices_backup (id, name, device_type_id, fleet_id, status, firmware, last_seen, uptime_seconds, created_at, updated_at)
SELECT id, name, device_type_id, fleet_id, status, firmware, last_seen, uptime_seconds, created_at, updated_at FROM devices;

DROP TABLE devices;
ALTER TABLE devices_backup RENAME TO devices;

CREATE INDEX idx_devices_device_type_id ON devices(device_type_id);
CREATE INDEX idx_devices_fleet_id ON devices(fleet_id);

PRAGMA foreign_keys = ON;
