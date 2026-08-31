DROP INDEX IF EXISTS idx_devices_declared_connections_gin;

ALTER TABLE devices
DROP COLUMN declared_connections;

DELETE FROM device_types
WHERE name = 'network-analyzer'
  AND NOT EXISTS (
      SELECT 1
      FROM devices
      WHERE devices.device_type_id = device_types.id
  );
