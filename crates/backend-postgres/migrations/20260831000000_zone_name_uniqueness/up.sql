DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM zones
         GROUP BY tenant_id COLLATE "C", name COLLATE "C"
        HAVING count(*) > 1
    ) THEN
        RAISE EXCEPTION USING
            MESSAGE = 'cannot enforce zone name uniqueness: duplicate (tenant_id, name) rows exist',
            HINT = 'Rename or merge duplicate zones, then rerun migrations.';
    END IF;
END
$$;

CREATE UNIQUE INDEX zones_tenant_name_unique
    ON zones (tenant_id COLLATE "C", name COLLATE "C");
