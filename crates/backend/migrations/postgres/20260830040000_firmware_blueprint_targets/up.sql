ALTER TABLE firmware_updates
    ADD COLUMN blueprint_revision_id TEXT,
    ADD COLUMN compatibility JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN update_strategy TEXT;

ALTER TABLE firmware_updates
    ADD CONSTRAINT firmware_updates_blueprint_revision_fk
    FOREIGN KEY (tenant_id, blueprint_revision_id)
    REFERENCES device_blueprint_revisions(tenant_id, id)
    NOT VALID;

ALTER TABLE firmware_updates
    VALIDATE CONSTRAINT firmware_updates_blueprint_revision_fk;

CREATE INDEX idx_firmware_updates_blueprint_revision
    ON firmware_updates (tenant_id, blueprint_revision_id, created_at DESC)
    WHERE blueprint_revision_id IS NOT NULL;
