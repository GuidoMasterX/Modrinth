use crate::state::instances::{
    ContentEntry, InstanceFile,
    adapters::sqlite::{content_rows, instance_rows},
};
use crate::state::{
    CacheBehaviour, CachedEntry, ProjectType, ReleaseChannel, State,
};
use std::collections::HashMap;

use super::sync_content_files::{
    project_type_for_file, sync_instance_content_files,
};

#[derive(Clone, Debug)]
pub(crate) struct ContentUpdate {
    pub relative_path: String,
    pub current_version_id: String,
    pub update_version_id: String,
}

#[derive(Clone, Debug)]
struct UpdateCandidate {
    entry: Option<ContentEntry>,
    file: InstanceFile,
    project_type: ProjectType,
    current_version_id: String,
}

pub(crate) async fn check_content_updates(
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentUpdate>> {
    check_content_updates_with_cache_behaviours(
        instance_id,
        cache_behaviour,
        cache_behaviour,
        cache_behaviour,
        state,
    )
    .await
}

pub(crate) async fn refresh_content_updates(
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<()> {
    // A failed Modrinth lookup must stay detectable: with
    // stale-while-revalidate its error is swallowed and the missing keys look
    // like "no update", which is what cleared every Modrinth badge on refresh.
    // Failures are non-destructive now, so both sources revalidate when asked.
    // CurseForge costs one batched request for the whole instance.
    check_content_updates_with_cache_behaviours(
        instance_id,
        None,
        Some(cache_behaviour.unwrap_or(CacheBehaviour::MustRevalidate)),
        Some(cache_behaviour.unwrap_or(CacheBehaviour::MustRevalidate)),
        state,
    )
    .await?;

    Ok(())
}

async fn check_content_updates_with_cache_behaviours(
    instance_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
    update_cache_behaviour: Option<CacheBehaviour>,
    curseforge_cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<Vec<ContentUpdate>> {
    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;
    let content_set =
        content_rows::get_applied_content_set(&instance.id, &state.pool)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Instance {} has no applied content set",
                    instance.id
                ))
            })?;
    let mut entries =
        content_rows::get_content_entries(&content_set.id, &state.pool).await?;
    let mut entries_by_file_id = entries
        .iter()
        .filter_map(|entry| {
            entry.file_id.as_deref().map(|file_id| (file_id, entry))
        })
        .collect::<HashMap<_, _>>();
    let skips_by_entry_id =
        content_rows::get_content_update_skips_for_content_set(
            &content_set.id,
            &state.pool,
        )
        .await?;
    let files = sync_instance_content_files(&instance, state).await?;
    let hashes = files
        .iter()
        .map(|file| file.sha1.as_str())
        .collect::<Vec<_>>();
    // Modrinth enrichment must degrade, never abort: a throttled metadata
    // lookup should still leave the CurseForge results below intact.
    let file_info = match CachedEntry::get_file_many(
        &hashes,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    {
        Ok(file_info) => file_info,
        Err(err) => {
            tracing::warn!(
                "Unable to fetch Modrinth file metadata: {err}; continuing \
                 without it"
            );
            Vec::new()
        }
    };
    let file_info_by_hash = file_info
        .into_iter()
        .map(|file| (file.hash.clone(), file))
        .collect::<HashMap<_, _>>();
    // Files that exist on disk without a tracked entry never receive update
    // checks. CurseForge fingerprints identify them exactly, so adopt them as
    // tracked entries before checking (the listing already displays them as
    // CurseForge projects).
    if !file_info_by_hash.is_empty() {
        let cf_metadata_by_hash =
            match super::list_content::detect_curseforge_metadata(
                state,
                &instance,
                &files,
                &entries_by_file_id,
                &file_info_by_hash,
                cache_behaviour,
            )
            .await
            {
                Ok(metadata) => metadata,
                Err(err) => {
                    tracing::warn!(
                        "Unable to match untracked CurseForge files: {err}"
                    );
                    HashMap::new()
                }
            };
        if !cf_metadata_by_hash.is_empty() {
            match super::apply_content_install::track_curseforge_files(
                &instance.id,
                &files,
                &entries_by_file_id,
                &cf_metadata_by_hash,
                state,
            )
            .await
            {
                Ok(tracked) if tracked > 0 => {
                    entries = content_rows::get_content_entries(
                        &content_set.id,
                        &state.pool,
                    )
                    .await?;
                    entries_by_file_id = entries
                        .iter()
                        .filter_map(|entry| {
                            entry
                                .file_id
                                .as_deref()
                                .map(|file_id| (file_id, entry))
                        })
                        .collect();
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(
                        "Unable to track matched CurseForge files: {err}"
                    );
                }
            }
        }
    }
    // CurseForge's update check costs one batched request for the whole
    // instance, so it can honor the caller's cache behaviour directly.
    let curseforge_updates = check_curseforge_content_updates(
        instance.update_channel,
        &content_set.game_version,
        content_set.loader.as_str(),
        &files,
        &entries_by_file_id,
        &skips_by_entry_id,
        state,
        curseforge_cache_behaviour,
    )
    .await
    .unwrap_or_else(|err| {
        tracing::warn!("CurseForge update check failed: {err}");
        Vec::new()
    });
    let candidates = files
        .into_iter()
        .filter_map(|file| {
            let project_type = project_type_for_file(&file)?;
            let metadata = file_info_by_hash.get(&file.sha1)?;
            let entry = entries_by_file_id
                .get(file.id.as_str())
                .copied()
                .cloned();
            // A tracked entry has exactly one source of truth: CurseForge
            // entries are resolved by `check_curseforge_content_updates`, so
            // never let a Modrinth hash hit (many CurseForge files are
            // mirrored on Modrinth) overwrite that row here.
            if entry
                .as_ref()
                .is_some_and(|entry| {
                    entry.source == crate::api::curseforge::normalize::Source::CurseForge
                })
            {
                return None;
            }
            Some(UpdateCandidate {
                entry,
                file,
                project_type,
                current_version_id: metadata.version_id.clone(),
            })
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() && curseforge_updates.is_empty() {
        return Ok(Vec::new());
    }

    let installed_channels =
        match installed_update_channels(&candidates, cache_behaviour, state)
            .await
        {
            Ok(channels) => channels,
            Err(err) => {
                tracing::warn!(
                    "Unable to fetch installed update channels: {err}; using \
                     instance defaults"
                );
                HashMap::new()
            }
        };
    let update_keys = candidates
        .iter()
        .map(|candidate| {
            update_cache_key(
                &candidate.file,
                candidate.project_type,
                effective_update_channel(
                    instance.update_channel,
                    installed_channels.get(&candidate.file.sha1).copied(),
                ),
                &content_set.game_version,
                content_set.loader.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let update_key_refs = update_keys
        .iter()
        .map(|key| key.as_str())
        .collect::<Vec<_>>();
    // A failed Modrinth lookup means "unknown", not "no update": leave every
    // stored row and badge untouched instead of clearing them. Only a
    // successful response may write the (possibly empty) update result.
    let updates = match CachedEntry::get_file_update_many(
        &update_key_refs,
        update_cache_behaviour,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    {
        Ok(updates) => Some(updates),
        Err(err) => {
            tracing::warn!(
                "Unable to fetch Modrinth version updates: {err}; leaving \
                 stored update state alone this cycle"
            );
            None
        }
    };

    let mut output = Vec::new();
    if let Some(updates) = updates {
        let mut updates_by_hash: HashMap<String, Vec<String>> = HashMap::new();
        for update in updates {
            updates_by_hash
                .entry(update.hash)
                .or_default()
                .push(update.update_version_id);
        }

        for candidate in candidates {
            let update_version_id = updates_by_hash
                .remove(&candidate.file.sha1)
                .unwrap_or_default()
                .into_iter()
                .find(|update_version_id| {
                    update_version_id != &candidate.current_version_id
                });

            if let Some(entry) = &candidate.entry {
                content_rows::upsert_content_update_check(
                    &entry.id,
                    instance.update_channel,
                    update_version_id.as_deref(),
                    &state.pool,
                )
                .await?;
            }

            if let Some(update_version_id) = update_version_id {
                let suppressed = candidate
                    .entry
                    .as_ref()
                    .is_some_and(|entry| {
                        entry.updates_ignored
                            || skips_by_entry_id
                                .get(entry.id.as_str())
                                .and_then(|skipped| skipped.as_deref())
                                == Some(update_version_id.as_str())
                    });
                if suppressed {
                    continue;
                }
                output.push(ContentUpdate {
                    relative_path: candidate.file.relative_path,
                    current_version_id: candidate.current_version_id,
                    update_version_id,
                });
            }
        }
    }
    output.extend(curseforge_updates);

    Ok(output)
}

pub(crate) fn curseforge_latest_compatible_file<'a>(
    files: &'a [crate::api::curseforge::structs::CFFile],
    game_version: &str,
    loader: &str,
    update_channel: ReleaseChannel,
) -> Option<&'a crate::api::curseforge::structs::CFFile> {
    use crate::api::curseforge::normalize::parse_cf_date;
    let allowed_release_types = match update_channel {
        ReleaseChannel::Release => 1..=1,
        ReleaseChannel::Beta => 1..=2,
        ReleaseChannel::Alpha => 1..=3,
    };
    files
        .iter()
        .filter(|file| {
            file.game_versions
                .iter()
                .any(|version| version == game_version)
        })
        .filter(|file| {
            let (_, game_version_loaders) =
                crate::api::curseforge::normalize::split_file_game_data(
                    &file.game_versions,
                );
            game_version_loaders
                .iter()
                .chain(file.loaders.iter())
                .any(|candidate| candidate.eq_ignore_ascii_case(loader))
        })
        .filter(|file| allowed_release_types.contains(&file.release_type))
        .max_by_key(|file| parse_cf_date(&file.file_date))
}

pub(crate) fn curseforge_loader_code(loader: &str) -> Option<i64> {
    match loader.to_ascii_lowercase().as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

/// Picks the newest file for the instance's game version, loader and update
/// channel from the cached CurseForge `latestFilesIndexes`. Index entries may
/// reference files that are no longer part of `latest_files` (per-release-type
/// entries go stale), so the cached value carries resolved metadata for every
/// index entry.
pub(crate) fn curseforge_latest_compatible_latest<'a>(
    latest: &'a crate::state::CachedCFProjectLatest,
    game_version: &str,
    loader_code: i64,
    update_channel: ReleaseChannel,
) -> Option<&'a crate::api::curseforge::structs::CFFile> {
    use crate::api::curseforge::normalize::parse_cf_date;
    let allowed_release_types = match update_channel {
        ReleaseChannel::Release => 1..=1,
        ReleaseChannel::Beta => 1..=2,
        ReleaseChannel::Alpha => 1..=3,
    };
    latest
        .latest_files_indexes
        .iter()
        .filter(|index| index.game_version.as_deref() == Some(game_version))
        .filter(|index| match index.mod_loader {
            None | Some(0) => true,
            Some(loader) => loader == loader_code,
        })
        .filter(|index| {
            allowed_release_types.contains(&index.release_type.unwrap_or(1))
        })
        .filter_map(|index| {
            latest.files.iter().find(|file| file.id == index.file_id)
        })
        .max_by_key(|file| parse_cf_date(&file.file_date))
}

async fn check_curseforge_content_updates(
    update_channel: ReleaseChannel,
    game_version: &str,
    loader: &str,
    files: &[InstanceFile],
    entries_by_file_id: &HashMap<&str, &ContentEntry>,
    skips_by_entry_id: &HashMap<String, Option<String>>,
    state: &State,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<ContentUpdate>> {
    use crate::api::curseforge::normalize::Source;

    let mut project_entries: HashMap<i64, Vec<(&InstanceFile, &ContentEntry)>> =
        HashMap::new();
    for file in files {
        if let Some(entry) = entries_by_file_id.get(file.id.as_str()) {
            if entry.source == Source::CurseForge {
                if let Some(cf_project_id) = entry.cf_project_id {
                    project_entries
                        .entry(cf_project_id)
                        .or_default()
                        .push((file, entry));
                }
            }
        }
    }

    if project_entries.is_empty() {
        return Ok(Vec::new());
    }

    let Some(loader_code) = curseforge_loader_code(loader) else {
        return Ok(Vec::new());
    };

    let cf_project_ids = project_entries
        .keys()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    let cf_project_id_refs = cf_project_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let latest_by_project =
        match CachedEntry::get_curseforge_project_latest_many(
            &cf_project_id_refs,
            cache_behaviour,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        {
            Ok(cached) => cached
                .into_iter()
                .map(|entry| (entry.project_id.clone(), entry))
                .collect::<HashMap<String, crate::state::CachedCFProjectLatest>>(),
            Err(err) => {
                tracing::warn!(
                    "Failed to fetch CurseForge project files for update \
                     checks: {err}"
                );
                HashMap::new()
            }
        };

    let mut output = Vec::new();
    for (cf_project_id, pairs) in project_entries {
        let Some(latest) = latest_by_project.get(&cf_project_id.to_string())
        else {
            // The fetch failed for this project: leave any previously stored
            // check alone rather than reporting a false "no update".
            continue;
        };
        let latest = curseforge_latest_compatible_latest(
            latest,
            game_version,
            loader_code,
            update_channel,
        );
        for (file, entry) in pairs {
            let Some(cf_version_id) = entry.cf_version_id else {
                continue;
            };
            let update_version_id = latest.and_then(|latest| {
                if latest.id == cf_version_id {
                    None
                } else {
                    Some(format!("cf-{}", latest.id))
                }
            });
            let suppressed = update_version_id.as_deref().is_some_and(
                |update_version_id| {
                    entry.updates_ignored
                        || skips_by_entry_id
                            .get(entry.id.as_str())
                            .and_then(|skipped| skipped.as_deref())
                            == Some(update_version_id)
                },
            );
            content_rows::upsert_content_update_check(
                &entry.id,
                update_channel,
                update_version_id.as_deref(),
                &state.pool,
            )
            .await?;
            let Some(update_version_id) = update_version_id else {
                continue;
            };
            if suppressed {
                continue;
            }
            output.push(ContentUpdate {
                relative_path: file.relative_path.clone(),
                current_version_id: format!("cf-{cf_version_id}"),
                update_version_id,
            });
        }
    }

    Ok(output)
}

async fn installed_update_channels(
    candidates: &[UpdateCandidate],
    cache_behaviour: Option<CacheBehaviour>,
    state: &State,
) -> crate::Result<HashMap<String, ReleaseChannel>> {
    let version_ids = candidates
        .iter()
        .map(|candidate| candidate.current_version_id.as_str())
        .collect::<Vec<_>>();
    let versions = CachedEntry::get_version_many(
        &version_ids,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
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

    Ok(candidates
        .iter()
        .filter_map(|candidate| {
            channels_by_version_id
                .get(&candidate.current_version_id)
                .copied()
                .map(|channel| (candidate.file.sha1.clone(), channel))
        })
        .collect())
}

fn effective_update_channel(
    preferred: ReleaseChannel,
    installed: Option<ReleaseChannel>,
) -> ReleaseChannel {
    installed.map_or(preferred, |channel| preferred.least_stable(channel))
}

fn update_cache_key(
    file: &InstanceFile,
    project_type: ProjectType,
    channel: ReleaseChannel,
    game_version: &str,
    loader: &str,
) -> String {
    format!(
        "{}-{}-{}-{}",
        file.sha1,
        if project_type == ProjectType::Mod {
            loader.to_string()
        } else {
            project_type.get_loaders().join("+")
        },
        channel.key(),
        game_version
    )
}
