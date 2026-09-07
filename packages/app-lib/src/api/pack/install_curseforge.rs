use crate::api::curseforge::api::get_download_url;
use crate::api::curseforge::structs::{
    CF_CLASS_MOD, CF_CLASS_RESOURCE_PACK, CF_CLASS_SHADER_PACK, CFFile,
};
use crate::api::pack::install_from::{
    BlockedFileInfo, CreatePack, CreatePackDescription, CreatePackFile,
    PackDependency, PackFile, PackFileHash, PackFormat,
};
use crate::state::{CacheBehaviour, CachedEntry};
use crate::util::fetch::{DownloadMeta, DownloadReason, fetch};
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::Deserialize;
use std::collections::HashMap;

use reqwest::Method;

#[derive(Deserialize, Debug)]
pub struct CFManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub files: Vec<CFManifestFile>,
    pub minecraft: CFManifestMinecraft,
    #[serde(default)]
    pub overrides: String,
}

#[derive(Deserialize, Debug)]
pub struct CFManifestFile {
    #[serde(rename = "projectID")]
    pub project_id: i64,
    #[serde(rename = "fileID")]
    pub file_id: i64,
}

#[derive(Deserialize, Debug)]
pub struct CFManifestMinecraft {
    pub version: String,
    #[serde(rename = "modLoaders", default)]
    pub mod_loaders: Vec<CFManifestModLoader>,
}

#[derive(Deserialize, Debug)]
pub struct CFManifestModLoader {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

fn loader_dependency(id: &str) -> Option<(PackDependency, String)> {
    let (loader, version) =
        if let Some(version) = id.strip_prefix("quilt-loader-") {
            ("quilt", version)
        } else {
            let (loader, version) = id.split_once('-')?;
            (loader, version)
        };
    match loader {
        "forge" => Some((PackDependency::Forge, version.to_string())),
        "neoforge" => Some((PackDependency::NeoForge, version.to_string())),
        "fabric" => Some((PackDependency::FabricLoader, version.to_string())),
        "quilt" => Some((PackDependency::QuiltLoader, version.to_string())),
        _ => None,
    }
}

fn folder_for_class(class_id: Option<i64>) -> Option<&'static str> {
    match class_id? {
        CF_CLASS_MOD => Some("mods"),
        CF_CLASS_RESOURCE_PACK => Some("resourcepacks"),
        CF_CLASS_SHADER_PACK => Some("shaderpacks"),
        _ => None,
    }
}

fn pack_file_from_cf(file: &CFFile, path: String) -> PackFile {
    PackFile {
        path: SafeRelativeUtf8UnixPathBuf::try_from(path).unwrap_or_else(
            |_| {
                SafeRelativeUtf8UnixPathBuf::try_from(format!(
                    "mods/{}",
                    file.file_name
                ))
                .expect("valid path")
            },
        ),
        hashes: file
            .hashes
            .iter()
            .filter(|hash| hash.algo == 1)
            .map(|hash| (PackFileHash::Sha1, hash.value.clone()))
            .collect(),
        env: None,
        downloads: vec![get_download_url(file)],
        file_size: file.file_length as u32,
        cf_project_id: Some(file.mod_id),
        cf_file_id: Some(file.id),
    }
}

