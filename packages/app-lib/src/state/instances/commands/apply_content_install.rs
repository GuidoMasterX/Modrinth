use crate::state::instances::{
    ContentRequirement, ContentSourceKind, Instance, InstanceFile,
    adapters::sqlite::{content_rows, instance_rows},
};
use crate::state::{
    CacheBehaviour, CachedEntry, Dependency, DependencyType, KnownModrinthFile,
    ModLoader, ProjectType, ReleaseChannel, State, Version, cache_file_hash,
};
use crate::util::fetch::{self, DownloadMeta, DownloadReason};
use crate::util::io;
use async_trait::async_trait;
use bytes::Bytes;
use modrinth_content_management::{
    ContentMetadataProvider, ContentType, Error as ResolveError,
    ResolutionPreferences, ResolveContentPlan, ResolveContentRequest,
    ResolvedContent, SkippedContent, SkippedReason,
};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::api::curseforge::normalize::Source;

pub(crate) struct ContentScope {
    pub instance: Instance,
    pub content_set_id: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EntryOrigin {
    pub source: Source,
    pub cf_project_id: Option<i64>,
    pub cf_version_id: Option<i64>,
}

impl EntryOrigin {
    pub fn curseforge(cf_project_id: i64, cf_version_id: Option<i64>) -> Self {
        Self {
            source: Source::CurseForge,
            cf_project_id: Some(cf_project_id),
            cf_version_id,
        }
    }
}

pub(crate) struct InstalledContentFile {
    pub relative_path: String,
    pub project_id: Option<String>,
    pub enabled: bool,
}

pub(crate) struct DownloadedProjectVersion {
    pub file_name: String,
    pub bytes: Bytes,
    pub sha1: Option<String>,
    pub project_type: ProjectType,
    pub project_id: String,
    pub version_id: String,
}

pub(crate) struct InstanceInstallProjectRequest {
    pub project_id: String,
    pub version_id: Option<String>,
    pub content_type: ContentType,
    pub selected: ResolutionPreferences,
}

struct CachedEntryContentProvider<'a> {
    state: &'a State,
    cache_behaviour: Option<CacheBehaviour>,
}

#[async_trait]
impl ContentMetadataProvider for CachedEntryContentProvider<'_> {
    async fn get_version(
        &mut self,
        version_id: &str,
    ) -> Result<Option<modrinth_content_management::Version>, ResolveError>
    {
        let version = CachedEntry::get_version(
            version_id,
            self.cache_behaviour,
            &self.state.pool,
            &self.state.api_semaphore,
        )
        .await
        .map_err(resolve_provider_error)?;

        Ok(version.map(version_to_resolver))
    }

    async fn get_project_versions(
        &mut self,
        project_id: &str,
    ) -> Result<Vec<modrinth_content_management::Version>, ResolveError> {
        let versions = CachedEntry::get_project_versions(
            project_id,
            self.cache_behaviour,
            &self.state.pool,
            &self.state.api_semaphore,
        )
        .await
        .map_err(resolve_provider_error)?;

        Ok(versions
            .unwrap_or_default()
            .into_iter()
            .map(version_to_resolver)
            .collect())
    }
}

fn resolve_provider_error(error: crate::Error) -> ResolveError {
    ResolveError::Provider(error.to_string())
}

fn resolver_error(error: ResolveError) -> crate::Error {
	crate::ErrorKind::InputError(error.to_string()).into()
}

/// Identity key for cross-source matching: lowercase ASCII-alphanumeric only
fn normalized_identity_key(value: &str) -> String {
	value
		.chars()
		.filter(|c| c.is_ascii_alphanumeric())
		.map(|c| c.to_ascii_lowercase())
		.collect()
}

fn project_matches_identity(
	slug: Option<&str>,
	title: &str,
	identity_keys: &HashSet<String>,
) -> bool {
	if identity_keys.is_empty() {
		return false;
	}
	let title_key = normalized_identity_key(title);
	if !title_key.is_empty() && identity_keys.contains(&title_key) {
		return true;
	}
	slug.map(normalized_identity_key).is_some_and(|slug_key| {
		!slug_key.is_empty() && identity_keys.contains(&slug_key)
	})
}

async fn installed_cf_identity_keys(
	content_set_id: &str,
	state: &State,
) -> crate::Result<HashSet<String>> {
	let entries =
		content_rows::get_content_entries(content_set_id, &state.pool).await?;
	let cf_ids = entries
		.iter()
		.filter_map(|entry| entry.cf_project_id.map(|id| id.to_string()))
		.collect::<Vec<_>>();
	if cf_ids.is_empty() {
		return Ok(HashSet::new());
	}
	let id_refs = cf_ids.iter().map(String::as_str).collect::<Vec<_>>();
	let projects = CachedEntry::get_curseforge_project_many(
		&id_refs,
		None,
		&state.pool,
		&state.api_semaphore,
	)
	.await?;
	Ok(projects
		.into_iter()
		.flat_map(|project| {
			project
				.slug
				.iter()
				.map(|slug| normalized_identity_key(slug))
				.chain(std::iter::once(normalized_identity_key(
					&project.title,
				)))
				.collect::<HashSet<_>>()
		})
		.collect())
}

