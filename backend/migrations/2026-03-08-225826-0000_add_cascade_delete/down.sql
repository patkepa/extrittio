CREATE TABLE telemetry_old (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    device_id TEXT NOT NULL REFERENCES devices(id),
    payload BLOB NOT NULL,
    temperature REAL,
    humidity REAL,
    battery_level REAL,
    custom_json TEXT,
    received_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO telemetry_old SELECT * FROM telemetry;
DROP TABLE telemetry;
ALTER TABLE telemetry_old RENAME TO telemetry;

CREATE INDEX idx_telemetry_device_id ON telemetry(device_id);
CREATE INDEX idx_telemetry_received_at ON telemetry(received_at);