/// Resolves a CurseForge `manifest.json` into the mrpack [`PackFormat`]
/// install pipeline, fetching file metadata from the CurseForge API.
pub async fn pack_from_manifest(
    manifest: &str,
    cf_file_id: Option<i64>,
) -> crate::Result<PackFormat> {
    let manifest: CFManifest = serde_json::from_str(manifest)?;

    let mut dependencies = HashMap::new();
    dependencies.insert(
        PackDependency::Minecraft,
        manifest.minecraft.version.clone(),
    );
    for loader in &manifest.minecraft.mod_loaders {
        if loader.primary {
            if let Some((dependency, version)) = loader_dependency(&loader.id) {
                dependencies.insert(dependency, version);
            }
        }
    }

    let state = crate::State::get().await?;
    let project_ids: Vec<i64> = manifest
        .files
        .iter()
        .map(|file| file.project_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let projects = if project_ids.is_empty() {
        Vec::new()
    } else {
        crate::api::curseforge::api::get_mods(
            &project_ids,
            &state.api_semaphore,
            &state.pool,
        )
        .await?
    };
    let projects_by_id: HashMap<i64, _> = projects
        .iter()
        .map(|project| (project.id, project))
        .collect();
    let folders: HashMap<i64, String> = projects
        .iter()
        .filter_map(|project| {
            folder_for_class(project.class_id)
                .map(|folder| (project.id, folder.to_string()))
        })
        .collect();

    let file_ids: Vec<i64> =
        manifest.files.iter().map(|file| file.file_id).collect();
    let mut resolved_files: HashMap<i64, CFFile> = HashMap::new();
    for chunk in file_ids.chunks(100) {
        let files = crate::api::curseforge::api::get_files(
            chunk,
            &state.api_semaphore,
            &state.pool,
        )
        .await?;
        resolved_files.extend(files.into_iter().map(|file| (file.id, file)));
    }

    let is_blocked = |file: &CFFile| {
        file.download_url.as_ref().is_none_or(|url| url.is_empty())
            || file.is_available == Some(false)
            || projects_by_id
                .get(&file.mod_id)
                .and_then(|project| project.allow_mod_distribution)
                == Some(false)
    };

    // Try to substitute distribution-blocked files with Modrinth
    // equivalents via SHA1 lookup.
    let blocked_sha1s: Vec<String> = manifest
        .files
        .iter()
        .filter_map(|manifest_file| resolved_files.get(&manifest_file.file_id))
        .filter(|file| is_blocked(file))
        .filter_map(|file| {
            file.hashes
                .iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone())
        })
        .collect();
    let modrinth_by_hash: HashMap<String, Vec<crate::state::Version>> =
        if blocked_sha1s.is_empty() {
            HashMap::new()
        } else {
            crate::util::fetch::fetch_json(
                Method::POST,
                &format!("{}/version_files", env!("MODRINTH_API_URL")),
                None,
                Some(serde_json::json!({
                    "algorithm": "sha1",
                    "hashes": blocked_sha1s
                })),
                Some("/v2/version_files"),
                &state.fetch_semaphore,
                &state.pool,
            )
            .await
            .unwrap_or_default()
        };

    let mut files = Vec::with_capacity(manifest.files.len());
    let mut blocked_files = Vec::new();
    for manifest_file in &manifest.files {
        let folder = folders
            .get(&manifest_file.project_id)
            .cloned()
            .unwrap_or_else(|| "mods".to_string());
        let Some(cf_file) = resolved_files.get(&manifest_file.file_id) else {
            return Err(crate::ErrorKind::InputError(format!(
                "Unknown CurseForge file {} in modpack manifest",
                manifest_file.file_id
            ))
            .into());
        };

        if !is_blocked(cf_file) {
            files.push(pack_file_from_cf(
                cf_file,
                format!("{}/{}", folder, cf_file.file_name),
            ));
            continue;
        }

        let sha1 = cf_file
            .hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .map(|hash| hash.value.clone());
        let substitute = sha1.as_ref().and_then(|sha1| {
            modrinth_by_hash
                .get(sha1)?
                .iter()
                .find(|version| {
                    version.game_versions.contains(&manifest.minecraft.version)
                })
                .and_then(|version| {
                    version.files.iter().find(|file| {
                        file.hashes.get("sha1").map(String::as_str)
                            == Some(sha1.as_str())
                    })
                })
        });

        if let Some(substitute) = substitute {
            files.push(PackFile {
                path: SafeRelativeUtf8UnixPathBuf::try_from(format!(
                    "{}/{}",
                    folder, substitute.filename
                ))
                .unwrap_or_else(|_| {
                    SafeRelativeUtf8UnixPathBuf::try_from(format!(
                        "mods/{}",
                        substitute.filename
                    ))
                    .expect("valid path")
                }),
                hashes: substitute
                    .hashes
                    .get("sha1")
                    .map(|sha1| (PackFileHash::Sha1, sha1.clone()))
                    .into_iter()
                    .collect(),
                env: None,
                downloads: vec![substitute.url.clone()],
                file_size: substitute.size,
                cf_project_id: None,
                cf_file_id: None,
            });
            continue;
        }

        let project = projects_by_id.get(&manifest_file.project_id);
        blocked_files.push(BlockedFileInfo {
            project_id: manifest_file.project_id,
            project_name: project
                .map(|project| project.name.clone())
                .unwrap_or_else(|| manifest_file.project_id.to_string()),
            file_name: cf_file.file_name.clone(),
            url: project
                .and_then(|project| project.links.website_url.clone())
                .filter(|url| !url.is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "https://www.curseforge.com/projects/{}",
                        manifest_file.project_id
                    )
                }),
        });
    }

    Ok(PackFormat {
        game: "minecraft".to_string(),
        format_version: 1,
        version_id: cf_file_id
            .map(|file_id| format!("cf-{file_id}"))
            .unwrap_or_else(|| manifest.version.clone()),
        name: manifest.name.clone(),
        summary: None,
        files,
        dependencies,
        blocked_files,
    })
}

/// Downloads a CurseForge modpack file and prepares it for the shared
/// zipped-pack install pipeline.
pub async fn generate_pack_from_curseforge(
    cf_project_id: i64,
    cf_file_id: i64,
    title: String,
    icon_url: Option<String>,
    instance_id: String,
    reason: DownloadReason,
) -> crate::Result<CreatePack> {
    let state = crate::State::get().await?;

    let cf_file = CachedEntry::get_curseforge_file(
        &cf_file_id.to_string(),
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unknown CurseForge file {cf_file_id}"
        ))
    })?;

    let metadata =
        crate::api::instance::get(&instance_id)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown instance {instance_id}"
                ))
            })?;
    let download_meta = DownloadMeta {
        reason,
        game_version: metadata.applied_content_set.game_version.clone(),
        loader: metadata.applied_content_set.loader.as_str().to_string(),
        dependent_on: Some(format!("cf-{cf_file_id}")),
    };

    let bytes = fetch(
        &cf_file.url,
        cf_file.sha1.as_deref(),
        Some(&download_meta),
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    if let Some(icon_url) = &icon_url {
        let icon_bytes = fetch(
            icon_url,
            None,
            None,
            None,
            &state.fetch_semaphore,
            &state.pool,
        )
        .await?;
        let icon = crate::api::instance::cache_icon(icon_bytes, &state).await?;
        let _ =
            crate::api::instance::edit_icon(&instance_id, Some(&icon)).await;
    }

    Ok(CreatePack {
        file: CreatePackFile::Bytes(bytes),
        description: CreatePackDescription {
            icon: None,
            override_title: Some(title),
            project_id: None,
            version_id: Some(format!("cf-{cf_file_id}")),
            instance_id,
            source_filename: None,
            curseforge: Some((cf_project_id, cf_file_id)),
        },
    })
}
