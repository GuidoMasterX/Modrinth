use super::sync_content_files::{
    project_type_for_file, sync_instance_content_files,
};
use crate::State;
use crate::api::curseforge::normalize::{
    Source, SourceProject, SourceVersionFile,
};
use crate::pack::install_from::{PackFileHash, PackFormat};
use crate::state::instances::adapters::sqlite;
use crate::state::instances::{
    ContentEntry, ContentSet, ContentSourceKind, Instance, InstanceFile,
    InstanceInstallCandidate, InstanceInstallTarget, InstanceLink,
};
use crate::state::{
    CacheBehaviour, CachedEntry, CachedFile, ContentFile, ContentItem,
    ContentItemOwner, ContentItemProject, ContentItemVersion, Dependency,
    License, LinkedModpackInfo, ModLoader, Organization, OwnerType, Project,
    ProjectType, ReleaseChannel, TeamMember, Version, VersionEnvironment,
    VersionV3,
};
use crate::util::fetch::{
    DownloadMeta, DownloadReason, FetchSemaphore, fetch_file_mirrors,
};
use async_zip::tokio::read::fs::ZipFileReader;
use dashmap::DashMap;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
struct ResolvedContentScope {
    instance: Instance,
    content_set: ContentSet,
}

#[derive(Clone, Copy, Debug)]
enum ContentFilter<'a> {
    All,
    ExcludeModpack(&'a ModpackIdentifiers),
    ExcludeSourceKind {
        source_kind: ContentSourceKind,
        exclude_untracked: bool,
    },
    OnlyModpack(&'a ModpackIdentifiers),
    OnlySourceKind {
        source_kind: ContentSourceKind,
        include_untracked: bool,
    },
}

pub(crate) async fn list_content_sets(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<ContentSet>> {
    let instance = sqlite::instance_rows::get_instance_by_id(instance_id, pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;

    sqlite::content_rows::get_content_sets_for_instance(&instance.id, pool)
        .await
}

pub(crate) async fn get_content_projects(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<DashMap<String, ContentFile>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;

    content_projects_for_scope(
        &resolved,
        cache_behaviour,
        state,
        ContentFilter::All,
    )
    .await
}

pub(crate) async fn get_installed_project_ids_for_instance(
    instance_id: &str,
    content_set_id: Option<&str>,
    state: &State,
) -> crate::Result<Vec<String>> {
    let projects =
        get_content_projects(instance_id, content_set_id, None, state).await?;

    Ok(projects
        .into_iter()
        .filter_map(|(_, file)| {
            file.metadata.map(|metadata| metadata.project_id)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect())
}

#[derive(sqlx::FromRow)]
struct InstanceInstallCandidateRow {
    id: String,
    name: String,
    icon_path: Option<String>,
    game_version: String,
    loader: String,
    installed: i64,
}

pub(crate) async fn get_instance_install_candidates(
    project_id: &str,
    cf_project_id: Option<i64>,
    project_type: ProjectType,
    targets: &[InstanceInstallTarget],
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceInstallCandidate>> {
    let rows = sqlx::query_as!(
        InstanceInstallCandidateRow,
        r#"
		SELECT
			i.id,
			i.name,
			i.icon_path,
			cs.game_version,
			cs.loader,
			CASE
				WHEN EXISTS (
					SELECT 1
					FROM instance_content_entries entry
					INNER JOIN instance_files file
						ON file.id = entry.file_id
					WHERE entry.content_set_id = cs.id
						AND (entry.project_id = ?
							OR entry.cf_project_id = ?)
						AND file.missing = 0
				)
					THEN 1
				ELSE 0
			END AS "installed!: i64"
		FROM instances i
		INNER JOIN instance_content_sets cs
			ON cs.id = i.applied_content_set_id
		LEFT JOIN instance_links link
			ON link.instance_id = i.id
		WHERE COALESCE(link.link_kind, 'unmanaged') NOT IN (
			'server_project',
			'server_project_modpack'
		)
		ORDER BY i.name ASC
		"#,
        project_id,
        cf_project_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let loader = ModLoader::from_string(&row.loader);
            let compatible = instance_matches_targets(
                project_type,
                &row.game_version,
                loader.as_str(),
                targets,
            );

            InstanceInstallCandidate {
                id: row.id,
                name: row.name,
                icon_path: row.icon_path,
                game_version: row.game_version,
                loader,
                installed: row.installed != 0,
                compatible,
            }
        })
        .collect())
}

fn instance_matches_targets(
    project_type: ProjectType,
    game_version: &str,
    loader: &str,
    targets: &[InstanceInstallTarget],
) -> bool {
    targets.iter().any(|target| {
        target.game_version == game_version
            && (project_type != ProjectType::Mod
                || target.loader == loader
                || target.loader == "datapack")
    })
}

pub(crate) async fn list_content(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    list_content_inner(
        instance_id,
        content_set_id,
        cache_behaviour,
        false,
        state,
    )
    .await
}

pub(crate) async fn list_pack_content(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    list_content_inner(instance_id, None, None, true, state).await
}

async fn list_content_inner(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    packs_only: bool,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;
    let link = sqlite::instance_rows::get_instance_link(
        &resolved.instance.id,
        &state.pool,
    )
    .await?;
    let imported_modpack_scope = is_imported_modpack_scope(&link);
    let linked_modpack_source_kind = linked_modpack_source_kind(&link);
    let modpack_ids = if imported_modpack_scope {
        None
    } else {
        match linked_modpack_ids(&link) {
            Some((_, version_id)) => {
                get_cached_modpack_identifiers(
                    &version_id,
                    &state.pool,
                    &state.api_semaphore,
                )
                .await?
            }
            None => None,
        }
    };
    let filter = if imported_modpack_scope {
        ContentFilter::ExcludeSourceKind {
            source_kind: imported_scope_source_kind(&link),
            exclude_untracked: resolved.instance.install_stage
                != crate::state::InstanceInstallStage::Installed,
        }
    } else if let Some(ids) = modpack_ids.as_ref() {
        ContentFilter::ExcludeModpack(ids)
    } else if let Some(source_kind) = linked_modpack_source_kind {
        ContentFilter::ExcludeSourceKind {
            source_kind,
            exclude_untracked: true,
        }
    } else {
        ContentFilter::All
    };
    let files = content_projects_for_scope_inner(
        &resolved,
        cache_behaviour,
        state,
        filter,
        packs_only,
    )
    .await?;
    let files = files.into_iter().collect::<Vec<_>>();

    content_files_to_content_items(
        &resolved.instance,
        resolved.content_set.loader,
        &files,
        cache_behaviour,
        state,
    )
    .await
}

pub(crate) async fn list_linked_modpack_content(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;
    let link = sqlite::instance_rows::get_instance_link(
        &resolved.instance.id,
        &state.pool,
    )
    .await?;
    if is_imported_modpack_scope(&link) {
        let files = content_projects_for_scope(
            &resolved,
            cache_behaviour,
            state,
            ContentFilter::OnlySourceKind {
                source_kind: imported_scope_source_kind(&link),
                include_untracked: resolved.instance.install_stage
                    != crate::state::InstanceInstallStage::Installed,
            },
        )
        .await?;
        let files = files.into_iter().collect::<Vec<_>>();

        return content_files_to_content_items(
            &resolved.instance,
            resolved.content_set.loader,
            &files,
            cache_behaviour,
            state,
        )
        .await;
    }

    let Some((_, version_id)) = linked_modpack_ids(&link) else {
        return Ok(Vec::new());
    };
    let modpack_ids = match get_modpack_identifiers(
        &version_id,
        &resolved.content_set,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    {
        Ok(ids) => Some(ids),
        Err(err) => {
            tracing::warn!("Failed to fetch modpack identifiers: {}", err);
            None
        }
    };
    let filter = if let Some(ids) = modpack_ids.as_ref() {
        ContentFilter::OnlyModpack(ids)
    } else if let Some(source_kind) = linked_modpack_source_kind(&link) {
        ContentFilter::OnlySourceKind {
            source_kind,
            include_untracked: true,
        }
    } else {
        return Ok(Vec::new());
    };
    let files =
        content_projects_for_scope(&resolved, cache_behaviour, state, filter)
            .await?;
    let files = files.into_iter().collect::<Vec<_>>();

    content_files_to_content_items(
        &resolved.instance,
        resolved.content_set.loader,
        &files,
        cache_behaviour,
        state,
    )
    .await
}

pub(crate) async fn get_linked_modpack_info(
    instance_id: &str,
    content_set_id: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Option<LinkedModpackInfo>> {
    let resolved = resolve_content_scope_with_instance(
        instance_id,
        content_set_id,
        &state.pool,
    )
    .await?;
    let link = sqlite::instance_rows::get_instance_link(
        &resolved.instance.id,
        &state.pool,
    )
    .await?;
    if let InstanceLink::CurseforgeModpack {
        project_id,
        file_id,
    } = &link
    {
        return get_curseforge_modpack_info(
            *project_id,
            *file_id,
            &resolved.content_set.game_version,
            resolved.content_set.loader.as_str(),
            resolved.instance.update_channel,
            cache_behaviour,
            state,
        )
        .await
        .map(Some);
    }
    let Some((project_id, version_id)) =
        linked_modpack_ids_for_instance(&resolved.instance.id, &state.pool)
            .await?
    else {
        return Ok(None);
    };
    let (project, version, all_versions) = tokio::try_join!(
        CachedEntry::get_project(
            &project_id,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        ),
        CachedEntry::get_version(
            &version_id,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        ),
        CachedEntry::get_project_versions(
            &project_id,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        ),
    )?;
    let version_project_id = version
        .as_ref()
        .filter(|version| version.project_id != project_id)
        .map(|version| version.project_id.clone());
    let (project, all_versions) =
        if let Some(version_project_id) = version_project_id {
            let (modpack_project, modpack_versions) = tokio::try_join!(
                CachedEntry::get_project(
                    &version_project_id,
                    cache_behaviour,
                    &state.pool,
                    &state.api_semaphore,
                ),
                CachedEntry::get_project_versions(
                    &version_project_id,
                    cache_behaviour,
                    &state.pool,
                    &state.api_semaphore,
                ),
            )?;
            (modpack_project.or(project), modpack_versions)
        } else {
            (project, all_versions)
        };
    let project = project.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Linked modpack project {project_id} not found"
        ))
    })?;
    let owner = if let Some(org_id) = &project.organization {
        let org = CachedEntry::get_organization(
            org_id,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        org.map(|org| ContentItemOwner {
            id: org.id,
            name: org.name,
            avatar_url: org.icon_url,
            owner_type: OwnerType::Organization,
        })
    } else {
        let team = CachedEntry::get_team(
            &project.team,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        team.and_then(|team| {
            team.into_iter()
                .find(|member| member.is_owner)
                .map(|member| ContentItemOwner {
                    id: member.user.id,
                    name: member.user.username,
                    avatar_url: member.user.avatar_url,
                    owner_type: OwnerType::User,
                })
        })
    };
    let (has_update, update_version_id, update_version) = version
        .as_ref()
        .map(|version| {
            check_modpack_update(
                &version_id,
                version,
                all_versions,
                resolved.instance.update_channel,
            )
        })
        .unwrap_or((false, None, None));

    Ok(Some(LinkedModpackInfo {
        project,
        version,
        owner,
        has_update,
        update_version_id,
        update_version,
    }))
}

pub(crate) async fn dependencies_to_content_items(
    dependencies: &[Dependency],
    cache_behaviour: Option<CacheBehaviour>,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Vec<ContentItem>> {
    let project_ids = dependencies
        .iter()
        .filter_map(|dependency| dependency.project_id.clone())
        .collect::<HashSet<_>>();
    if project_ids.is_empty() {
        return Ok(Vec::new());
    }
    let version_ids = dependencies
        .iter()
        .filter_map(|dependency| dependency.version_id.clone())
        .collect::<HashSet<_>>();
    let meta = resolve_metadata(
        &project_ids,
        &version_ids,
        cache_behaviour,
        pool,
        fetch_semaphore,
    )
    .await?;
    let mut items = dependencies
        .iter()
        .filter_map(|dependency| {
            let project_id = dependency.project_id.as_ref()?;
            let project = meta
                .projects
                .iter()
                .find(|project| &project.id == project_id)?;
            let version =
                dependency.version_id.as_ref().and_then(|version_id| {
                    meta.versions
                        .iter()
                        .find(|version| &version.id == version_id)
                });
            let owner =
                resolve_owner(project, &meta.teams, &meta.organizations);
            let project_type =
                project_type_from_api_name(&project.project_type);

            Some(ContentItem {
                synced_pack: None,
                file_name: version
                    .and_then(|version| version.files.first())
                    .map(|file| file.filename.clone())
                    .unwrap_or_else(|| {
                        format!(
                            "{}.jar",
                            project.slug.as_deref().unwrap_or(&project.id)
                        )
                    }),
                file_path: String::new(),
                id: String::new(),
                size: version
                    .and_then(|version| version.files.first())
                    .map(|file| file.size as u64)
                    .unwrap_or(0),
                enabled: true,
                locked: false,
                project_type,
                project: Some(content_item_project(project)),
                version: version.map(|version| ContentItemVersion {
                    id: version.id.clone(),
                    version_number: version.version_number.clone(),
                    file_name: version
                        .files
                        .first()
                        .map(|file| file.filename.clone())
                        .unwrap_or_default(),
                    date_published: Some(version.date_published.to_rfc3339()),
                }),
                environment: resolve_environment(
                    dependency.version_id.as_deref(),
                    &meta.versions_v3,
                ),
                owner,
                has_update: false,
                update_version_id: None,
                date_added: None,
                source_kind: None,
                package_source: Some(Source::Modrinth),
                cf_project_id: None,
                cf_version_id: None,
                external_url: None,
                embedded_metadata: None,
            })
        })
        .collect::<Vec<_>>();
    sort_content_items(&mut items);

    Ok(items)
}

async fn resolve_content_scope_with_instance(
    instance_id: &str,
    content_set_id: Option<&str>,
    pool: &SqlitePool,
) -> crate::Result<ResolvedContentScope> {
    let instance = sqlite::instance_rows::get_instance_by_id(instance_id, pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let content_set = match content_set_id {
        Some(content_set_id) => {
            let content_set =
                sqlite::content_rows::get_content_set(content_set_id, pool)
                    .await?
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(format!(
                            "Unknown content set {content_set_id}"
                        ))
                    })?;

            if content_set.instance_id != instance.id {
                return Err(crate::ErrorKind::InputError(format!(
					"Content set {content_set_id} does not belong to instance {}",
					instance.id
				))
				.into());
            }

            content_set
        }
        None => {
            sqlite::content_rows::get_applied_content_set(&instance.id, pool)
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Instance {} has no applied content set",
                        instance.id
                    ))
                })?
        }
    };

    Ok(ResolvedContentScope {
        instance,
        content_set,
    })
}