async fn installed_mr_identity_keys(
	content_set_id: &str,
	state: &State,
) -> crate::Result<HashSet<String>> {
	let entries =
		content_rows::get_content_entries(content_set_id, &state.pool).await?;
	let mr_ids = entries
		.iter()
		.filter_map(|entry| entry.project_id.clone())
		.collect::<Vec<_>>();
	if mr_ids.is_empty() {
		return Ok(HashSet::new());
	}
	let id_refs = mr_ids.iter().map(String::as_str).collect::<Vec<_>>();
	let projects = CachedEntry::get_project_many(
		&id_refs,
		None,
		&state.pool,
		&state.api_semaphore,
	)
	.await?;
	Ok(projects
		.into_iter()
		.flat_map(|project| {
			project
				.slug
				.iter()
				.map(|slug| normalized_identity_key(slug))
				.chain(std::iter::once(normalized_identity_key(
					&project.title,
				)))
				.collect::<HashSet<_>>()
		})
		.collect())
}

fn version_to_resolver(
    version: Version,
) -> modrinth_content_management::Version {
    modrinth_content_management::Version {
        id: version.id,
        project_id: version.project_id,
        date_published: version.date_published,
        dependencies: version
            .dependencies
            .into_iter()
            .map(dependency_to_resolver)
            .collect(),
        game_versions: version.game_versions,
        loaders: version.loaders,
    }
}

fn dependency_to_resolver(
    dependency: Dependency,
) -> modrinth_content_management::Dependency {
    modrinth_content_management::Dependency {
        version_id: dependency.version_id,
        project_id: dependency.project_id,
        file_name: dependency.file_name,
        dependency_type: match dependency.dependency_type {
            DependencyType::Required => {
                modrinth_content_management::DependencyType::Required
            }
            DependencyType::Optional => {
                modrinth_content_management::DependencyType::Optional
            }
            DependencyType::Incompatible => {
                modrinth_content_management::DependencyType::Incompatible
            }
            DependencyType::Embedded => {
                modrinth_content_management::DependencyType::Embedded
            }
        },
    }
}

fn target_preferences(
    game_version: String,
    loader: ModLoader,
    content_type: ContentType,
) -> ResolutionPreferences {
    let loader = match content_type {
        ContentType::DataPack => "datapack".to_string(),
        ContentType::ResourcePack => "minecraft".to_string(),
        ContentType::Shader => "iris".to_string(),
        _ => loader.as_str().to_string(),
    };

    ResolutionPreferences {
        game_versions: vec![game_version],
        loaders: vec![loader],
    }
}

pub(crate) async fn resolve_install_plan(
    instance_id: &str,
    request: InstanceInstallProjectRequest,
    state: &State,
) -> crate::Result<ResolveContentPlan> {
    let content_set =
        content_rows::get_applied_content_set(instance_id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Instance {instance_id} has no applied content set"
                ))
            })?;
    let existing_project_ids =
        crate::state::get_installed_project_ids_for_instance(
            instance_id,
            None,
            state,
        )
        .await?;
    let provider = CachedEntryContentProvider {
        state,
        cache_behaviour: Some(CacheBehaviour::MustRevalidate),
    };
    let content_type = request.content_type;
    let request = ResolveContentRequest {
        project_id: request.project_id,
        version_id: request.version_id,
        content_type,
        selected: request.selected,
        target: target_preferences(
            content_set.game_version,
            content_set.loader,
            content_type,
        ),
        existing_project_ids,
    };

	let mut plan = modrinth_content_management::resolve_content(
		provider,
		request,
	)
	.await
	.map_err(resolver_error)?;

	if !plan.dependencies.is_empty() {
		let cf_identity =
			installed_cf_identity_keys(&content_set.id, state).await?;
		if !cf_identity.is_empty() {
			let mut dependencies = Vec::new();
			for dependency in plan.dependencies {
				let skip = match CachedEntry::get_project(
					&dependency.project_id,
					Some(CacheBehaviour::MustRevalidate),
					&state.pool,
					&state.api_semaphore,
				)
				.await
				{
					Ok(Some(project)) => project_matches_identity(
						project.slug.as_deref(),
						&project.title,
						&cf_identity,
					),
					Ok(None) => false,
					Err(error) => {
						tracing::warn!(
							"Unable to check Modrinth dependency {} \
							 against CurseForge installs: {error}",
							dependency.project_id
						);
						false
					}
				};
				if skip {
					tracing::info!(
						"Skipping Modrinth dependency {}: already \
						 installed from CurseForge",
						dependency.project_id
					);
					plan.skipped.push(SkippedContent {
						project_id: dependency.project_id.clone(),
						version_id: Some(dependency.version_id),
						dependent_on_version_id: dependency
							.dependent_on_version_id,
						reason: SkippedReason::AlreadyInstalled,
					});
				} else {
					dependencies.push(dependency);
				}
			}
			plan.dependencies = dependencies;
		}
	}

	Ok(plan)
}

