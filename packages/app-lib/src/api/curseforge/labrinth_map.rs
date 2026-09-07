//! Maps normalized CurseForge DTOs into the Labrinth shapes the UI already
//! consumes. This is the single source of truth for CF -> Labrinth mapping;
//! the frontend receives Modrinth-shaped data directly from the Tauri
//! commands and never converts CF payloads itself.

use chrono::DateTime;
use serde::Serialize;
use serde_json::{Value, json};

use crate::state::{
    Dependency, DependencyType, License, Project, SideType, Version,
    VersionFile,
};

use super::api::get_download_url;
use super::normalize::{
    SourceProject, SourceProjectType, SourceVersion, SourceVersionFile,
    parse_cf_date,
};
use super::structs::{CFFile, CFProject, CFSearchResponse};

/// Serializes to the shape the shared browse UI expects for a search response
/// (`projectHits` / `serverHits` / `total_hits` / `per_page`).
#[derive(Serialize, Clone, Debug)]
pub struct BrowseSearchResponse {
    #[serde(rename = "projectHits")]
    pub project_hits: Vec<Value>,
    #[serde(rename = "serverHits")]
    pub server_hits: Vec<Value>,
    #[serde(rename = "total_hits")]
    pub total_hits: u32,
    #[serde(rename = "per_page")]
    pub per_page: u32,
}

pub fn project_type_str(project_type: SourceProjectType) -> &'static str {
    match project_type {
        SourceProjectType::Mod => "mod",
        SourceProjectType::Modpack => "modpack",
        SourceProjectType::ResourcePack => "resourcepack",
        SourceProjectType::ShaderPack => "shader",
        SourceProjectType::DataPack => "datapack",
    }
}
pub fn project_to_labrinth(project: &SourceProject) -> Project {
    Project {
        id: format!("cf-{}", project.id),
        slug: project.slug.clone(),
        project_type: project_type_str(project.project_type).to_string(),
        team: String::new(),
        organization: None,
        title: project.title.clone(),
        description: project.description.clone(),
        body: project.description.clone(),
        published: DateTime::from_timestamp_millis(parse_cf_date(
            &project.date_created,
        ))
        .unwrap_or_default(),
        updated: DateTime::from_timestamp_millis(parse_cf_date(
            &project.updated,
        ))
        .unwrap_or_default(),
        approved: None,
        status: "approved".to_string(),
        license: License {
            id: "LicenseRef-All-Rights-Reserved".to_string(),
            name: "All Rights Reserved".to_string(),
            url: None,
        },
        client_side: SideType::Unknown,
        server_side: SideType::Unknown,
        downloads: project.downloads as u32,
        followers: 0,
        categories: project.categories.clone(),
        additional_categories: Vec::new(),
        game_versions: project.game_versions.clone(),
        loaders: project.loaders.clone(),
        versions: Vec::new(),
        icon_url: project.icon_url.clone(),
        raw_icon_url: None,
        issues_url: project.issues_url.clone(),
        source_url: project.source_url.clone(),
        wiki_url: project.wiki_url.clone(),
        discord_url: None,
        donation_urls: Some(Vec::new()),
        gallery: project
            .gallery
            .iter()
            .enumerate()
            .map(|(index, item)| crate::state::GalleryItem {
                url: item.url.clone(),
                raw_url: item.url.clone(),
                featured: item.featured,
                title: item.title.clone(),
                description: item.description.clone(),
                created: DateTime::from_timestamp_millis(parse_cf_date(
                    &project.updated,
                ))
                .unwrap_or_default(),
                ordering: index as i64,
            })
            .collect(),
        color: None,
    }
}

/// The canonical CurseForge page for a project. CF's `websiteUrl` is
/// author-controlled and may point off-site, so construct from the class
/// when possible.
pub fn canonical_project_url(project: &SourceProject) -> String {
    let class_path = match project.project_type {
        SourceProjectType::Mod => "mc-mods",
        SourceProjectType::Modpack => "modpacks",
        SourceProjectType::ResourcePack => "texture-packs",
        SourceProjectType::ShaderPack => "shaders",
        SourceProjectType::DataPack => "customization",
    };
    let slug = project.slug.clone().unwrap_or_else(|| project.id.clone());
    let constructed =
        format!("https://www.curseforge.com/minecraft/{class_path}/{slug}");
    let website = project.website_url.as_deref().unwrap_or_default();
    if website.starts_with("https://www.curseforge.com/") {
        website.to_string()
    } else {
        constructed
    }
}

