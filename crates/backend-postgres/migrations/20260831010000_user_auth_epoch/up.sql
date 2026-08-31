ALTER TABLE users
    ADD COLUMN auth_epoch TEXT;

UPDATE users
SET auth_epoch = gen_random_uuid()::text
WHERE auth_epoch IS NULL;

ALTER TABLE users
    ALTER COLUMN auth_epoch SET NOT NULL,
    ALTER COLUMN auth_epoch SET DEFAULT gen_random_uuid()::text,
    ADD CONSTRAINT users_auth_epoch_key UNIQUE (auth_epoch);