async fn content_projects_for_scope(
    resolved: &ResolvedContentScope,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
    filter: ContentFilter<'_>,
) -> crate::Result<DashMap<String, ContentFile>> {
    content_projects_for_scope_inner(
        resolved,
        cache_behaviour,
        state,
        filter,
        false,
    )
    .await
}

async fn content_projects_for_scope_inner(
    resolved: &ResolvedContentScope,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
    filter: ContentFilter<'_>,
    packs_only: bool,
) -> crate::Result<DashMap<String, ContentFile>> {
    let mut files =
        sync_instance_content_files(&resolved.instance, state).await?;
    if packs_only {
        files.retain(|file| {
            matches!(
                project_type_for_file(file),
                Some(ProjectType::ResourcePack | ProjectType::DataPack),
            )
        });
    }
    let entries = sqlite::content_rows::get_content_entries(
        &resolved.content_set.id,
        &state.pool,
    )
    .await?;
    let cf_update_checks =
        sqlite::content_rows::get_content_update_checks_for_content_set(
            &resolved.content_set.id,
            &state.pool,
        )
        .await?;
    let entries_by_file_id = entries
        .iter()
        .filter_map(|entry| {
            entry.file_id.as_deref().map(|file_id| (file_id, entry))
        })
        .collect::<HashMap<_, _>>();
    let locked_file_ids = sqlite::content_rows::get_locked_instance_file_ids(
        &resolved.instance.id,
        &state.pool,
    )
    .await?;
    let hashes = files
        .iter()
        .map(|file| file.sha1.as_str())
        .collect::<Vec<_>>();
    let file_info = CachedEntry::get_file_many(
        &hashes,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let file_info_by_hash = file_info
        .into_iter()
        .map(|file| (file.hash.clone(), file))
        .collect::<HashMap<_, _>>();
    let cf_metadata_by_hash = detect_curseforge_metadata(
        state,
        &resolved.instance,
        &files,
        &entries_by_file_id,
        &file_info_by_hash,
        cache_behaviour,
    )
    .await?;
    let installed_channels = if packs_only {
        HashMap::new()
    } else {
        get_installed_update_channels(
            &file_info_by_hash,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        )
        .await?
    };
    let update_keys = files
        .iter()
        .filter(|_| !packs_only)
        .filter(|file| file_info_by_hash.contains_key(&file.sha1))
        .filter_map(|file| {
            let project_type = project_type_for_file(file)?;
            let channel = resolved.instance.update_channel.least_stable(
                installed_channels
                    .get(&file.sha1)
                    .copied()
                    .unwrap_or(resolved.instance.update_channel),
            );
            Some(file_update_cache_key(
                &file.sha1,
                project_type,
                &resolved.content_set,
                channel,
            ))
        })
        .collect::<Vec<_>>();
    let update_key_refs =
        update_keys.iter().map(String::as_str).collect::<Vec<_>>();
    let file_updates = CachedEntry::get_file_update_many(
        &update_key_refs,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let mut updates_by_hash: HashMap<String, Vec<String>> = HashMap::new();
    for update in file_updates {
        updates_by_hash
            .entry(update.hash)
            .or_default()
            .push(update.update_version_id);
    }
    let output = DashMap::new();

    for file in files {
        if file.missing {
            continue;
        }

        let Some(project_type) = project_type_for_file(&file) else {
            continue;
        };
        let metadata = file_info_by_hash.get(&file.sha1).cloned();
        let entry = entries_by_file_id.get(file.id.as_str()).copied();
        let cf_metadata = cf_metadata_by_hash.get(&file.sha1);

        match filter {
            ContentFilter::All => {}
            ContentFilter::ExcludeModpack(ids) => {
                if ids.is_modpack_file(
                    &file.sha1,
                    metadata.as_ref(),
                    entry.and_then(|entry| entry.project_id.as_deref()),
                ) {
                    continue;
                }
            }
            ContentFilter::ExcludeSourceKind {
                source_kind,
                exclude_untracked,
            } => {
                if entry.is_some_and(|entry| entry.source_kind == source_kind)
                    || (exclude_untracked && entry.is_none())
                {
                    continue;
                }
            }
            ContentFilter::OnlyModpack(ids) => {
                if !ids.is_modpack_file(
                    &file.sha1,
                    metadata.as_ref(),
                    entry.and_then(|entry| entry.project_id.as_deref()),
                ) {
                    continue;
                }
            }
            ContentFilter::OnlySourceKind {
                source_kind,
                include_untracked,
            } => {
                if !(entry
                    .is_some_and(|entry| entry.source_kind == source_kind)
                    || include_untracked && entry.is_none())
                {
                    continue;
                }
            }
        }

        let mut update_version_id = metadata.as_ref().and_then(|metadata| {
            let update_ids =
                updates_by_hash.remove(&file.sha1).unwrap_or_default();
            if !update_ids.contains(&metadata.version_id) {
                update_ids.into_iter().next()
            } else {
                None
            }
        });
        if update_version_id.is_none()
            && entry.is_some_and(|entry| entry.source == Source::CurseForge)
        {
            update_version_id = entry
                .and_then(|entry| cf_update_checks.get(entry.id.as_str()))
                .cloned();
        }

        output.insert(
            file.relative_path.clone(),
            ContentFile {
                update_version_id,
                hash: file.sha1,
                file_name: file.file_name,
                enabled: entry.map_or(file.enabled, |entry| {
                    entry.enabled && file.enabled
                }),
                locked: locked_file_ids.contains(&file.id),
                size: file.size,
                metadata: file_metadata_from_entry_or_cache(
                    entry,
                    metadata,
                    cf_metadata,
                ),
                project_type,
                source_kind: entry.map(|entry| entry.source_kind),
            },
        );
    }

    Ok(output)
}

async fn get_installed_update_channels(
    file_info_by_hash: &HashMap<String, CachedFile>,
    cache_behaviour: Option<CacheBehaviour>,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<HashMap<String, ReleaseChannel>> {
    let version_ids = file_info_by_hash
        .values()
        .map(|file| file.version_id.as_str())
        .collect::<HashSet<_>>();
    if version_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let version_id_refs = version_ids.iter().copied().collect::<Vec<_>>();
    let versions = CachedEntry::get_version_many(
        &version_id_refs,
        cache_behaviour,
        pool,
        fetch_semaphore,
    )
    .await?;
    let channels_by_version_id = versions
        .into_iter()
        .map(|version| {
            (
                version.id,
                ReleaseChannel::from_version_type(&version.version_type),
            )
        })
        .collect::<HashMap<_, _>>();

    Ok(file_info_by_hash
        .iter()
        .filter_map(|(hash, file)| {
            channels_by_version_id
                .get(&file.version_id)
                .copied()
                .map(|channel| (hash.clone(), channel))
        })
        .collect())
}

fn file_update_cache_key(
    hash: &str,
    project_type: ProjectType,
    content_set: &ContentSet,
    channel: ReleaseChannel,
) -> String {
    let loader_key = if project_type == ProjectType::Mod {
        content_set.loader.as_str().to_string()
    } else {
        project_type.get_loaders().join("+")
    };

    format!(
        "{}-{}-{}-{}",
        hash,
        loader_key,
        channel.key(),
        content_set.game_version
    )
}

async fn content_files_to_content_items(
    instance: &Instance,
    loader: ModLoader,
    files: &[(String, ContentFile)],
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentItem>> {
    let project_ids = files
        .iter()
        .filter_map(|(_, file)| {
            file.metadata
                .as_ref()
                .filter(|metadata| !metadata.project_id.is_empty())
                .map(|metadata| metadata.project_id.clone())
        })
        .collect::<HashSet<_>>();
    let version_ids = files
        .iter()
        .filter_map(|(_, file)| {
            file.metadata
                .as_ref()
                .filter(|metadata| !metadata.version_id.is_empty())
                .map(|metadata| metadata.version_id.clone())
        })
        .collect::<HashSet<_>>();
    let mut cf_project_ids = HashSet::new();
    let mut cf_file_ids = HashSet::new();
    for (_, file) in files {
        if let Some(metadata) = &file.metadata {
            if metadata.source == Source::CurseForge {
                if let Some(id) = metadata.cf_project_id {
                    cf_project_ids.insert(id);
                }
                if let Some(id) = metadata.cf_version_id {
                    cf_file_ids.insert(id);
                }
            }
        }
    }
    let project_key_strings = cf_project_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    let file_key_strings = cf_file_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    let project_keys = project_key_strings
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let file_keys = file_key_strings
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let (cf_projects_by_id, cf_files_by_id) =
        if !project_keys.is_empty() || !file_keys.is_empty() {
            let (cf_projects, cf_files) = tokio::try_join!(
                async {
                    if project_keys.is_empty() {
                        Ok(Vec::new())
                    } else {
                        CachedEntry::get_curseforge_project_many(
                            &project_keys,
                            cache_behaviour,
                            &state.pool,
                            &state.api_semaphore,
                        )
                        .await
                    }
                },
                async {
                    if file_keys.is_empty() {
                        Ok(Vec::new())
                    } else {
                        CachedEntry::get_curseforge_file_many(
                            &file_keys,
                            cache_behaviour,
                            &state.pool,
                            &state.api_semaphore,
                        )
                        .await
                    }
                }
            )?;
            (
                cf_projects
                    .into_iter()
                    .filter_map(|project| {
                        project.id.parse::<i64>().ok().map(|id| (id, project))
                    })
                    .collect::<HashMap<i64, SourceProject>>(),
                cf_files
                    .into_iter()
                    .filter_map(|file| {
                        file.id.parse::<i64>().ok().map(|id| (id, file))
                    })
                    .collect::<HashMap<i64, SourceVersionFile>>(),
            )
        } else {
            (HashMap::new(), HashMap::new())
        };
    let meta = resolve_metadata(
        &project_ids,
        &version_ids,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let embedded_metadata =
        super::embedded_content_metadata::resolve_embedded_content_metadata(
            instance, loader, files, state,
        )
        .await?;
    let instance_path = state.directories.instances_dir().join(&instance.path);
    let paths = files
        .iter()
        .map(|(path, _)| instance_path.join(path))
        .collect::<Vec<_>>();
    let modification_times: Vec<Option<String>> =
        tokio::task::spawn_blocking(move || {
            paths
                .iter()
                .map(|path| {
                    std::fs::metadata(path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .map(|time| {
                            chrono::DateTime::<chrono::Utc>::from(time)
                                .to_rfc3339()
                        })
                })
                .collect()
        })
        .await?;
    let mut items = files
        .iter()
        .enumerate()
        .map(|(index, (path, file))| {
            let metadata = file.metadata.as_ref();
            let modrinth_metadata =
                metadata.filter(|metadata| metadata.source == Source::Modrinth);
            let cf_metadata = metadata
                .filter(|metadata| metadata.source == Source::CurseForge);
            let cf_project = cf_metadata
                .and_then(|metadata| metadata.cf_project_id)
                .and_then(|id| cf_projects_by_id.get(&id));
            let cf_file = cf_metadata
                .and_then(|metadata| metadata.cf_version_id)
                .and_then(|id| cf_files_by_id.get(&id));
            let project = modrinth_metadata.and_then(|metadata| {
                meta.projects
                    .iter()
                    .find(|project| project.id == metadata.project_id)
            });
            let version = modrinth_metadata.and_then(|metadata| {
                meta.versions
                    .iter()
                    .find(|version| version.id == metadata.version_id)
            });
            let owner = project
                .and_then(|project| {
                    resolve_owner(project, &meta.teams, &meta.organizations)
                })
                .or_else(|| {
                    cf_project.and_then(|project| {
                        project.authors.first().map(|author| ContentItemOwner {
                            id: format!("cf-author-{}", author.id),
                            name: author.name.clone().unwrap_or_default(),
                            avatar_url: author.avatar_url.clone(),
                            owner_type: OwnerType::User,
                        })
                    })
                });
            let external_url = cf_project.map(|project| {
                project
                    .website_url
                    .clone()
                    .filter(|url| !url.is_empty())
                    .unwrap_or_else(|| {
                        format!(
                            "https://www.curseforge.com/projects/{}",
                            project.slug.as_deref().unwrap_or(&project.id)
                        )
                    })
            });

            ContentItem {
                synced_pack: None,
                file_name: file.file_name.clone(),
                file_path: path.clone(),
                id: file.hash.clone(),
                size: file.size,
                enabled: file.enabled,
                locked: file.locked,
                project_type: file.project_type,
                project: project.map(content_item_project).or_else(|| {
                    cf_project.map(|project| ContentItemProject {
                        id: format!("cf-{}", project.id),
                        slug: project.slug.clone(),
                        title: project.title.clone(),
                        icon_url: project.icon_url.clone(),
                        license: License {
                            id: String::new(),
                            name: String::new(),
                            url: None,
                        },
                        categories: project.categories.clone(),
                        additional_categories: Vec::new(),
                    })
                }),
                version: version
                    .map(|version| ContentItemVersion {
                        id: version.id.clone(),
                        version_number: version.version_number.clone(),
                        file_name: file.file_name.clone(),
                        date_published: Some(
                            version.date_published.to_rfc3339(),
                        ),
                    })
                    .or_else(|| {
                        cf_file.map(|file| ContentItemVersion {
                            id: format!("cf-{}", file.id),
                            version_number: file
                                .display_name
                                .clone()
                                .filter(|name| !name.is_empty())
                                .unwrap_or_else(|| file.filename.clone()),
                            file_name: file.filename.clone(),
                            date_published: None,
                        })
                    }),
                environment: resolve_environment(
                    modrinth_metadata
                        .map(|metadata| metadata.version_id.as_str()),
                    &meta.versions_v3,
                ),
                owner,
                has_update: file.update_version_id.is_some(),
                update_version_id: file.update_version_id.clone(),
                date_added: modification_times[index].clone(),
                source_kind: file.source_kind,
                package_source: metadata.map(|metadata| metadata.source),
                cf_project_id: cf_metadata
                    .and_then(|metadata| metadata.cf_project_id),
                cf_version_id: cf_metadata
                    .and_then(|metadata| metadata.cf_version_id),
                external_url,
                embedded_metadata: embedded_metadata.get(&file.hash).cloned(),
            }
        })
        .collect::<Vec<_>>();
    merge_cross_source_authors(&mut items);
    sort_content_items(&mut items);

    Ok(items)
}

fn merge_cross_source_authors(items: &mut [ContentItem]) {
	let modrinth_owners: HashMap<String, (String, String, Option<String>)> = items
		.iter()
		.filter_map(|item| {
			let owner = item.owner.as_ref()?;
			if owner.owner_type != OwnerType::User || owner.id.starts_with("cf-author-") {
				return None;
			}
			Some((
				owner.name.trim().to_lowercase(),
				(owner.id.clone(), owner.name.clone(), owner.avatar_url.clone()),
			))
		})
		.collect();

	for item in items.iter_mut() {
		let Some(owner) = item.owner.as_mut() else {
			continue;
		};
		if owner.owner_type != OwnerType::User || !owner.id.starts_with("cf-author-") {
			continue;
		}
		let Some((id, name, avatar_url)) = modrinth_owners.get(&owner.name.trim().to_lowercase())
		else {
			continue;
		};
		*owner = ContentItemOwner {
			id: id.clone(),
			name: name.clone(),
			avatar_url: avatar_url.clone().or_else(|| owner.avatar_url.clone()),
			owner_type: OwnerType::User,
		};
	}
}

struct ResolvedMetadata {
    projects: Vec<Project>,
    versions: Vec<Version>,
    versions_v3: Vec<VersionV3>,
    teams: Vec<Vec<TeamMember>>,
    organizations: Vec<Organization>,
}

async fn resolve_metadata(
    project_ids: &HashSet<String>,
    version_ids: &HashSet<String>,
    cache_behaviour: Option<CacheBehaviour>,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<ResolvedMetadata> {
    let project_id_refs =
        project_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let version_id_refs =
        version_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let (projects, versions, versions_v3) =
        if !project_ids.is_empty() || !version_ids.is_empty() {
            tokio::try_join!(
                async {
                    if project_ids.is_empty() {
                        Ok(Vec::new())
                    } else {
                        CachedEntry::get_project_many(
                            &project_id_refs,
                            cache_behaviour,
                            pool,
                            fetch_semaphore,
                        )
                        .await
                    }
                },
                async {
                    if version_ids.is_empty() {
                        Ok(Vec::new())
                    } else {
                        CachedEntry::get_version_many(
                            &version_id_refs,
                            cache_behaviour,
                            pool,
                            fetch_semaphore,
                        )
                        .await
                    }
                },
                async {
                    if version_ids.is_empty() {
                        Ok(Vec::new())
                    } else {
                        CachedEntry::get_version_v3_many(
                            &version_id_refs,
                            cache_behaviour,
                            pool,
                            fetch_semaphore,
                        )
                        .await
                    }
                }
            )?
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };
    let team_ids = projects
        .iter()
        .map(|project| project.team.clone())
        .collect::<HashSet<_>>();
    let org_ids = projects
        .iter()
        .filter_map(|project| project.organization.clone())
        .collect::<HashSet<_>>();
    let team_id_refs = team_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let org_id_refs = org_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let (teams, organizations) = if !team_ids.is_empty() || !org_ids.is_empty()
    {
        tokio::try_join!(
            async {
                if team_ids.is_empty() {
                    Ok(Vec::new())
                } else {
                    CachedEntry::get_team_many(
                        &team_id_refs,
                        cache_behaviour,
                        pool,
                        fetch_semaphore,
                    )
                    .await
                }
            },
            async {
                if org_ids.is_empty() {
                    Ok(Vec::new())
                } else {
                    CachedEntry::get_organization_many(
                        &org_id_refs,
                        cache_behaviour,
                        pool,
                        fetch_semaphore,
                    )
                    .await
                }
            }
        )?
    } else {
        (Vec::new(), Vec::new())
    };

    Ok(ResolvedMetadata {
        projects,
        versions,
        versions_v3,
        teams,
        organizations,
    })
}

fn resolve_environment(
    version_id: Option<&str>,
    versions: &[VersionV3],
) -> Option<VersionEnvironment> {
    let version_id = version_id?;
    versions
        .iter()
        .find(|version| version.id == version_id)
        .and_then(|version| version.environment)
}

fn resolve_owner(
    project: &Project,
    teams: &[Vec<TeamMember>],
    organizations: &[Organization],
) -> Option<ContentItemOwner> {
    if let Some(org_id) = &project.organization {
        organizations
            .iter()
            .find(|organization| &organization.id == org_id)
            .map(|organization| ContentItemOwner {
                id: organization.id.clone(),
                name: organization.name.clone(),
                avatar_url: organization.icon_url.clone(),
                owner_type: OwnerType::Organization,
            })
    } else {
        teams
            .iter()
            .find(|team| {
                team.first()
                    .is_some_and(|member| member.team_id == project.team)
            })
            .and_then(|team| team.iter().find(|member| member.is_owner))
            .map(|member| ContentItemOwner {
                id: member.user.id.clone(),
                name: member.user.username.clone(),
                avatar_url: member.user.avatar_url.clone(),
                owner_type: OwnerType::User,
            })
    }
}

fn content_item_project(project: &Project) -> ContentItemProject {
    ContentItemProject {
        id: project.id.clone(),
        slug: project.slug.clone(),
        title: project.title.clone(),
        icon_url: project.icon_url.clone(),
        license: project.license.clone(),
        categories: project.categories.clone(),
        additional_categories: project.additional_categories.clone(),
    }
}

fn file_metadata_from_entry_or_cache(
    entry: Option<&ContentEntry>,
    cached: Option<CachedFile>,
    cf: Option<&crate::state::FileMetadata>,
) -> Option<crate::state::FileMetadata> {
    if let Some(entry) = entry {
        if entry.source == Source::CurseForge {
            return Some(crate::state::FileMetadata {
                project_id: String::new(),
                version_id: String::new(),
                source: entry.source,
                cf_project_id: entry.cf_project_id,
                cf_version_id: entry.cf_version_id,
            });
        }
    }

    if let Some(cf) = cf {
        return Some(cf.clone());
    }

    let project_id = entry
        .and_then(|entry| entry.project_id.clone())
        .or_else(|| cached.as_ref().map(|file| file.project_id.clone()))?;
    let version_id = entry
        .and_then(|entry| entry.version_id.clone())
        .or_else(|| cached.as_ref().map(|file| file.version_id.clone()))?;

    Some(crate::state::FileMetadata {
        project_id,
        version_id,
        source: Source::Modrinth,
        cf_project_id: None,
        cf_version_id: None,
    })
}

async fn detect_curseforge_metadata(
    state: &State,
    instance: &Instance,
    files: &[InstanceFile],
    entries_by_file_id: &HashMap<&str, &ContentEntry>,
    file_info_by_hash: &HashMap<String, CachedFile>,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<HashMap<String, crate::state::FileMetadata>> {
    let untracked: Vec<&InstanceFile> = files
        .iter()
        .filter(|file| !file.missing)
        .filter(|file| project_type_for_file(file).is_some())
        .filter(|file| !file_info_by_hash.contains_key(&file.sha1))
        .filter(|file| {
            entries_by_file_id
                .get(file.id.as_str())
                .is_none_or(|entry| entry.cf_project_id.is_none())
        })
        .collect();
    if untracked.is_empty() {
        return Ok(HashMap::new());
    }

    let instance_dir = state.directories.instances_dir().join(&instance.path);
    let mut fingerprints = Vec::with_capacity(untracked.len());
    for file in &untracked {
        let fingerprint =
            tokio::fs::read(instance_dir.join(&file.relative_path))
                .await
                .ok()
                .map(|bytes| crate::util::murmur2::murmur2(&bytes));
        fingerprints.push(fingerprint);
    }
    let mut unique: Vec<u32> = fingerprints.iter().flatten().copied().collect();
    unique.sort_unstable();
    unique.dedup();
    if unique.is_empty() {
        return Ok(HashMap::new());
    }
    let key = unique
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let Some(cached) = CachedEntry::get_curseforge_fingerprints(
        &key,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    else {
        return Ok(HashMap::new());
    };

    let matched = cached
        .data
        .exact_fingerprints
        .iter()
        .zip(cached.data.exact_matches.iter())
        .map(|(fingerprint, fmatch)| {
            (*fingerprint as u32, (fmatch.id, fmatch.file.id))
        })
        .collect::<HashMap<u32, (i64, i64)>>();

    let mut by_hash = HashMap::new();
    for (file, fingerprint) in untracked.into_iter().zip(&fingerprints) {
        let Some(fingerprint) = fingerprint else {
            continue;
        };
        let Some((cf_project_id, cf_version_id)) =
            matched.get(fingerprint).copied()
        else {
            continue;
        };
        by_hash.insert(
            file.sha1.clone(),
            crate::state::FileMetadata {
                project_id: String::new(),
                version_id: String::new(),
                source: Source::CurseForge,
                cf_project_id: Some(cf_project_id),
                cf_version_id: Some(cf_version_id),
            },
        );
    }
    Ok(by_hash)
}

fn is_imported_modpack_scope(link: &InstanceLink) -> bool {
    matches!(
        link,
        InstanceLink::ImportedModpack { .. }
            | InstanceLink::CurseforgeModpack { .. }
    )
}

fn imported_scope_source_kind(link: &InstanceLink) -> ContentSourceKind {
    match link {
        InstanceLink::CurseforgeModpack { .. } => {
            ContentSourceKind::CurseforgeModpack
        }
        _ => ContentSourceKind::ImportedModpack,
    }
}

async fn linked_modpack_ids_for_instance(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<(String, String)>> {
    let link =
        sqlite::instance_rows::get_instance_link(instance_id, pool).await?;
    Ok(linked_modpack_ids(&link))
}

fn linked_modpack_ids(link: &InstanceLink) -> Option<(String, String)> {
    match link {
        InstanceLink::ModrinthModpack {
            project_id,
            version_id,
        } => Some((project_id.clone(), version_id.clone())),
        InstanceLink::ServerProjectModpack {
            content_project_id,
            content_version_id,
            ..
        } => Some((content_project_id.clone(), content_version_id.clone())),
        InstanceLink::ImportedModpack {
            project_id: Some(project_id),
            version_id: Some(version_id),
            ..
        } => Some((project_id.clone(), version_id.clone())),
        InstanceLink::SharedInstance {
            modpack_project_id: Some(project_id),
            modpack_version_id: Some(version_id),
        } => Some((project_id.clone(), version_id.clone())),
        _ => None,
    }
}

fn linked_modpack_source_kind(
    link: &InstanceLink,
) -> Option<ContentSourceKind> {
    match link {
        InstanceLink::ModrinthModpack { .. } => {
            Some(ContentSourceKind::ModrinthModpack)
        }
        InstanceLink::ServerProjectModpack { .. } => {
            Some(ContentSourceKind::ServerProject)
        }
        InstanceLink::SharedInstance {
            modpack_project_id: Some(_),
            modpack_version_id: Some(_),
        } => Some(ContentSourceKind::ModrinthModpack),
        _ => None,
    }
}

fn check_modpack_update(
    installed_version_id: &str,
    installed_version: &Version,
    all_versions: Option<Vec<Version>>,
    preferred_update_channel: ReleaseChannel,
) -> (bool, Option<String>, Option<Version>) {
    let Some(versions) = all_versions else {
        return (false, None, None);
    };
    let installed_channel =
        ReleaseChannel::from_version_type(&installed_version.version_type);
    let effective_channel =
        preferred_update_channel.least_stable(installed_channel);

    for version_types in effective_channel.version_type_fallbacks() {
        if !versions.iter().any(|version| {
            version_types.contains(&version.version_type.as_str())
        }) {
            continue;
        }

        let mut newer_versions = versions
            .iter()
            .filter(|version| {
                version.id != installed_version_id
                    && version.date_published > installed_version.date_published
                    && version_types.contains(&version.version_type.as_str())
            })
            .collect::<Vec<_>>();
        newer_versions
            .sort_by_key(|version| std::cmp::Reverse(version.date_published));

        if let Some(newest) = newer_versions.first() {
            return (true, Some(newest.id.clone()), Some((*newest).clone()));
        }

        return (false, None, None);
    }

    (false, None, None)
}

#[derive(Clone, Debug)]
struct ModpackIdentifiers {
    hashes: HashSet<String>,
    project_ids: HashSet<String>,
}

impl ModpackIdentifiers {
    fn is_modpack_file(
        &self,
        hash: &str,
        file: Option<&CachedFile>,
        entry_project_id: Option<&str>,
    ) -> bool {
        self.hashes.contains(hash)
            || entry_project_id
                .is_some_and(|project_id| self.project_ids.contains(project_id))
            || file
                .is_some_and(|file| self.project_ids.contains(&file.project_id))
    }
}

async fn get_cached_modpack_identifiers(
    version_id: &str,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<Option<ModpackIdentifiers>> {
    let Some(cached) =
        CachedEntry::get_modpack_files(version_id, pool, fetch_semaphore)
            .await?
    else {
        return Ok(None);
    };

    if cached.project_ids.is_empty() {
        return Ok(None);
    }

    Ok(Some(ModpackIdentifiers {
        hashes: cached.file_hashes.into_iter().collect(),
        project_ids: cached.project_ids.into_iter().collect(),
    }))
}

async fn get_modpack_identifiers(
    version_id: &str,
    content_set: &ContentSet,
    pool: &SqlitePool,
    fetch_semaphore: &FetchSemaphore,
) -> crate::Result<ModpackIdentifiers> {
    if let Some(cached) =
        CachedEntry::get_modpack_files(version_id, pool, fetch_semaphore)
            .await?
    {
        if !cached.project_ids.is_empty() {
            return Ok(ModpackIdentifiers {
                hashes: cached.file_hashes.into_iter().collect(),
                project_ids: cached.project_ids.into_iter().collect(),
            });
        }

        let hash_refs = cached
            .file_hashes
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let files =
            CachedEntry::get_file_many(&hash_refs, None, pool, fetch_semaphore)
                .await?;
        let project_ids = files
            .iter()
            .map(|file| file.project_id.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        CachedEntry::cache_modpack_files(
            version_id,
            cached.file_hashes.clone(),
            project_ids.clone(),
            pool,
        )
        .await?;

        return Ok(ModpackIdentifiers {
            hashes: cached.file_hashes.into_iter().collect(),
            project_ids: project_ids.into_iter().collect(),
        });
    }

    let version =
        CachedEntry::get_version(version_id, None, pool, fetch_semaphore)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Modpack version {version_id} not found"
                ))
            })?;
    let primary_file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "No files found for modpack version {version_id}"
            ))
        })?;
    let download_meta = DownloadMeta {
        reason: DownloadReason::Modpack,
        game_version: content_set.game_version.clone(),
        loader: content_set.loader.as_str().to_string(),
        dependent_on: Some(version_id.to_string()),
    };
    let mrpack_file = fetch_file_mirrors(
        &[&primary_file.url],
        primary_file.hashes.get("sha1").map(String::as_str),
        Some(&download_meta),
        None,
        fetch_semaphore,
        pool,
        None,
    )
    .await?;
    let zip_reader =
        ZipFileReader::new(mrpack_file.path()).await.map_err(|_| {
            crate::ErrorKind::InputError(
                "Failed to read modpack zip".to_string(),
            )
        })?;
    let manifest_idx = zip_reader
        .file()
        .entries()
        .iter()
        .position(|file| {
            matches!(file.filename().as_str(), Ok("modrinth.index.json"))
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "No modrinth.index.json found in mrpack".to_string(),
            )
        })?;
    let mut manifest = String::new();
    let mut entry_reader = zip_reader.reader_with_entry(manifest_idx).await?;
    entry_reader.read_to_string_checked(&mut manifest).await?;
    let pack: PackFormat = serde_json::from_str(&manifest)?;
    let mut hashes = pack
        .files
        .iter()
        .filter_map(|file| file.hashes.get(&PackFileHash::Sha1).cloned())
        .collect::<Vec<_>>();
    let project_ids = pack
        .files
        .iter()
        .filter_map(|file| {
            file.downloads.iter().find_map(|url| {
                let parts = url.split('/').collect::<Vec<_>>();
                let data_idx = parts.iter().position(|part| *part == "data")?;
                parts.get(data_idx + 1).map(|part| part.to_string())
            })
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let override_entries = zip_reader
        .file()
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let filename = entry.filename().as_str().ok()?;
            let is_override = (filename.starts_with("overrides/")
                || filename.starts_with("client-overrides/")
                || filename.starts_with("server-overrides/"))
                && !filename.ends_with('/');
            is_override.then_some(index)
        })
        .collect::<Vec<_>>();

    let mut buffer = vec![0_u8; 256 * 1024];
    for index in override_entries {
        let mut reader = zip_reader.reader_with_entry(index).await?;
        let crc32 = reader.entry().crc32();
        let mut hasher = sha1_smol::Sha1::new();
        loop {
            let read =
                futures_lite::io::AsyncReadExt::read(&mut reader, &mut buffer)
                    .await?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        if reader.compute_hash() != crc32 {
            return Err(async_zip::error::ZipError::CRC32CheckError.into());
        }
        hashes.push(hasher.hexdigest());
    }

    CachedEntry::cache_modpack_files(
        version_id,
        hashes.clone(),
        project_ids.clone(),
        pool,
    )
    .await?;

    Ok(ModpackIdentifiers {
        hashes: hashes.into_iter().collect(),
        project_ids: project_ids.into_iter().collect(),
    })
}

fn project_type_from_api_name(project_type: &str) -> ProjectType {
    ProjectType::from_name(project_type).unwrap_or(ProjectType::Mod)
}

fn sort_content_items(items: &mut [ContentItem]) {
    items.sort_by(|left, right| {
        let left_name = left
            .project
            .as_ref()
            .map(|project| project.title.as_str())
            .unwrap_or(&left.file_name);
        let right_name = right
            .project
            .as_ref()
            .map(|project| project.title.as_str())
            .unwrap_or(&right.file_name);

        left_name
            .to_lowercase()
            .cmp(&right_name.to_lowercase())
            .then_with(|| left.file_name.cmp(&right.file_name))
    });
}

async fn get_curseforge_modpack_info(
    cf_project_id: i64,
    cf_file_id: i64,
    game_version: &str,
    loader: &str,
    update_channel: ReleaseChannel,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<LinkedModpackInfo> {
    let source_project = CachedEntry::get_curseforge_project(
        &cf_project_id.to_string(),
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Linked CurseForge modpack {cf_project_id} not found"
        ))
    })?;

    let cf_files = crate::api::curseforge::api::get_mod_files(
        cf_project_id,
        Some(game_version),
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await
    .unwrap_or_default();
    let latest =
        super::check_content_updates::curseforge_latest_compatible_file(
            &cf_files,
            game_version,
            loader,
            update_channel,
        );
    let has_update = latest.map(|file| file.id != cf_file_id).unwrap_or(false);
    let update_version = latest.map(|file| {
        crate::api::curseforge::labrinth_map::cf_file_to_version(file, None)
    });

    Ok(LinkedModpackInfo {
        project: crate::api::curseforge::labrinth_map::project_to_labrinth(
            &source_project,
        ),
        version: None,
        owner: None,
        has_update,
        update_version_id: latest.map(|file| format!("cf-{}", file.id)),
        update_version,
    })
}