pub fn version_to_labrinth(version: &SourceVersion) -> Version {
    Version {
        id: format!("cf-{}", version.id),
        project_id: format!("cf-{}", version.project_id),
        author_id: String::new(),
        featured: false,
        name: version.version_number.clone(),
        version_number: version.version_number.clone(),
        changelog: version.changelog.clone(),
        changelog_url: None,
        date_published: DateTime::from_timestamp_millis(parse_cf_date(
            &version.date_published,
        ))
        .unwrap_or_default(),
        downloads: version.downloads as u32,
        version_type: version.version_type.clone(),
        files: version
            .files
            .iter()
            .map(|file| VersionFile {
                hashes: file
                    .sha1
                    .iter()
                    .map(|sha1| ("sha1".to_string(), sha1.clone()))
                    .collect(),
                url: file.url.clone(),
                filename: file.filename.clone(),
                primary: file.primary,
                size: u32::try_from(file.size).unwrap_or(u32::MAX),
                file_type: None,
            })
            .collect(),
        dependencies: version
            .dependencies
            .iter()
            .map(|dependency| Dependency {
                version_id: None,
                project_id: Some(format!("cf-{}", dependency.mod_id)),
                file_name: None,
                dependency_type: match dependency.relation_type {
                    1 => DependencyType::Embedded,
                    2 => DependencyType::Optional,
                    5 => DependencyType::Incompatible,
                    _ => DependencyType::Required,
                },
            })
            .collect(),
        game_versions: version.game_versions.clone(),
        loaders: version.loaders.clone(),
    }
}

/// Builds a Labrinth `Version` straight from a CurseForge file, preserving
/// download counts and dependency relations.
pub fn cf_file_to_version(file: &CFFile, changelog: Option<String>) -> Version {
    let (game_versions, loaders) =
        super::normalize::split_file_game_data(&file.game_versions);
    Version {
        id: format!("cf-{}", file.id),
        project_id: format!("cf-{}", file.mod_id),
        author_id: String::new(),
        featured: false,
        name: file.display_name.clone(),
        version_number: file.display_name.clone(),
        changelog,
        changelog_url: None,
        date_published: DateTime::from_timestamp_millis(parse_cf_date(
            &file.file_date,
        ))
        .unwrap_or_default(),
        downloads: file.download_count as u32,
        version_type: super::normalize::release_type_name(file.release_type)
            .to_string(),
        files: vec![VersionFile {
            hashes: file
                .hashes
                .iter()
                .filter(|hash| hash.algo == 1)
                .map(|hash| ("sha1".to_string(), hash.value.clone()))
                .collect(),
            url: get_download_url(file),
            filename: file.file_name.clone(),
            primary: true,
            size: u32::try_from(file.file_length).unwrap_or(u32::MAX),
            file_type: None,
        }],
        dependencies: file
            .dependencies
            .iter()
            .map(|dependency| Dependency {
                version_id: None,
                project_id: Some(format!("cf-{}", dependency.mod_id)),
                file_name: None,
                dependency_type: match dependency.relation_type {
                    1 => DependencyType::Embedded,
                    2 => DependencyType::Optional,
                    5 => DependencyType::Incompatible,
                    _ => DependencyType::Required,
                },
            })
            .collect(),
        game_versions,
        loaders,
    }
}

pub fn project_to_search_hit(project: &CFProject) -> Value {
    let category_names: Vec<&str> =
        project.categories.iter().map(|c| c.name.as_str()).collect();
    let project_type =
        project_type_str(SourceProjectType::from_cf_class(project.class_id));
    json!({
        "project_id": format!("cf-{}", project.id),
        "project_types": [project_type],
        "all_project_types": [project_type],
        "slug": project.slug,
        "author": project.authors.first().and_then(|a| a.name.clone()).unwrap_or_default(),
        "author_id": Value::Null,
        "organization": Value::Null,
        "organization_id": Value::Null,
        "name": project.name,
        "summary": project.summary,
        "categories": category_names,
        "display_categories": category_names,
        "downloads": project.download_count,
        "follows": 0,
        "icon_url": project.logo.url.clone().or_else(|| project.logo.thumbnail_url.clone()),
        "date_created": project.date_created.clone().unwrap_or_default(),
        "date_modified": project.date_modified.clone().unwrap_or_default(),
        "license": "",
        "gallery": [],
        "featured_gallery": Value::Null,
        "color": Value::Null,
        "loaders": [],
        "disclosure_types": [],
    })
}

pub fn search_response_to_browse(
    response: &CFSearchResponse,
) -> BrowseSearchResponse {
    BrowseSearchResponse {
        project_hits: response.data.iter().map(project_to_search_hit).collect(),
        server_hits: Vec::new(),
        total_hits: response.pagination.total_count,
        per_page: 20,
    }
}

/// Kept for callers that still hold a `SourceVersionFile`.
pub fn source_version_file_to_labrinth(
    file: &SourceVersionFile,
) -> VersionFile {
    VersionFile {
        hashes: file
            .sha1
            .iter()
            .map(|sha1| ("sha1".to_string(), sha1.clone()))
            .collect(),
        url: file.url.clone(),
        filename: file.filename.clone(),
        primary: file.primary,
        size: u32::try_from(file.size).unwrap_or(u32::MAX),
        file_type: None,
    }
}
