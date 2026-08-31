-- New firmware uploads are stored outside PostgreSQL. Existing BYTEA rows stay
-- readable during migration and can be copied to object storage separately.

ALTER TABLE firmware_blobs
    ALTER COLUMN data DROP NOT NULL,
    ADD COLUMN storage_key TEXT,
    ADD COLUMN storage_backend TEXT NOT NULL DEFAULT 'database',
    ADD COLUMN created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ADD CONSTRAINT firmware_blobs_storage_location_check CHECK (
        (storage_backend = 'database' AND data IS NOT NULL AND storage_key IS NULL)
        OR
        (storage_backend IN ('local', 's3') AND data IS NULL AND storage_key IS NOT NULL)
    );

CREATE UNIQUE INDEX firmware_blobs_storage_key_unique
    ON firmware_blobs(storage_backend, storage_key)
    WHERE storage_key IS NOT NULL;
