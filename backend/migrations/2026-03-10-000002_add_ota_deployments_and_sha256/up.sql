-- Add sha256 hash column to firmware_updates for binary verification
ALTER TABLE firmware_updates ADD COLUMN sha256 TEXT;

-- OTA deployment tracking: records every OTA push to a device
CREATE TABLE ota_deployments (
    id              INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    device_id       TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    firmware_update_id INTEGER NOT NULL REFERENCES firmware_updates(id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'pending',
    error_message   TEXT,
    initiated_at    TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at    TIMESTAMP
);

CREATE INDEX idx_ota_deployments_device_id ON ota_deployments(device_id);
CREATE INDEX idx_ota_deployments_firmware_update_id ON ota_deployments(firmware_update_id);