pub(crate) async fn install_resolved_content_plan(
    instance_id: &str,
    plan: &ResolveContentPlan,
    state: &State,
) -> crate::Result<()> {
    add_resolved_content(
        instance_id,
        &plan.primary,
        DownloadReason::Standalone,
        state,
    )
    .await?;
    for dependency in &plan.dependencies {
        add_resolved_content(
            instance_id,
            dependency,
            DownloadReason::Dependency,
            state,
        )
        .await?;
    }

    Ok(())
}

pub(crate) async fn switch_project_version_with_dependencies(
    instance_id: &str,
    project_path: &str,
    version_id: &str,
    state: &State,
) -> crate::Result<String> {
    let version = CachedEntry::get_version(
        version_id,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to install version id {version_id}. Not found."
        ))
    })?;
    let content_type = ProjectType::get_from_loaders(version.loaders.clone())
        .map(ContentType::from)
        .unwrap_or(ContentType::Mod);
    let plan = resolve_install_plan(
        instance_id,
        InstanceInstallProjectRequest {
            project_id: version.project_id,
            version_id: Some(version_id.to_string()),
            content_type,
            selected: ResolutionPreferences::default(),
        },
        state,
    )
    .await?;

    let was_disabled = project_path.ends_with(".disabled");
    let mut new_path = add_project_from_version(
        instance_id,
        &plan.primary.version_id,
        DownloadReason::Update,
        None,
        ContentSourceKind::Local,
        state,
    )
    .await?;

    if was_disabled {
        new_path =
            toggle_disable_project(instance_id, &new_path, Some(false), state)
                .await?;
    }

    for dependency in &plan.dependencies {
        add_resolved_content(
            instance_id,
            dependency,
            DownloadReason::Dependency,
            state,
        )
        .await?;
    }

    if new_path != project_path {
        rename_project_companion_file(
            instance_id,
            project_path,
            &new_path,
            state,
        )
        .await?;
        remove_project(instance_id, project_path, state).await?;
    }

    Ok(new_path)
}

async fn add_resolved_content(
    instance_id: &str,
    content: &ResolvedContent,
    reason: DownloadReason,
    state: &State,
) -> crate::Result<String> {
    add_project_from_version(
        instance_id,
        &content.version_id,
        reason,
        content.dependent_on_version_id.clone(),
        ContentSourceKind::Local,
        state,
    )
    .await
}

pub(crate) async fn resolve_content_scope(
    instance_id: &str,
    content_set_id: Option<&str>,
    state: &State,
) -> crate::Result<ContentScope> {
    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let content_set_id = match content_set_id {
        Some(id) => id.to_string(),
        None => instance.applied_content_set_id.clone().ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Instance {} has no applied content set",
                instance.id
            ))
        })?,
    };

    Ok(ContentScope {
        instance,
        content_set_id,
    })
}

pub(crate) async fn add_project_from_version(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    source_kind: ContentSourceKind,
    state: &State,
) -> crate::Result<String> {
    let downloaded = download_project_version(
        instance_id,
        version_id,
        reason,
        dependent_on_version_id,
        state,
    )
    .await?;

    add_downloaded_project_version(instance_id, downloaded, source_kind, state)
        .await
}

pub(crate) async fn download_project_version(
    instance_id: &str,
    version_id: &str,
    reason: DownloadReason,
    dependent_on_version_id: Option<String>,
    state: &State,
) -> crate::Result<DownloadedProjectVersion> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let content_set =
        content_rows::get_content_set(&scope.content_set_id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown content set {}",
                    scope.content_set_id
                ))
            })?;
    let version = CachedEntry::get_version(
        version_id,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to install version id {version_id}. Not found."
        ))
    })?;
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "No files for input version present!".to_string(),
            )
        })?;
    let download_meta = DownloadMeta {
        reason,
        game_version: content_set.game_version,
        loader: content_set.loader.as_str().to_string(),
        dependent_on: dependent_on_version_id,
    };
    let bytes = fetch::fetch(
        &file.url,
        file.hashes.get("sha1").map(|hash| hash.as_str()),
        Some(&download_meta),
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;
    let project_type = ProjectType::get_from_loaders(version.loaders.clone())
        .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to infer project type for version {version_id}"
        ))
    })?;
    let project_id = version.project_id.clone();
    let version_id = version.id.clone();

    Ok(DownloadedProjectVersion {
        file_name: file.filename.clone(),
        bytes,
        sha1: file.hashes.get("sha1").cloned(),
        project_type,
        project_id,
        version_id,
    })
}

