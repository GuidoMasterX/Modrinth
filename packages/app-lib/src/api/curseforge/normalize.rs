//! Normalized cross-source DTOs
//!
//! CurseForge entities are mapped into these shapes so the frontend can render
//! either source through the same components. Modrinth data keeps using its
//! native types; these DTOs exist primarily for CurseForge.

use serde::{Deserialize, Serialize};

use super::structs::{
    CF_CLASS_MODPACK, CF_CLASS_RESOURCE_PACK, CF_CLASS_SHADER_PACK, CFFile,
    CFProject,
};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Modrinth,
    CurseForge,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SourceProjectType {
    Mod,
    Modpack,
    ResourcePack,
    ShaderPack,
}

impl SourceProjectType {
    pub fn from_cf_class(class_id: Option<i64>) -> Self {
        match class_id {
            Some(CF_CLASS_MODPACK) => Self::Modpack,
            Some(CF_CLASS_RESOURCE_PACK) => Self::ResourcePack,
            Some(CF_CLASS_SHADER_PACK) => Self::ShaderPack,
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
    pub project_type: SourceProjectType,
    pub gallery: Vec<SourceGalleryItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceVersionFile {
    pub id: String,
    pub filename: String,
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
    pub dependencies: Vec<String>,
}

impl SourceProject {
    pub fn from_cf(project: CFProject) -> Self {
        Self {
            source: Source::CurseForge,
            id: project.id.to_string(),
            slug: project.slug,
            title: project.name,
            description: project.summary,
            body_url: project.links.website_url.clone().unwrap_or_default(),
            icon_url: project.logo.thumbnail_url.or(project.logo.url),
            categories: project
                .categories
                .iter()
                .map(|category| category.name.clone())
                .collect(),
            downloads: project.download_count,
            updated: project.date_modified,
            date_created: project.date_created,
            website_url: project.links.website_url,
            project_type: SourceProjectType::from_cf_class(project.class_id),
            gallery: Vec::new(),
        }
    }
}

impl SourceVersionFile {
    pub fn from_cf(file: &CFFile) -> Self {
        Self {
            id: file.id.to_string(),
            filename: file.file_name.clone(),
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
        Self {
            source: Source::CurseForge,
            id: file.id.to_string(),
            project_id: file.mod_id.to_string(),
            version_number: file.display_name.clone(),
            changelog,
            game_versions: file.game_versions.clone(),
            loaders: file.loaders.clone(),
            date_published: file.file_date.clone(),
            version_type: release_type_name(file.release_type).to_string(),
            files: vec![SourceVersionFile::from_cf(&file)],
            dependencies: file
                .dependencies
                .iter()
                .map(|dependency| dependency.mod_id.to_string())
                .collect(),
        }
    }
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
            } else {
                0
            }
        }
        None => 0,
    }
}

fn release_type_name(release_type: i32) -> &'static str {
    match release_type {
        2 => "beta",
        3 => "alpha",
        _ => "release",
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
        assert_eq!(version.dependencies, vec!["789".to_string()]);
    }
}
