ALTER TABLE device_types
    DROP CONSTRAINT IF EXISTS device_types_name_key;
ALTER TABLE fleets
    DROP CONSTRAINT IF EXISTS fleets_name_key;
ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_username_key;
ALTER TABLE firmware_updates
    DROP CONSTRAINT IF EXISTS firmware_updates_device_type_id_version_key;

ALTER TABLE device_types
    ADD CONSTRAINT device_types_tenant_id_name_key UNIQUE (tenant_id, name);
ALTER TABLE fleets
    ADD CONSTRAINT fleets_tenant_id_name_key UNIQUE (tenant_id, name);
ALTER TABLE users
    ADD CONSTRAINT users_tenant_id_username_key UNIQUE (tenant_id, username);
ALTER TABLE firmware_updates
    ADD CONSTRAINT firmware_updates_tenant_device_type_version_key
        UNIQUE (tenant_id, device_type_id, version);
