DROP TABLE network_observed_hosts;

UPDATE devices
SET device_type_id = (
        SELECT replacement.id
        FROM device_types AS replacement
        WHERE replacement.tenant_id = devices.tenant_id
          AND replacement.name = 'default'
    ),
    updated_at = CAST(unixepoch('subsec') * 1000000 AS INTEGER)
WHERE device_type_id IN (
    SELECT removed.id
    FROM device_types AS removed
    WHERE removed.name = 'network-analyzer'
);

DELETE FROM device_types
WHERE name = 'network-analyzer';