pub(crate) async fn add_downloaded_project_version(
    instance_id: &str,
    downloaded: DownloadedProjectVersion,
    source_kind: ContentSourceKind,
    state: &State,
) -> crate::Result<String> {
    let DownloadedProjectVersion {
        file_name,
        bytes,
        sha1,
        project_type,
        project_id,
        version_id,
    } = downloaded;

    add_project_bytes(
        instance_id,
        &file_name,
        bytes,
        sha1.as_deref(),
        Some(project_type),
        source_kind,
        Some(project_id.as_str()),
        Some(version_id.as_str()),
        EntryOrigin::default(),
        state,
    )
    .await
}

pub(crate) async fn add_project_from_path(
    instance_id: &str,
    path: &Path,
    project_type: Option<ProjectType>,
    state: &State,
) -> crate::Result<String> {
    let file = io::read(path).await?;
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    add_project_bytes(
        instance_id,
        &file_name,
        Bytes::from(file),
        None,
        project_type,
        ContentSourceKind::Local,
        None,
        None,
        EntryOrigin::default(),
        state,
    )
    .await
}

pub(crate) async fn add_project_from_curseforge_file(
    instance_id: &str,
    cf_project_id: i64,
    cf_file_id: i64,
    reason: DownloadReason,
    state: &State,
) -> crate::Result<String> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let content_set =
        content_rows::get_content_set(&scope.content_set_id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown content set {}",
                    scope.content_set_id
                ))
            })?;
    let project = CachedEntry::get_curseforge_project(
        &cf_project_id.to_string(),
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to install CurseForge project {cf_project_id}. Not found."
        ))
    })?;
    let file = CachedEntry::get_curseforge_file(
        &cf_file_id.to_string(),
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to install CurseForge file {cf_file_id}. Not found."
        ))
    })?;
    let raw_file = crate::api::curseforge::api::get_mod_file(
        cf_file_id,
        &state.api_semaphore,
        &state.pool,
    )
    .await?;
    if raw_file
        .download_url
        .as_ref()
        .is_none_or(|url| url.is_empty())
        || raw_file.is_available == Some(false)
    {
        return Err(crate::ErrorKind::InputError(format!(
            "'{}' blocks third-party downloads. Download it manually from its CurseForge page (https://www.curseforge.com/projects/{}); once placed in the instance it will be recognized automatically.",
            project.title, cf_project_id
        ))
        .into());
    }
    let project_type = match project.project_type {
        crate::api::curseforge::normalize::SourceProjectType::Mod => {
            ProjectType::Mod
        }
        crate::api::curseforge::normalize::SourceProjectType::ResourcePack => {
            ProjectType::ResourcePack
        }
        crate::api::curseforge::normalize::SourceProjectType::ShaderPack => {
            ProjectType::ShaderPack
        }
        crate::api::curseforge::normalize::SourceProjectType::DataPack => {
            ProjectType::DataPack
        }
        crate::api::curseforge::normalize::SourceProjectType::Modpack => {
            return Err(crate::ErrorKind::InputError(
                "CurseForge modpacks cannot be installed as single files"
                    .to_string(),
            )
            .into());
        }
    };
    let download_meta = DownloadMeta {
        reason,
        game_version: content_set.game_version,
        loader: content_set.loader.as_str().to_string(),
        dependent_on: None,
    };
    let bytes = fetch::fetch(
        &file.url,
        file.sha1.as_deref(),
        Some(&download_meta),
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    add_project_bytes(
        instance_id,
        &file.filename,
        bytes,
        file.sha1.as_deref(),
        Some(project_type),
        ContentSourceKind::Local,
        None,
        None,
        EntryOrigin::curseforge(cf_project_id, Some(cf_file_id)),
        state,
    )
    .await
}

const CF_DEPENDENCY_RELATION_REQUIRED: i32 = 3;
const CF_DEPENDENCY_MAX_DEPTH: usize = 5;

