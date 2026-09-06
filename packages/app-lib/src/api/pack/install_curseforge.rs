use crate::api::curseforge::api::{get_download_url, get_mod_file};
use crate::api::curseforge::structs::{
    CF_CLASS_MOD, CF_CLASS_RESOURCE_PACK, CF_CLASS_SHADER_PACK, CFFile,
};
use crate::api::pack::install_from::{
    CreatePack, CreatePackDescription, CreatePackFile, PackDependency,
    PackFile, PackFileHash, PackFormat,
};
use crate::state::{CacheBehaviour, CachedEntry};
use crate::util::fetch::{DownloadMeta, DownloadReason, fetch};
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::Deserialize;
use std::collections::HashMap;

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
    let (loader, version) = id.split_once('-')?;
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
    let folders: HashMap<i64, String> = projects
        .iter()
        .filter_map(|project| {
            folder_for_class(project.class_id)
                .map(|folder| (project.id, folder.to_string()))
        })
        .collect();

    let mut files = Vec::with_capacity(manifest.files.len());
    for manifest_file in &manifest.files {
        let cf_file = get_mod_file(
            manifest_file.file_id,
            &state.api_semaphore,
            &state.pool,
        )
        .await?;
        let folder = folders
            .get(&manifest_file.project_id)
            .cloned()
            .unwrap_or_else(|| "mods".to_string());
        files.push(pack_file_from_cf(
            &cf_file,
            format!("{}/{}", folder, cf_file.file_name),
        ));
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
            version_id: None,
            instance_id,
            source_filename: None,
            curseforge: Some((cf_project_id, cf_file_id)),
        },
    })
}
