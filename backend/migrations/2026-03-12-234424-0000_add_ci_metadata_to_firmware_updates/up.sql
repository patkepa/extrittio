ALTER TABLE firmware_updates ADD COLUMN commit_sha TEXT;
ALTER TABLE firmware_updates ADD COLUMN branch TEXT;
ALTER TABLE firmware_updates ADD COLUMN ci_run_url TEXT;
ALTER TABLE firmware_updates ADD COLUMN build_timestamp TIMESTAMP;
ALTER TABLE firmware_updates ADD COLUMN changelog TEXT;
ALTER TABLE firmware_updates ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