/// Downloads the bytes for a CurseForge file through the shared download
/// plumbing, resolving the file via the batch files endpoint. Returns the
/// resolved `CFFile` (for hashes/filename) alongside the bytes.
pub(crate) async fn download_curseforge_file_bytes(
    instance_id: &str,
    cf_file_id: i64,
    reason: DownloadReason,
    state: &State,
) -> crate::Result<(crate::api::curseforge::structs::CFFile, Bytes)> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let content_set =
        content_rows::get_content_set(&scope.content_set_id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown content set {}",
                    scope.content_set_id
                ))
            })?;
    let file = crate::api::curseforge::api::get_files(
        &[cf_file_id],
        &state.api_semaphore,
        &state.pool,
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unable to install CurseForge file {cf_file_id}. Not found."
        ))
    })?;
    if file.download_url.as_ref().is_none_or(|url| url.is_empty())
        || file.is_available == Some(false)
    {
        return Err(crate::ErrorKind::InputError(format!(
            "This CurseForge file ({cf_file_id}) blocks third-party downloads. Download it manually from its CurseForge page; once placed in the instance it will be recognized automatically."
        ))
        .into());
    }
    let download_meta = DownloadMeta {
        reason,
        game_version: content_set.game_version,
        loader: content_set.loader.as_str().to_string(),
        dependent_on: None,
    };
    let sha1 = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .map(|hash| hash.value.clone());
    let bytes = fetch::fetch(
        &crate::api::curseforge::api::get_download_url(&file),
        sha1.as_deref(),
        Some(&download_meta),
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    Ok((file, bytes))
}

/// Installs a CurseForge project into an instance, then resolves and
/// installs its required dependencies (transitively, depth-capped). The
/// root project must install successfully; per-dependency failures are
/// logged and skipped. Returns the `cf-<project id>` ids installed.
pub(crate) async fn resolve_and_install_curseforge_project(
    instance_id: &str,
    cf_project_id: i64,
    cf_file_id: Option<i64>,
    state: &State,
) -> crate::Result<Vec<String>> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let content_set =
        content_rows::get_content_set(&scope.content_set_id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown content set {}",
                    scope.content_set_id
                ))
            })?;
    let game_version = content_set.game_version.clone();
    let loader_str = content_set.loader.as_str().to_string();
    let update_channel = scope.instance.update_channel;
    let installed_cf_project_ids: HashSet<i64> =
        content_rows::get_content_entries(&scope.content_set_id, &state.pool)
            .await?
            .into_iter()
            .filter_map(|entry| entry.cf_project_id)
            .collect();
    let installed_mr_identity =
        installed_mr_identity_keys(&scope.content_set_id, state).await?;

    let mut installed = Vec::new();
    let root_file_id = match cf_file_id {
        Some(id) => id,
        None => {
            let project = crate::api::curseforge::api::get_mod(
                cf_project_id,
                &state.api_semaphore,
                &state.pool,
            )
            .await?;
            curseforge_file_for_context(
                &project,
                &game_version,
                &loader_str,
                update_channel,
                state,
            )
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "No compatible CurseForge file found for project \
                     {cf_project_id} targeting Minecraft {game_version}"
                ))
            })?
        }
    };
    add_project_from_curseforge_file(
        instance_id,
        cf_project_id,
        root_file_id,
        DownloadReason::Standalone,
        state,
    )
    .await?;
    installed.push(format!("cf-{cf_project_id}"));

    let mut queue = VecDeque::new();
    if let Ok(root_file) = crate::api::curseforge::api::get_mod_file(
        root_file_id,
        &state.api_semaphore,
        &state.pool,
    )
    .await
    {
        for dependency in root_file.dependencies.iter().filter(|dependency| {
            dependency.relation_type == CF_DEPENDENCY_RELATION_REQUIRED
        }) {
            queue.push_back((dependency.mod_id, None, 0));
        }
    }

    let mut visited = HashSet::new();
    visited.insert(cf_project_id);
    while let Some((dep_id, dep_file_id, depth)) = queue.pop_front() {
        if depth > CF_DEPENDENCY_MAX_DEPTH || !visited.insert(dep_id) {
            continue;
        }
        if installed_cf_project_ids.contains(&dep_id) {
            continue;
        }

        let dep_project = match crate::api::curseforge::api::get_mod(
            dep_id,
            &state.api_semaphore,
            &state.pool,
        )
        .await
        {
            Ok(project) => Some(project),
            Err(error) if dep_file_id.is_none() => {
                tracing::warn!(
                    "Skipping CurseForge dependency {dep_id}: {error}"
                );
                continue;
            }
            Err(_) => None,
        };
        if let Some(project) = &dep_project
            && project_matches_identity(
                project.slug.as_deref(),
                &project.name,
                &installed_mr_identity,
            )
        {
            tracing::info!(
                "Skipping CurseForge dependency {dep_id}: already installed \
                 from Modrinth"
            );
            continue;
        }

        let file_id = match dep_file_id {
            Some(id) => id,
            None => {
                let Some(project) = dep_project.as_ref() else {
                    continue;
                };
                match curseforge_file_for_context(
                    project,
                    &game_version,
                    &loader_str,
                    update_channel,
                    state,
                )
                .await
                {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        tracing::warn!(
                            "Skipping CurseForge dependency {dep_id}: no \
                             compatible file for Minecraft {game_version}"
                        );
                        continue;
                    }
                    Err(error) => {
                        tracing::warn!(
                            "Skipping CurseForge dependency {dep_id}: {error}"
                        );
                        continue;
                    }
                }
            }
        };

        match add_project_from_curseforge_file(
            instance_id,
            dep_id,
            file_id,
            DownloadReason::Dependency,
            state,
        )
        .await
        {
            Ok(_) => installed.push(format!("cf-{dep_id}")),
            Err(error) => {
                tracing::warn!(
                    "Skipping CurseForge dependency {dep_id}: {error}"
                );
                continue;
            }
        }

        if let Ok(dep_file) = crate::api::curseforge::api::get_mod_file(
            file_id,
            &state.api_semaphore,
            &state.pool,
        )
        .await
        {
            for dependency in
                dep_file.dependencies.iter().filter(|dependency| {
                    dependency.relation_type == CF_DEPENDENCY_RELATION_REQUIRED
                })
            {
                queue.push_back((dependency.mod_id, None, depth + 1));
            }
        }
    }

    Ok(installed)
}

