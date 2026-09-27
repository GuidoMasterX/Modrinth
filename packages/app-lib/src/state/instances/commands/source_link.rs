use std::collections::{HashMap, HashSet};

use crate::api::instance::{InstalledSource, SourceCounterpart};
use crate::state::instances::adapters::sqlite::content_rows;
use crate::state::instances::{ContentEntry, InstanceFile};
use crate::state::{CacheBehaviour, CachedEntry, ProjectType, State};

use super::apply_content_install::{
	ContentScope, add_project_from_curseforge_file, add_project_from_version,
	normalized_identity_key_pub, resolve_content_scope,
};
use super::content_mutation::{remove_project, toggle_disable_project};
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

/// Resolve the installed file's tracked entry and its exact counterparts.
/// Shared by [`get_source_counterpart`] and [`switch_project_source`] so both
/// agree on what "available on the other source" means.
async fn resolve_counterpart_parts(
	instance_id: &str,
	file_path: &str,
	state: &State,
) -> crate::Result<(ContentScope, InstanceFile, Option<ContentEntry>, FileCounterparts)> {
	let scope = resolve_content_scope(instance_id, None, state).await?;
	let file = content_rows::get_instance_file_by_relative_path(
		instance_id,
		file_path,
		&state.pool,
	)
	.await?
	.ok_or_else(|| {
		crate::ErrorKind::InputError(format!("No file found at {file_path}"))
	})?;
	let entry = content_rows::get_content_entry_by_file(
		&scope.content_set_id,
		&file.id,
		&state.pool,
	)
	.await?;
	let counterparts = resolve_file_counterparts(
		&scope,
		file_path,
		&file.sha1,
		entry.as_ref(),
		state,
	)
	.await;
	Ok((scope, file, entry, counterparts))
}

fn counterpart_source(
	entry: Option<&ContentEntry>,
	counterparts: &FileCounterparts,
) -> Option<InstalledSource> {
	if let Some(entry) = entry {
		if entry.source == crate::api::curseforge::normalize::Source::CurseForge {
			return Some(InstalledSource::CurseForge);
		}
		if entry.project_id.as_deref().is_some_and(|id| !id.is_empty()) {
			return Some(InstalledSource::Modrinth);
		}
		return Some(InstalledSource::External);
	}
	// Untracked files (imported or dropped into the instance) still have a
	// source when either side recognizes them.
	// Modrinth wins when both sides recognize the file, matching the listing's
	// default source; the user can still switch to CurseForge explicitly.
	match (counterparts.mr.is_some(), counterparts.cf.is_some()) {
		(true, _) => Some(InstalledSource::Modrinth),
		(false, true) => Some(InstalledSource::CurseForge),
		_ => Some(InstalledSource::External),
	}
}

/// Read-only counterpart lookup for the UI. Never fails: an unrecognized or
/// unmanaged file simply reports no counterpart.
pub(crate) async fn get_source_counterpart(
	instance_id: &str,
	file_path: &str,
	state: &State,
) -> SourceCounterpart {
	let Ok((_, _, entry, counterparts)) =
		resolve_counterpart_parts(instance_id, file_path, state).await
	else {
		return SourceCounterpart::default();
	};
	SourceCounterpart {
		source: counterpart_source(entry.as_ref(), &counterparts),
		managed: entry.is_some(),
		modrinth_project_id: counterparts.mr.as_ref().map(|(project, _)| project.clone()),
		modrinth_version_id: counterparts.mr.as_ref().map(|(_, version)| version.clone()),
		curseforge_project_id: counterparts.cf.map(|(project, _)| project),
		curseforge_file_id: counterparts.cf.map(|(_, file)| file),
	}
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
	entry: Option<&ContentEntry>,
	state: &State,
) -> FileCounterparts {
	let mut counterparts = FileCounterparts::default();

	// Ids stored on the entry are authoritative and free. Only resolve the
	// sides that are still unknown: reading the jar and hitting both APIs on
	// every source-tag click is what made the prompt slow.
	if let Some(entry) = entry {
		if let (Some(project_id), Some(version_id)) = (
			entry.project_id.clone().filter(|id| !id.is_empty()),
			entry.version_id.clone().filter(|id| !id.is_empty()),
		) {
			counterparts.mr = Some((project_id, version_id));
		}
		if let (Some(cf_project_id), Some(cf_version_id)) =
			(entry.cf_project_id, entry.cf_version_id)
		{
			counterparts.cf = Some((cf_project_id, cf_version_id));
		}
	}

	if counterparts.mr.is_none() && !sha1.is_empty() {
		match CachedEntry::get_file_many(
			&[sha1],
			Some(CacheBehaviour::StaleWhileRevalidateSkipOffline),
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

	if counterparts.cf.is_none() {
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
					Some(CacheBehaviour::StaleWhileRevalidateSkipOffline),
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
	let (_scope, file, entry, counterparts) =
		resolve_counterpart_parts(instance_id, project_path, state).await?;

	// Direction follows the entry's tracked source when there is one; an
	// untracked file is displayed as Modrinth, so it switches to CurseForge.
	let switching_to_curseforge = match entry.as_ref() {
		Some(entry) => {
			entry.source != crate::api::curseforge::normalize::Source::CurseForge
		}
		None => true,
	};
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
		let project_type_name = entry
			.as_ref()
			.map(|entry| entry.project_type.get_name().to_string())
			.or_else(|| {
				super::sync_content_files::project_type_for_file(&file)
					.map(|project_type| project_type.get_name().to_string())
			});
		if let Some(project_type_name) = project_type_name {
			record_source_link(
				mr_project_id,
				cf_project_id,
				&project_type_name,
				state,
			)
			.await;
		}
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
