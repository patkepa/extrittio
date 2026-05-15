DELETE FROM device_types
WHERE tenant_id = 'default'
  AND name = 'OrganBath'
  AND NOT EXISTS (
      SELECT 1
      FROM devices
      WHERE devices.device_type_id = device_types.id
  );