/// Picks the best file id for a CurseForge project in the given instance
/// context: first a matching entry from `latest_files_indexes` (release
/// preferred, loader ignored for vanilla sets), falling back to a paginated
/// file listing filtered by `curseforge_latest_compatible_file`.
async fn curseforge_file_for_context(
    project: &crate::api::curseforge::structs::CFProject,
    game_version: &str,
    loader_str: &str,
    update_channel: ReleaseChannel,
    state: &State,
) -> crate::Result<Option<i64>> {
    let matching = project
        .latest_files_indexes
        .iter()
        .filter(|index| index.game_version.as_deref() == Some(game_version))
        .filter(|index| {
            loader_str == "vanilla"
                || index.mod_loader.is_none_or(|mod_loader| {
                    crate::api::curseforge::normalize::loader_name(mod_loader)
                        .is_some_and(|name| {
                            name.eq_ignore_ascii_case(loader_str)
                        })
                })
        })
        .collect::<Vec<_>>();
    if let Some(index) = matching
        .iter()
        .find(|index| index.release_type == Some(1))
        .or_else(|| matching.first())
    {
        return Ok(Some(index.file_id));
    }

    let files = crate::api::curseforge::api::get_mod_files(
        project.id,
        Some(game_version),
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await?;
    Ok(
        super::check_content_updates::curseforge_latest_compatible_file(
            &files,
            game_version,
            loader_str,
            update_channel,
        )
        .map(|file| file.id),
    )
}

pub(crate) async fn add_project_bytes(
    instance_id: &str,
    file_name: &str,
    bytes: Bytes,
    hash: Option<&str>,
    project_type: Option<ProjectType>,
    source_kind: ContentSourceKind,
    project_id: Option<&str>,
    version_id: Option<&str>,
    origin: EntryOrigin,
    state: &State,
) -> crate::Result<String> {
    if !path_util::is_safe_file_name(file_name) {
        return Err(crate::ErrorKind::InputError(format!(
            "Project file {file_name} has an invalid file name"
        ))
        .into());
    }

    let _content_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let project_type = match project_type {
        Some(project_type) => project_type,
        None => {
            super::embedded_content_metadata::infer_project_type_bytes(&bytes)?
        }
    };
    let relative_path = format!("{}/{}", project_type.get_folder(), file_name);
    let full_path =
        instance_full_path(state, &scope.instance).join(&relative_path);
    let sha1 = match hash {
        Some(hash) => hash.to_string(),
        None => fetch::sha1_async(bytes.clone()).await?,
    };

    fetch::write(&full_path, &bytes, &state.io_semaphore).await?;
    let modified_at_ns =
        crate::state::file_modified_at_ns(&io::metadata(&full_path).await?)?;
    cache_file_hash(
        bytes.clone(),
        &scope.instance.id,
        &relative_path,
        modified_at_ns,
        Some(&sha1),
        Some(project_type),
        project_id.zip(version_id).map(|(project_id, version_id)| {
            KnownModrinthFile {
                project_id,
                version_id,
            }
        }),
        &state.pool,
    )
    .await?;

    let mut tx = state.pool.begin().await?;
    let file = content_rows::upsert_instance_file_from_parts(
        content_rows::UpsertInstanceFile {
            instance_id: &scope.instance.id,
            relative_path: &relative_path,
            file_name,
            enabled: !relative_path.ends_with(".disabled"),
            sha1: &sha1,
            size: bytes.len() as u64,
            missing: false,
        },
        &mut tx,
    )
    .await?;
    upsert_entry_for_file(
        &scope,
        &file,
        project_type,
        project_id,
        version_id,
        source_kind,
        origin,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    super::mark_shared_instance_stale(instance_id, &state.pool).await?;

    Ok(relative_path)
}

pub(crate) async fn record_project_file(
    instance_id: &str,
    relative_path: &str,
    sha1: &str,
    size: u64,
    project_type: ProjectType,
    source_kind: ContentSourceKind,
    project_id: Option<&str>,
    version_id: Option<&str>,
    origin: EntryOrigin,
    state: &State,
) -> crate::Result<()> {
    let _content_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let file_name = Path::new(relative_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut tx = state.pool.begin().await?;
    let file = content_rows::upsert_instance_file_from_parts(
        content_rows::UpsertInstanceFile {
            instance_id: &scope.instance.id,
            relative_path,
            file_name: &file_name,
            enabled: !relative_path.ends_with(".disabled"),
            sha1,
            size,
            missing: false,
        },
        &mut tx,
    )
    .await?;
    upsert_entry_for_file(
        &scope,
        &file,
        project_type,
        project_id,
        version_id,
        source_kind,
        origin,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    super::mark_shared_instance_stale(instance_id, &state.pool).await?;

    Ok(())
}

pub(crate) async fn toggle_disable_project(
    instance_id: &str,
    project_path: &str,
    desired_enabled: Option<bool>,
    state: &State,
) -> crate::Result<String> {
    let _content_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_full_path(state, &scope.instance);
    let trimmed = project_path.trim_end_matches(".disabled");
    let current_path = if base.join(project_path).exists() {
        project_path.to_string()
    } else if base.join(format!("{trimmed}.disabled")).exists() {
        format!("{trimmed}.disabled")
    } else if base.join(trimmed).exists() {
        trimmed.to_string()
    } else {
        return Err(crate::ErrorKind::FSError(format!(
            "Could not find project file for '{project_path}' in instance"
        ))
        .into());
    };
    let current_enabled = !current_path.ends_with(".disabled");
    let enabled = desired_enabled.unwrap_or(!current_enabled);
    let new_path = if enabled {
        trimmed.to_string()
    } else {
        format!("{trimmed}.disabled")
    };

    if current_path != new_path {
        io::rename_or_move(&base.join(&current_path), &base.join(&new_path))
            .await?;
    }

    let file_name = Path::new(&new_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut tx = state.pool.begin().await?;
    let file = match content_rows::rename_instance_file(
        &scope.instance.id,
        &current_path,
        &new_path,
        &file_name,
        enabled,
        &mut tx,
    )
    .await?
    {
        Some(file) => file,
        None if current_path != project_path => {
            match content_rows::rename_instance_file(
                &scope.instance.id,
                project_path,
                &new_path,
                &file_name,
                enabled,
                &mut tx,
            )
            .await?
            {
                Some(file) => file,
                None => {
                    index_existing_file(&scope, &new_path, state, &mut tx)
                        .await?
                }
            }
        }
        None => index_existing_file(&scope, &new_path, state, &mut tx).await?,
    };
    let updated_entry = content_rows::set_content_entry_enabled_for_file(
        &scope.content_set_id,
        &file.id,
        enabled,
        &mut tx,
    )
    .await?;
    if !updated_entry {
        let project_type = ProjectType::get_from_parent_folder(&new_path)
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unable to infer project type from {new_path}"
                ))
            })?;
        upsert_entry_for_file(
            &scope,
            &file,
            project_type,
            None,
            None,
            ContentSourceKind::Local,
            EntryOrigin::default(),
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;

    super::mark_shared_instance_stale(instance_id, &state.pool).await?;

    Ok(new_path)
}

pub(crate) async fn remove_project(
    instance_id: &str,
    project_path: &str,
    state: &State,
) -> crate::Result<()> {
    let _content_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let base = instance_full_path(state, &scope.instance);
    let file = content_rows::get_instance_file_by_relative_path(
        &scope.instance.id,
        project_path,
        &state.pool,
    )
    .await?;

    match io::remove_file(base.join(project_path)).await {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }

    if let Some(file) = file {
        let mut tx = state.pool.begin().await?;
        content_rows::remove_content_entries_for_file(
            &scope.content_set_id,
            &file.id,
            &mut tx,
        )
        .await?;
        content_rows::remove_instance_file_by_relative_path(
            &scope.instance.id,
            project_path,
            &mut tx,
        )
        .await?;
        tx.commit().await?;
    }

    super::mark_shared_instance_stale(instance_id, &state.pool).await?;

    Ok(())
}

pub(crate) async fn content_source_kind_for_project_path(
    instance_id: &str,
    project_path: &str,
    state: &State,
) -> crate::Result<Option<ContentSourceKind>> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let Some(file) = content_rows::get_instance_file_by_relative_path(
        &scope.instance.id,
        project_path,
        &state.pool,
    )
    .await?
    else {
        return Ok(None);
    };
    let entries =
        content_rows::get_content_entries(&scope.content_set_id, &state.pool)
            .await?;

    Ok(entries.into_iter().find_map(|entry| {
        (entry.file_id.as_deref() == Some(file.id.as_str()))
            .then_some(entry.source_kind)
    }))
}

pub(crate) async fn is_project_locked(
    instance_id: &str,
    project_path: &str,
    state: &State,
) -> crate::Result<bool> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    content_rows::is_instance_file_locked(
        &scope.instance.id,
        project_path,
        &state.pool,
    )
    .await
}

pub(crate) async fn set_project_locked(
    instance_id: &str,
    project_path: &str,
    locked: bool,
    state: &State,
) -> crate::Result<()> {
    let _content_lock = state.lock_instance_content(instance_id).await;
    let scope = resolve_content_scope(instance_id, None, state).await?;
    content_rows::set_instance_file_locked(
        &scope.instance.id,
        project_path,
        locked,
        &state.pool,
    )
    .await
}

pub(crate) async fn rename_project_companion_file(
    instance_id: &str,
    old_project_path: &str,
    new_project_path: &str,
    state: &State,
) -> crate::Result<()> {
    let project_type = ProjectType::get_from_parent_folder(new_project_path);
    if project_type == Some(ProjectType::ShaderPack) {
        let scope = resolve_content_scope(instance_id, None, state).await?;
        let base = instance_full_path(state, &scope.instance);

        let old_txt_path = base.join(format!(
            "{}.txt",
            old_project_path.trim_end_matches(".disabled")
        ));
        let new_txt_path = base.join(format!(
            "{}.txt",
            new_project_path.trim_end_matches(".disabled")
        ));

        if old_txt_path.exists() {
            if new_txt_path.exists()
                && io::canonicalize(&old_txt_path)?
                    == io::canonicalize(&new_txt_path)?
            {
                return Ok(());
            }

            io::copy(&old_txt_path, &new_txt_path).await?;
            io::remove_file(&old_txt_path).await?;
        }
    }

    Ok(())
}

pub(crate) async fn list_project_files(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstalledContentFile>> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let entries =
        content_rows::get_content_entries(&scope.content_set_id, &state.pool)
            .await?;
    let files =
        content_rows::get_instance_files(&scope.instance.id, &state.pool)
            .await?
            .into_iter()
            .map(|file| (file.id.clone(), file))
            .collect::<std::collections::HashMap<_, _>>();

    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let file = files.get(entry.file_id.as_ref()?)?;
            Some(InstalledContentFile {
                relative_path: file.relative_path.clone(),
                project_id: entry.project_id,
                enabled: entry.enabled && file.enabled,
            })
        })
        .collect())
}

