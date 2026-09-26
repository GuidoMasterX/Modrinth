CREATE TABLE IF NOT EXISTS project_source_links (
	mr_project_id TEXT NOT NULL,
	cf_project_id INTEGER NOT NULL,
	project_type TEXT NOT NULL,
	created_at INTEGER NOT NULL,
	PRIMARY KEY (mr_project_id, cf_project_id)
);

CREATE INDEX IF NOT EXISTS idx_project_source_links_cf ON project_source_links (cf_project_id);
