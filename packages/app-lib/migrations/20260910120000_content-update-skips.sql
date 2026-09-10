ALTER TABLE instance_content_update_checks ADD COLUMN skipped_version_id TEXT NULL;

ALTER TABLE instance_content_entries ADD COLUMN updates_ignored INTEGER NOT NULL DEFAULT 0;
