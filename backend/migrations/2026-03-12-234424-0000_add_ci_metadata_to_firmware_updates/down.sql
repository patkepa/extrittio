CREATE TABLE firmware_updates_backup (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    device_type_id INTEGER NOT NULL REFERENCES device_types(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    url TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    sha256 TEXT,
    UNIQUE(device_type_id, version)
);

INSERT INTO firmware_updates_backup (id, device_type_id, version, url, description, created_at, sha256)
SELECT id, device_type_id, version, url, description, created_at, sha256
FROM firmware_updates;

DROP TABLE firmware_updates;

ALTER TABLE firmware_updates_backup RENAME TO firmware_updates;

CREATE INDEX idx_firmware_updates_device_type_id ON firmware_updates(device_type_id);
