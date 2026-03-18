PRAGMA foreign_keys = OFF;

CREATE TABLE telemetry_backup (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    payload BLOB NOT NULL,
    temperature FLOAT,
    humidity FLOAT,
    battery_level FLOAT,
    custom_json TEXT,
    received_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO telemetry_backup (id, device_id, payload, temperature, humidity, battery_level, custom_json, received_at)
SELECT id, device_id, payload, temperature, humidity, battery_level, custom_json, received_at FROM telemetry;

DROP TABLE telemetry;
ALTER TABLE telemetry_backup RENAME TO telemetry;

CREATE INDEX idx_telemetry_device_received ON telemetry(device_id, received_at);

PRAGMA foreign_keys = ON;
