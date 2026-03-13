DROP INDEX IF EXISTS idx_telemetry_device_received;
CREATE INDEX idx_telemetry_device_id ON telemetry(device_id);
CREATE INDEX idx_telemetry_received_at ON telemetry(received_at);
