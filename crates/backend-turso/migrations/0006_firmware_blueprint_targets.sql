ALTER TABLE firmware_updates ADD COLUMN blueprint_revision_id TEXT;
ALTER TABLE firmware_updates ADD COLUMN compatibility TEXT NOT NULL DEFAULT '{}';
ALTER TABLE firmware_updates ADD COLUMN update_strategy TEXT;

CREATE INDEX idx_firmware_updates_blueprint_revision
    ON firmware_updates (tenant_id, blueprint_revision_id, created_at DESC)
    WHERE blueprint_revision_id IS NOT NULL;