pub(crate) fn instance_full_path(
    state: &State,
    instance: &Instance,
) -> PathBuf {
    state.directories.instances_dir().join(&instance.path)
}

async fn index_existing_file(
    scope: &ContentScope,
    relative_path: &str,
    state: &State,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<InstanceFile> {
    let full_path =
        instance_full_path(state, &scope.instance).join(relative_path);
    let (size, sha1) = fetch::sha1_file_async(&full_path).await?;
    let file_name = Path::new(relative_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let project_type = ProjectType::get_from_parent_folder(relative_path)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Unable to infer project type from {relative_path}"
            ))
        })?;

    let file = content_rows::upsert_instance_file_from_parts(
        content_rows::UpsertInstanceFile {
            instance_id: &scope.instance.id,
            relative_path,
            file_name: &file_name,
            enabled: !relative_path.ends_with(".disabled"),
            sha1: &sha1,
            size,
            missing: false,
        },
        tx,
    )
    .await?;
    upsert_entry_for_file(
        scope,
        &file,
        project_type,
        None,
        None,
        ContentSourceKind::Local,
        EntryOrigin::default(),
        tx,
    )
    .await?;

    Ok(file)
}

async fn upsert_entry_for_file(
    scope: &ContentScope,
    file: &InstanceFile,
    project_type: ProjectType,
    project_id: Option<&str>,
    version_id: Option<&str>,
    source_kind: ContentSourceKind,
    origin: EntryOrigin,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<()> {
    content_rows::upsert_content_entry_from_parts(
        content_rows::UpsertContentEntry {
            instance_id: &scope.instance.id,
            content_set_id: &scope.content_set_id,
            file_id: Some(&file.id),
            project_type,
            project_id,
            version_id,
            source_kind,
            source: origin.source,
            cf_project_id: origin.cf_project_id,
            cf_version_id: origin.cf_version_id,
            server_requirement: ContentRequirement::Required,
            client_requirement: ContentRequirement::Required,
            enabled: file.enabled,
        },
        tx,
    )
    .await?;

    Ok(())
}
