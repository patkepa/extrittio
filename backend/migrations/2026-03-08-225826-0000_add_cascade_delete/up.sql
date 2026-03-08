-- SQLite doesn't support ALTER FOREIGN KEY, so we need to recreate the table
CREATE TABLE telemetry_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    payload BLOB NOT NULL,
    temperature REAL,
    humidity REAL,
    battery_level REAL,
    custom_json TEXT,
    received_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO telemetry_new SELECT * FROM telemetry;
DROP TABLE telemetry;
ALTER TABLE telemetry_new RENAME TO telemetry;

CREATE INDEX idx_telemetry_device_id ON telemetry(device_id);
CREATE INDEX idx_telemetry_received_at ON telemetry(received_at);
