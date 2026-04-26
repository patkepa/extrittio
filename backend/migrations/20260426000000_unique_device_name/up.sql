-- Rename duplicate device names before adding the unique constraint.
-- For each set of duplicates, keep the oldest row's name unchanged and
-- append '-2', '-3', etc. to the newer rows (ordered by created_at).
WITH ranked AS (
    SELECT id,
           name,
           ROW_NUMBER() OVER (PARTITION BY name ORDER BY created_at) AS rn
    FROM devices
)
UPDATE devices
SET name = ranked.name || '-' || ranked.rn
FROM ranked
WHERE devices.id = ranked.id
  AND ranked.rn > 1;

-- Enforce unique device names to prevent duplicates.
ALTER TABLE devices ADD CONSTRAINT devices_name_unique UNIQUE (name);
