//! CurseForge API response types
//!
//! Field names mirror the CurseForge API (camelCase JSON), while the Rust
//! fields stay snake_case.

use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

/// Deserializes `null` as `T::default()`; serde's `default` attribute only
/// covers missing fields, but the CF API returns explicit `null`s (e.g.
/// `unmatchedFingerprints`).
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
	D: serde::Deserializer<'de>,
	T: DeserializeOwned + Default,
{
	let value: Option<T> = Option::deserialize(deserializer)?;
	Ok(value.unwrap_or_default())
}

pub const CF_CLASS_MOD: i64 = 6;
pub const CF_CLASS_RESOURCE_PACK: i64 = 12;
pub const CF_CLASS_MODPACK: i64 = 4471;
pub const CF_CLASS_SHADER_PACK: i64 = 6552;
pub const CF_CLASS_DATA_PACK: i64 = 6945;

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
    pub authors: Vec<CFAuthor>,
    pub class_id: Option<i64>,
    pub allow_mod_distribution: Option<bool>,
    pub main_file_id: Option<i64>,
    pub latest_files_indexes: Vec<CFLatestFileIndex>,
    pub screenshots: Vec<CFModAsset>,
    pub date_released: Option<String>,
    pub thumbs_up_count: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFModAsset {
    pub id: i64,
    pub mod_id: i64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFAuthor {
    pub id: i64,
    pub name: Option<String>,
    pub url: Option<String>,
    pub avatar_url: Option<String>,
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub hashes: Vec<CFHash>,
    pub file_date: Option<String>,
    pub file_length: u64,
    pub download_count: u64,
    pub download_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub game_versions: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub loaders: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub dependencies: Vec<CFFileDependency>,
    pub is_available: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFLatestFileIndex {
    pub game_version: Option<String>,
    pub file_id: i64,
    pub filename: Option<String>,
    pub release_type: Option<i32>,
    pub mod_loader: Option<i64>,
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub latest_files: Vec<CFFile>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFFingerprintData {
    pub is_cache_built: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub exact_matches: Vec<CFFingerprintMatch>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub exact_fingerprints: Vec<i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
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
    #[serde(rename = "modIds")]
    pub mod_ids: Vec<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFFileIdsBody {
    #[serde(rename = "fileIds")]
    pub file_ids: Vec<i64>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CFData<T> {
    pub data: T,
}
