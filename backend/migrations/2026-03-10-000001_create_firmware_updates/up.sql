CREATE TABLE firmware_updates (
    id          INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    device_type_id INTEGER NOT NULL REFERENCES device_types(id) ON DELETE CASCADE,
    version     TEXT NOT NULL,
    url         TEXT NOT NULL,
    description TEXT,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(device_type_id, version)
);

CREATE INDEX idx_firmware_updates_device_type_id ON firmware_updates(device_type_id);
