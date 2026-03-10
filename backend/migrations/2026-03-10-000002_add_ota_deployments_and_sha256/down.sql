DROP TABLE IF EXISTS ota_deployments;

-- SQLite does not support DROP COLUMN; recreate the table without sha256
CREATE TABLE firmware_updates_backup AS SELECT id, device_type_id, version, url, description, created_at FROM firmware_updates;
DROP TABLE firmware_updates;
CREATE TABLE firmware_updates (
    id          INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    device_type_id INTEGER NOT NULL REFERENCES device_types(id) ON DELETE CASCADE,
    version     TEXT NOT NULL,
    url         TEXT NOT NULL,
    description TEXT,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(device_type_id, version)
);
INSERT INTO firmware_updates SELECT * FROM firmware_updates_backup;
DROP TABLE firmware_updates_backup;
CREATE INDEX idx_firmware_updates_device_type_id ON firmware_updates(device_type_id);
