//! Normalized cross-source DTOs
//!
//! CurseForge entities are mapped into these shapes so the frontend can render
//! either source through the same components. Modrinth data keeps using its
//! native types; these DTOs exist primarily for CurseForge.

use serde::{Deserialize, Serialize};

use super::structs::{
    CF_CLASS_DATA_PACK, CF_CLASS_MODPACK, CF_CLASS_RESOURCE_PACK,
    CF_CLASS_SHADER_PACK, CFAuthor, CFFile, CFFileDependency, CFProject,
};

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    #[default]
    Modrinth,
    #[serde(rename = "curseforge")]
    CurseForge,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::CurseForge => "curseforge",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "curseforge" => Self::CurseForge,
            _ => Self::Modrinth,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SourceProjectType {
    Mod,
    Modpack,
    ResourcePack,
    ShaderPack,
    DataPack,
}

impl SourceProjectType {
    pub fn from_cf_class(class_id: Option<i64>) -> Self {
        match class_id {
            Some(CF_CLASS_MODPACK) => Self::Modpack,
            Some(CF_CLASS_RESOURCE_PACK) => Self::ResourcePack,
            Some(CF_CLASS_SHADER_PACK) => Self::ShaderPack,
            Some(CF_CLASS_DATA_PACK) => Self::DataPack,
            _ => Self::Mod,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceGalleryItem {
    pub url: String,
    pub featured: bool,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceProject {
    pub source: Source,
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    pub description: String,
    pub body_url: String,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub downloads: u64,
    pub updated: Option<String>,
    pub date_created: Option<String>,
    pub website_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub authors: Vec<CFAuthor>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    pub project_type: SourceProjectType,
    pub gallery: Vec<SourceGalleryItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceVersionFile {
    pub id: String,
    pub filename: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub url: String,
    pub primary: bool,
    pub size: u64,
    pub sha1: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceVersion {
    pub source: Source,
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    pub changelog: Option<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: Option<String>,
    pub version_type: String,
    pub files: Vec<SourceVersionFile>,
    pub dependencies: Vec<CFFileDependency>,
    #[serde(default)]
    pub downloads: u64,
}

impl SourceProject {
    pub fn from_cf(project: CFProject) -> Self {
        let project_type = SourceProjectType::from_cf_class(project.class_id);
        Self {
            source: Source::CurseForge,
            id: project.id.to_string(),
            slug: project.slug,
            title: project.name,
            description: project.summary,
            body_url: project.links.website_url.clone().unwrap_or_default(),
            icon_url: project.logo.url.clone().or(project.logo.thumbnail_url),
            categories: project
                .categories
                .iter()
                .map(|category| category.name.clone())
                .collect(),
            downloads: project.download_count,
            updated: project.date_modified,
            date_created: project.date_created,
            website_url: project.links.website_url,
            issues_url: project.links.issues_url,
            source_url: project.links.source_url,
            wiki_url: project.links.wiki_url,
            authors: project.authors,
            project_type,
            game_versions: {
                let mut versions: Vec<String> = project
                    .latest_files_indexes
                    .iter()
                    .filter_map(|index| index.game_version.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                versions.sort();
                versions
            },
            loaders: match project_type {
                SourceProjectType::Mod | SourceProjectType::Modpack => {
                    let mut loaders: Vec<String> = project
                        .latest_files_indexes
                        .iter()
                        .filter_map(|index| {
                            loader_name(index.mod_loader?).map(String::from)
                        })
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();
                    loaders.sort();
                    loaders
                }
                SourceProjectType::ShaderPack => vec!["shader".to_string()],
                SourceProjectType::DataPack => vec!["datapack".to_string()],
                SourceProjectType::ResourcePack => Vec::new(),
            },
            gallery: project
                .screenshots
                .iter()
                .enumerate()
                .map(|(index, screenshot)| SourceGalleryItem {
                    url: screenshot
                        .url
                        .clone()
                        .or_else(|| screenshot.thumbnail_url.clone())
                        .unwrap_or_default(),
                    featured: index == 0,
                    title: screenshot.title.clone(),
                    description: screenshot.description.clone(),
                })
                .collect(),
        }
    }
}

impl SourceVersionFile {
    pub fn from_cf(file: &CFFile) -> Self {
        Self {
            id: file.id.to_string(),
            filename: file.file_name.clone(),
            display_name: Some(file.display_name.clone()),
            url: super::api::get_download_url(file),
            primary: true,
            size: file.file_length,
            sha1: file
                .hashes
                .iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone()),
        }
    }
}

impl SourceVersion {
    pub fn from_cf(file: CFFile, changelog: Option<String>) -> Self {
        let (game_versions, mut loaders) =
            split_file_game_data(&file.game_versions);
        for loader in &file.loaders {
            let loader = loader.to_ascii_lowercase();
            if !loaders.contains(&loader) {
                loaders.push(loader);
            }
        }
        Self {
            source: Source::CurseForge,
            id: file.id.to_string(),
            project_id: file.mod_id.to_string(),
            version_number: file.display_name.clone(),
            changelog,
            game_versions,
            loaders,
            date_published: file.file_date.clone(),
            version_type: release_type_name(file.release_type).to_string(),
            files: vec![SourceVersionFile::from_cf(&file)],
            dependencies: file.dependencies.clone(),
            downloads: file.download_count,
        }
    }
}

/// Splits a CF file's `game_versions` list — which mixes loader names into
/// the versions, since CF files have no dedicated loaders field — into
/// `(game_versions_without_loaders, loaders)`. Loader names are matched
/// case-insensitively and returned lowercased; other entries keep their
/// original order.
pub fn split_file_game_data(
    game_versions: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut versions = Vec::with_capacity(game_versions.len());
    let mut loaders = Vec::new();
    for version in game_versions {
        let lower = version.to_ascii_lowercase();
        if matches!(lower.as_str(), "forge" | "neoforge" | "fabric" | "quilt") {
            loaders.push(lower);
        } else {
            versions.push(version.clone());
        }
    }
    (versions, loaders)
}

/// Parses a CurseForge date (RFC 3339 string or epoch milliseconds) to epoch
/// milliseconds, for sorting. Unparseable dates sort oldest.
pub fn parse_cf_date(date: &Option<String>) -> i64 {
    match date {
        Some(date) => {
            if let Ok(millis) = date.parse::<i64>() {
                millis
            } else if let Ok(date_time) =
                chrono::DateTime::parse_from_rfc3339(date)
            {
                date_time.timestamp_millis()
            } else if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(
                date,
                "%Y-%m-%dT%H:%M:%S%.f",
            ) {
                naive.and_utc().timestamp_millis()
            } else {
                0
            }
        }
        None => 0,
    }
}

pub fn release_type_name(release_type: i32) -> &'static str {
    match release_type {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    }
}

pub fn loader_name(mod_loader: i64) -> Option<&'static str> {
    match mod_loader {
        1 => Some("forge"),
        4 => Some("fabric"),
        5 => Some("quilt"),
        6 => Some("neoforge"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_project() -> serde_json::Value {
        serde_json::json!({
            "id": 123,
            "gameId": 432,
            "name": "Test Mod",
            "slug": "test-mod",
            "summary": "A test",
            "downloadCount": 100,
            "dateCreated": "2020-01-01T00:00:00Z",
            "dateModified": "2021-01-01T00:00:00Z",
            "links": { "websiteUrl": "https://example.com" },
            "logo": { "thumbnailUrl": "https://example.com/icon.png" },
            "categories": [{ "id": 1, "name": "Adventure" }],
            "classId": 6
        })
    }

    #[test]
    fn project_mapping() {
        let project: CFProject =
            serde_json::from_value(sample_project()).unwrap();
        let normalized = SourceProject::from_cf(project);
        assert_eq!(normalized.source, Source::CurseForge);
        assert_eq!(normalized.id, "123");
        assert_eq!(normalized.title, "Test Mod");
        assert_eq!(normalized.project_type, SourceProjectType::Mod);
        assert_eq!(
            normalized.website_url.as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn file_mapping() {
        let file: CFFile = serde_json::from_value(serde_json::json!({
            "id": 456,
            "gameId": 432,
            "modId": 123,
            "displayName": "1.0.0",
            "fileName": "mod-1.0.0.jar",
            "releaseType": 1,
            "fileLength": 1024,
            "fileDate": "2021-01-01T00:00:00Z",
            "hashes": [{ "value": "abc", "algo": 1 }],
            "gameVersions": ["1.21.1"],
            "loaders": ["forge"],
            "dependencies": [{ "modId": 789, "relationType": 3 }]
        }))
        .unwrap();
        let version = SourceVersion::from_cf(file, None);
        assert_eq!(version.id, "456");
        assert_eq!(version.version_type, "release");
        assert_eq!(version.files.len(), 1);
        assert_eq!(
            version.files[0].url,
            "https://edge.forgecdn.net/files/0/456/mod-1.0.0.jar"
        );
        assert_eq!(version.files[0].sha1.as_deref(), Some("abc"));
        assert_eq!(version.dependencies.len(), 1);
        assert_eq!(version.dependencies[0].mod_id, 789);
        assert_eq!(version.dependencies[0].relation_type, 3);
        assert_eq!(version.game_versions, vec!["1.21.1".to_string()]);
        assert_eq!(version.loaders, vec!["forge".to_string()]);
    }

    #[test]
    fn file_game_data_split() {
        let (versions, loaders) = split_file_game_data(&[
            "1.21.1".to_string(),
            "Forge".to_string(),
            "NeoForge".to_string(),
            "1.20.1".to_string(),
            "fabric".to_string(),
        ]);
        assert_eq!(versions, vec!["1.21.1".to_string(), "1.20.1".to_string()]);
        assert_eq!(
            loaders,
            vec![
                "forge".to_string(),
                "neoforge".to_string(),
                "fabric".to_string()
            ]
        );
    }
}
