DROP INDEX IF EXISTS idx_firmware_updates_blueprint_revision;

ALTER TABLE firmware_updates
    DROP CONSTRAINT IF EXISTS firmware_updates_blueprint_revision_fk,
    DROP COLUMN IF EXISTS update_strategy,
    DROP COLUMN IF EXISTS compatibility,
    DROP COLUMN IF EXISTS blueprint_revision_id;
