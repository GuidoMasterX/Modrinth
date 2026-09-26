use std::collections::{HashMap, HashSet};

use crate::state::instances::adapters::sqlite::content_rows;
use crate::state::{CacheBehaviour, CachedEntry, ProjectType, State};

use super::apply_content_install::{
	ContentScope, add_project_from_curseforge_file, add_project_from_version,
	normalized_identity_key_pub, remove_project, resolve_content_scope, toggle_disable_project,
};
use crate::state::instances::ContentSourceKind;
use crate::util::fetch::DownloadReason;

#[derive(Default, Clone, Debug)]
pub(crate) struct InstalledCrossSource {
	pub mr_project_ids: HashSet<String>,
	pub cf_project_ids: HashSet<i64>,
	pub identity_keys: HashSet<(ProjectType, String)>,
}

impl InstalledCrossSource {
	pub fn is_empty(&self) -> bool {
		self.mr_project_ids.is_empty()
			&& self.cf_project_ids.is_empty()
			&& self.identity_keys.is_empty()
	}
}

#[derive(Default, Clone, Debug)]
pub(crate) struct FileCounterparts {
	pub mr: Option<(String, String)>,
	pub cf: Option<(i64, i64)>,
}

fn insert_identity_keys(
	keys: &mut HashSet<(ProjectType, String)>,
	project_type: ProjectType,
	slug: Option<&str>,
	title: &str,
) {
	if let Some(slug) = slug {
		let key = normalized_identity_key_pub(slug);
		if !key.is_empty() {
			keys.insert((project_type, key));
		}
	}
	let key = normalized_identity_key_pub(title);
	if !key.is_empty() {
		keys.insert((project_type, key));
	}
}

/// Whether a project on either source is already present in the instance.
///
/// Exact source ids are authoritative; the normalized title/slug fallback is
/// scoped by project type so unrelated projects sharing a generic name cannot
/// match.
pub(crate) fn project_already_installed(
	project_type: ProjectType,
	mr_project_id: Option<&str>,
	cf_project_id: Option<i64>,
	slug: Option<&str>,
	title: &str,
	installed: &InstalledCrossSource,
) -> bool {
	if mr_project_id.is_some_and(|id| !id.is_empty() && installed.mr_project_ids.contains(id)) {
		return true;
	}
	if cf_project_id.is_some_and(|id| installed.cf_project_ids.contains(&id)) {
		return true;
	}
	if let Some(slug) = slug {
		let key = normalized_identity_key_pub(slug);
		if !key.is_empty() && installed.identity_keys.contains(&(project_type, key)) {
			return true;
		}
	}
	let key = normalized_identity_key_pub(title);
	!key.is_empty() && installed.identity_keys.contains(&(project_type, key))
}

