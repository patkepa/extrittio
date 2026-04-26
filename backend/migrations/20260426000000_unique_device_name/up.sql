-- Enforce unique device names to prevent duplicates.
ALTER TABLE devices ADD CONSTRAINT devices_name_unique UNIQUE (name);
