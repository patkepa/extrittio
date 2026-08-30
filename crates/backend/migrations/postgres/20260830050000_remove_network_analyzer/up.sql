DROP TABLE network_observed_hosts;

UPDATE devices AS device
SET device_type_id = replacement.id,
    updated_at = NOW()
FROM device_types AS removed,
     device_types AS replacement
WHERE device.device_type_id = removed.id
  AND removed.name = 'network-analyzer'
  AND replacement.tenant_id = device.tenant_id
  AND replacement.name = 'default';

DELETE FROM device_types
WHERE name = 'network-analyzer';
