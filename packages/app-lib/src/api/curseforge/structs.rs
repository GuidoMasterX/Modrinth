//! CurseForge API response types
//!
//! Field names mirror the CurseForge API (camelCase JSON), while the Rust
//! fields stay snake_case.

use serde::{Deserialize, Serialize};

pub const CF_CLASS_MOD: i64 = 6;
pub const CF_CLASS_RESOURCE_PACK: i64 = 12;
pub const CF_CLASS_MODPACK: i64 = 4471;
pub const CF_CLASS_SHADER_PACK: i64 = 6552;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Pagination {
    pub index: u32,
    pub page_size: u32,
    pub result_count: u32,
    pub total_count: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFProjectLinks {
    pub website_url: Option<String>,
    pub wiki_url: Option<String>,
    pub issues_url: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFProjectLogo {
    pub url: Option<String>,
    pub thumbnail_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFCategory {
    pub id: i64,
    pub game_id: i64,
    pub name: String,
    pub slug: String,
    pub icon_url: Option<String>,
    pub class_id: Option<i64>,
    pub parent_category_id: Option<i64>,
    pub is_class: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFProject {
    pub id: i64,
    pub game_id: i64,
    pub name: String,
    pub slug: Option<String>,
    pub links: CFProjectLinks,
    pub summary: String,
    pub download_count: u64,
    pub date_created: Option<String>,
    pub date_modified: Option<String>,
    pub logo: CFProjectLogo,
    pub categories: Vec<CFCategory>,
    pub class_id: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFSearchResponse {
    pub data: Vec<CFProject>,
    pub pagination: Pagination,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFHash {
    pub value: String,
    pub algo: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFFileDependency {
    pub mod_id: i64,
    pub relation_type: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFFile {
    pub id: i64,
    pub game_id: i64,
    pub mod_id: i64,
    pub display_name: String,
    pub file_name: String,
    pub release_type: i32,
    pub hashes: Vec<CFHash>,
    pub file_date: Option<String>,
    pub file_length: u64,
    pub download_count: u64,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub dependencies: Vec<CFFileDependency>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFModFilesResponse {
    pub data: Vec<CFFile>,
    pub pagination: Pagination,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFFingerprintMatch {
    pub id: i64,
    pub file: CFFile,
    pub latest_files: Vec<CFFile>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFFingerprintData {
    pub is_cache_built: bool,
    pub exact_matches: Vec<CFFingerprintMatch>,
    pub exact_fingerprints: Vec<i64>,
    pub unmatched_fingerprints: Vec<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFFingerprintsResponse {
    pub data: CFFingerprintData,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFDescriptionResponse {
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFModIdsBody {
    pub mod_ids: Vec<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFFingerprintsBody {
    pub fingerprints: Vec<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFDataVec<T> {
    pub data: Vec<T>,
}