/// Collect the source ids and identity keys of everything installed in a
/// content set, including the cross-source counterparts learned from installed
/// file hashes. Best effort: failures are logged and degrade to an empty set
/// rather than aborting an install.
pub(crate) async fn installed_cross_source(
	instance_id: &str,
	content_set_id: &str,
	state: &State,
) -> InstalledCrossSource {
	let mut installed = InstalledCrossSource::default();

	let entries = match content_rows::get_content_entries(content_set_id, &state.pool).await {
		Ok(entries) => entries,
		Err(error) => {
			tracing::warn!("Unable to read content entries for {content_set_id}: {error}");
			return installed;
		}
	};

	for entry in &entries {
		if let Some(project_id) = entry.project_id.as_deref().filter(|id| !id.is_empty()) {
			installed.mr_project_ids.insert(project_id.to_string());
		}
		if let Some(cf_project_id) = entry.cf_project_id {
			installed.cf_project_ids.insert(cf_project_id);
		}
	}

	let cf_ids: Vec<String> = installed
		.cf_project_ids
		.iter()
		.map(|id| id.to_string())
		.collect();
	if !cf_ids.is_empty() {
		let refs: Vec<&str> = cf_ids.iter().map(String::as_str).collect();
		match CachedEntry::get_curseforge_project_many(
			&refs,
			None,
			&state.pool,
			&state.api_semaphore,
		)
		.await
		{
			Ok(projects) => {
				let by_id: HashMap<String, &crate::api::curseforge::normalize::SourceProject> =
					projects
						.iter()
						.map(|project| (project.id.clone(), project))
						.collect();
				for entry in &entries {
					if let Some(cf_project_id) = entry.cf_project_id
						&& let Some(project) = by_id.get(&cf_project_id.to_string())
					{
						insert_identity_keys(
							&mut installed.identity_keys,
							entry.project_type,
							project.slug.as_deref(),
							&project.title,
						);
					}
				}
			}
			Err(error) => tracing::warn!("Unable to resolve installed CurseForge projects: {error}"),
		}
	}

	let mr_ids: Vec<String> = installed.mr_project_ids.iter().cloned().collect();
	if !mr_ids.is_empty() {
		let refs: Vec<&str> = mr_ids.iter().map(String::as_str).collect();
		match CachedEntry::get_project_many(&refs, None, &state.pool, &state.api_semaphore).await {
			Ok(projects) => {
				let by_id: HashMap<&str, &crate::state::Project> = projects
					.iter()
					.map(|project| (project.id.as_str(), project))
					.collect();
				for entry in &entries {
					if let Some(project_id) = entry.project_id.as_deref().filter(|id| !id.is_empty())
						&& let Some(project) = by_id.get(project_id)
					{
						insert_identity_keys(
							&mut installed.identity_keys,
							entry.project_type,
							project.slug.as_deref(),
							&project.title,
						);
					}
				}
			}
			Err(error) => tracing::warn!("Unable to resolve installed Modrinth projects: {error}"),
		}
	}

	let files = match content_rows::get_instance_files(instance_id, &state.pool).await {
		Ok(files) => files,
		Err(error) => {
			tracing::warn!("Unable to read instance files for {instance_id}: {error}");
			return installed;
		}
	};

	let sha1s: Vec<String> = files
		.iter()
		.filter(|file| !file.missing && !file.sha1.is_empty())
		.map(|file| file.sha1.clone())
		.collect();
	let mut project_by_hash: HashMap<String, String> = HashMap::new();
	if !sha1s.is_empty() {
		let refs: Vec<&str> = sha1s.iter().map(String::as_str).collect();
		match CachedEntry::get_file_many(&refs, None, &state.pool, &state.api_semaphore).await {
			Ok(cached_files) => {
				for cached in cached_files {
					if !cached.project_id.is_empty() {
						installed.mr_project_ids.insert(cached.project_id.clone());
						project_by_hash.insert(cached.hash, cached.project_id);
					}
				}
			}
			Err(error) => tracing::warn!("Unable to resolve installed file hashes: {error}"),
		}
	}

	let entries_by_file: HashMap<&str, (Option<i64>, ProjectType)> = entries
		.iter()
		.filter_map(|entry| {
			entry
				.file_id
				.as_deref()
				.map(|file_id| (file_id, (entry.cf_project_id, entry.project_type)))
		})
		.collect();
	let mut learned: Vec<(String, i64, &'static str)> = Vec::new();
	for file in &files {
		if file.missing {
			continue;
		}
		let Some((Some(cf_project_id), project_type)) = entries_by_file.get(file.id.as_str()) else {
			continue;
		};
		let Some(mr_project_id) = project_by_hash.get(&file.sha1) else {
			continue;
		};
		learned.push((
			mr_project_id.clone(),
			*cf_project_id,
			project_type.get_name(),
		));
	}
	for (mr_project_id, cf_project_id, project_type) in learned {
		record_source_link(&mr_project_id, cf_project_id, project_type, state).await;
		installed.mr_project_ids.insert(mr_project_id);
		installed.cf_project_ids.insert(cf_project_id);
	}

	if let Ok(links) = read_source_links(state).await {
		for (mr_project_id, cf_project_id) in links {
			if installed.mr_project_ids.contains(&mr_project_id) {
				installed.cf_project_ids.insert(cf_project_id);
			}
			if installed.cf_project_ids.contains(&cf_project_id) {
				installed.mr_project_ids.insert(mr_project_id);
			}
		}
	}

	installed
}

/// Exact-file counterparts of an installed file. A file is available on both
/// sources when both `mr` and `cf` are populated.
pub(crate) async fn resolve_file_counterparts(
	scope: &ContentScope,
	relative_path: &str,
	sha1: &str,
	state: &State,
) -> FileCounterparts {
	let mut counterparts = FileCounterparts::default();

	if !sha1.is_empty() {
		match CachedEntry::get_file_many(
			&[sha1],
			Some(CacheBehaviour::MustRevalidate),
			&state.pool,
			&state.api_semaphore,
		)
		.await
		{
			Ok(files) => {
				if let Some(file) = files.into_iter().next()
					&& !file.project_id.is_empty()
				{
					counterparts.mr = Some((file.project_id, file.version_id));
				}
			}
			Err(error) => tracing::warn!("Unable to resolve Modrinth file for {sha1}: {error}"),
		}
	}

	let full_path = state
		.directories
		.instances_dir()
		.join(&scope.instance.path)
		.join(relative_path);
	match tokio::fs::read(&full_path).await {
		Ok(bytes) => {
			let fingerprint = crate::util::murmur2::murmur2(&bytes) as i64;
			match CachedEntry::get_curseforge_fingerprints(
				&fingerprint.to_string(),
				Some(CacheBehaviour::MustRevalidate),
				&state.pool,
				&state.api_semaphore,
			)
			.await
			{
				Ok(Some(cached)) => {
					if let Some(exact) = cached.data.exact_matches.first() {
						counterparts.cf = Some((exact.id, exact.file.id));
					}
				}
				Ok(None) => {}
				Err(error) => tracing::warn!(
					"Unable to resolve CurseForge fingerprint for {relative_path}: {error}"
				),
			}
		}
		Err(error) => {
			tracing::warn!("Unable to read {full_path:?} for fingerprinting: {error}")
		}
	}

	counterparts
}

/// Switch an installed project to its exact-version counterpart on the other
/// source. Fails when the identical file is not available on both sources.
pub(crate) async fn switch_project_source(
	instance_id: &str,
	project_path: &str,
	state: &State,
) -> crate::Result<String> {
	let scope = resolve_content_scope(instance_id, None, state).await?;
	let file =
		content_rows::get_instance_file_by_relative_path(instance_id, project_path, &state.pool)
			.await?
			.ok_or_else(|| {
				crate::ErrorKind::InputError(format!("No file found at {project_path}"))
			})?;
	let entry =
		content_rows::get_content_entry_by_file(&scope.content_set_id, &file.id, &state.pool)
			.await?
			.ok_or_else(|| {
				crate::ErrorKind::InputError(
					"This project is not managed by the launcher".to_string(),
				)
			})?;

	let switching_to_curseforge = entry.project_id.as_deref().is_some_and(|id| !id.is_empty());
	let counterparts = resolve_file_counterparts(&scope, project_path, &file.sha1, state).await;
	let was_disabled = !file.enabled || project_path.ends_with(".disabled");

	let mr_link = counterparts.mr.as_ref();
	let cf_link = counterparts.cf;

	let new_path = if switching_to_curseforge {
		let Some((cf_project_id, cf_file_id)) = cf_link else {
			return Err(crate::ErrorKind::InputError(
				"This version is not available on CurseForge".to_string(),
			)
			.into());
		};
		add_project_from_curseforge_file(
			instance_id,
			cf_project_id,
			cf_file_id,
			DownloadReason::Update,
			state,
		)
		.await?
	} else {
		let Some((_project_id, version_id)) = mr_link else {
			return Err(crate::ErrorKind::InputError(
				"This version is not available on Modrinth".to_string(),
			)
			.into());
		};
		add_project_from_version(
			instance_id,
			version_id,
			DownloadReason::Update,
			None,
			ContentSourceKind::Local,
			state,
		)
		.await?
	};

	if was_disabled {
		toggle_disable_project(instance_id, &new_path, Some(false), state).await?;
	}
	if new_path != project_path {
		remove_project(instance_id, project_path, state).await?;
	}

	if let (Some((mr_project_id, _)), Some((cf_project_id, _))) = (mr_link, cf_link) {
		record_source_link(
			mr_project_id,
			cf_project_id,
			entry.project_type.get_name(),
			state,
		)
		.await;
	}

	Ok(new_path)
}

async fn record_source_link(
	mr_project_id: &str,
	cf_project_id: i64,
	project_type: &str,
	state: &State,
) {
	let result = sqlx::query(
		"INSERT OR IGNORE INTO project_source_links \
		 (mr_project_id, cf_project_id, project_type, created_at) VALUES (?, ?, ?, ?)",
	)
	.bind(mr_project_id)
	.bind(cf_project_id)
	.bind(project_type)
	.bind(chrono::Utc::now().timestamp_millis())
	.execute(&state.pool)
	.await;
	if let Err(error) = result {
		tracing::warn!("Unable to record source link {mr_project_id} <-> {cf_project_id}: {error}");
	}
}

async fn read_source_links(state: &State) -> crate::Result<Vec<(String, i64)>> {
	Ok(sqlx::query_as::<_, (String, i64)>(
		"SELECT mr_project_id, cf_project_id FROM project_source_links",
	)
	.fetch_all(&state.pool)
	.await?)
}
