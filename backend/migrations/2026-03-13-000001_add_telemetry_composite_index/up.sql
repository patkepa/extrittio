-- Replace the two separate indexes with a composite index that covers
-- the common query pattern: WHERE device_id = ? AND received_at > ? ORDER BY received_at DESC
DROP INDEX IF EXISTS idx_telemetry_device_id;
DROP INDEX IF EXISTS idx_telemetry_received_at;
CREATE INDEX idx_telemetry_device_received ON telemetry(device_id, received_at);
