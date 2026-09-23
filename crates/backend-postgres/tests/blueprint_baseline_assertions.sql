-- Run only against a disposable database initialized from the fresh baseline.
DO $$
DECLARE
    first_epoch text;
    second_epoch text;
BEGIN
    IF EXISTS (
        SELECT FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name IN (
            'device_types', 'telemetry', 'telemetry_rollups_hourly',
            'telemetry_maintenance_state', 'device_latest_state', 'network_observed_hosts'
        )
    ) THEN RAISE EXCEPTION 'retired tables remain'; END IF;
    IF EXISTS (
        SELECT FROM information_schema.columns
        WHERE table_schema = 'public' AND (
            column_name IN ('device_type_id', 'latest_latitude', 'latest_longitude')
            OR (table_name = 'firmware_blobs' AND column_name = 'data')
        )
    ) THEN RAISE EXCEPTION 'retired columns remain'; END IF;
    IF EXISTS (SELECT FROM public.role_permissions WHERE permission LIKE 'device_types.%')
        THEN RAISE EXCEPTION 'retired permissions remain'; END IF;

    INSERT INTO public.users (username, password_hash) VALUES ('baseline-a', 'test-only')
        RETURNING auth_epoch INTO first_epoch;
    INSERT INTO public.users (username, password_hash) VALUES ('baseline-b', 'test-only')
        RETURNING auth_epoch INTO second_epoch;
    IF first_epoch = '' OR second_epoch = '' OR first_epoch = second_epoch
        THEN RAISE EXCEPTION 'authentication epochs must be distinct and nonempty'; END IF;
    BEGIN
        INSERT INTO public.users (username, password_hash, auth_epoch)
            VALUES ('baseline-duplicate', 'test-only', first_epoch);
        RAISE EXCEPTION 'duplicate epoch accepted';
    EXCEPTION WHEN unique_violation THEN NULL;
    END;
    BEGIN
        INSERT INTO public.users (username, password_hash, auth_epoch)
            VALUES ('baseline-empty', 'test-only', '');
        RAISE EXCEPTION 'empty epoch accepted';
    EXCEPTION WHEN check_violation THEN NULL;
    END;

    INSERT INTO public.organizations (id, name) VALUES ('baseline-tenant', 'Test');
    INSERT INTO public.zones (id, tenant_id, name, geometry_type, geometry_json) VALUES
        ('baseline-zone-a', 'default', 'North', 'polygon', '{}'),
        ('baseline-zone-b', 'baseline-tenant', 'North', 'polygon', '{}'),
        ('baseline-zone-c', 'default', 'north', 'polygon', '{}');
    BEGIN
        INSERT INTO public.zones (id, name, geometry_type, geometry_json)
            VALUES ('baseline-zone-duplicate', 'North', 'polygon', '{}');
        RAISE EXCEPTION 'duplicate tenant/zone name accepted';
    EXCEPTION WHEN unique_violation THEN NULL;
    END;
END $$;
