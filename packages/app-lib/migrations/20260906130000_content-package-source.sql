ALTER TABLE instance_content_entries
	ADD COLUMN source TEXT NOT NULL DEFAULT 'modrinth';
ALTER TABLE instance_content_entries ADD COLUMN cf_project_id INTEGER;
ALTER TABLE instance_content_entries ADD COLUMN cf_version_id INTEGER;
ALTER TABLE instances
	ADD COLUMN preferred_source TEXT NOT NULL DEFAULT 'modrinth';
