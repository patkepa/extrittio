DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM firmware_blobs WHERE storage_backend <> 'database'
    ) THEN
        RAISE EXCEPTION
            'cannot roll back firmware object storage while external firmware objects exist';
    END IF;
END
$$;

DROP INDEX IF EXISTS firmware_blobs_storage_key_unique;

ALTER TABLE firmware_blobs
    DROP CONSTRAINT firmware_blobs_storage_location_check,
    DROP COLUMN created_at,
    DROP COLUMN storage_backend,
    DROP COLUMN storage_key,
    ALTER COLUMN data SET NOT NULL;
