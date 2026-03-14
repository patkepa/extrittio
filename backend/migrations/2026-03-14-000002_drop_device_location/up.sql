-- SQLite does not support DROP COLUMN before 3.35.0, so we recreate the table.
PRAGMA foreign_keys = OFF;

CREATE TABLE devices_new (
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

INSERT INTO devices_new (id, name, device_type_id, fleet_id, status, firmware, last_seen, uptime_seconds, created_at, updated_at)
    SELECT id, name, device_type_id, fleet_id, status, firmware, last_seen, uptime_seconds, created_at, updated_at
    FROM devices;

DROP TABLE devices;
ALTER TABLE devices_new RENAME TO devices;

PRAGMA foreign_keys = ON;
