//! CurseForge API requests
//!
//! All requests are authenticated with the user-provided API key stored in
//! settings, sent as the `x-api-key` header. The same header is required for
//! CurseForge CDN downloads.

use reqwest::Method;
use serde::de::DeserializeOwned;
use sqlx::SqlitePool;

use crate::state::Settings;
use crate::util::fetch::{FetchSemaphore, fetch_advanced};

use super::structs::{
    CFCategory, CFDataVec, CFDescriptionResponse, CFFile, CFFingerprintsBody,
    CFFingerprintsResponse, CFModFilesResponse, CFModIdsBody, CFProject,
    CFSearchResponse,
};

pub const CURSEFORGE_API_URL: &str = "https://api.curseforge.com/v1";
pub const CURSEFORGE_CDN_URL: &str = "https://edge.forgecdn.net";
/// CurseForge's numeric game ID for Minecraft
pub const CF_GAME_ID_MINECRAFT: i64 = 432;

pub async fn settings_api_key(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<String> {
    let settings = Settings::get(exec).await?;
    settings.curseforge_api_key.ok_or_else(|| {
        crate::ErrorKind::OtherError(String::from(
            "CurseForge API key not configured",
        ))
        .as_error()
    })
}

pub async fn cf_fetch_json<T: DeserializeOwned>(
    method: Method,
    url: &str,
    json_body: Option<serde_json::Value>,
    uri_path: Option<&'static str>,
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<T> {
    let api_key = settings_api_key(pool).await?;
    let result = fetch_advanced(
        method,
        url,
        None,
        json_body,
        Some(("x-api-key", api_key.as_str())),
        None,
        None,
        uri_path,
        fetch_semaphore,
        pool,
    )
    .await?;
    Ok(serde_json::from_slice(&result)?)
}

/// Performs a mods search with a pre-built query suffix (including the
/// leading `?`). Used by the cached layer, whose cache key is the suffix.
pub async fn search_raw(
    params: &str,
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<CFSearchResponse> {
    cf_fetch_json(
        Method::GET,
        &format!("{}/mods/search{}", CURSEFORGE_API_URL, params),
        None,
        Some("curseforge/mods/search"),
        fetch_semaphore,
        pool,
    )
    .await
}

pub async fn search(
    query: Option<&str>,
    game_version: Option<&str>,
    class_id: Option<i64>,
    category_ids: &[i64],
    sort_field: Option<i64>,
    sort_descending: bool,
    index: u32,
    page_size: u32,
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<CFSearchResponse> {
    let mut params = format!("?gameId={}", CF_GAME_ID_MINECRAFT);

    if let Some(query) = query
        && !query.is_empty()
    {
        params.push_str(&format!(
            "&searchFilter={}",
            url::form_urlencoded::byte_serialize(query.as_bytes())
                .collect::<String>()
        ));
    }
    if let Some(game_version) = game_version {
        params.push_str(&format!("&gameVersion={}", game_version));
    }
    if let Some(class_id) = class_id {
        params.push_str(&format!("&classId={}", class_id));
    }
    for category_id in category_ids {
        params.push_str(&format!("&categoryId={}", category_id));
    }
    if let Some(sort_field) = sort_field {
        params.push_str(&format!(
            "&sortField={}&sortOrder={}",
            sort_field,
            i32::from(sort_descending)
        ));
    }
    params.push_str(&format!("&index={}&pageSize={}", index, page_size));

    search_raw(&params, fetch_semaphore, pool).await
}

pub async fn get_categories(
    class_id: Option<i64>,
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<Vec<CFCategory>> {
    let mut url = format!(
        "{}/categories?gameId={}",
        CURSEFORGE_API_URL, CF_GAME_ID_MINECRAFT
    );
    if let Some(class_id) = class_id {
        url.push_str(&format!("&classId={}", class_id));
    }

    let res: CFDataVec<CFCategory> = cf_fetch_json(
        Method::GET,
        &url,
        None,
        Some("curseforge/categories"),
        fetch_semaphore,
        pool,
    )
    .await?;
    Ok(res.data)
}

pub async fn get_mod(
    id: i64,
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<CFProject> {
    cf_fetch_json(
        Method::GET,
        &format!("{}/mods/{}", CURSEFORGE_API_URL, id),
        None,
        Some("curseforge/mods/:id"),
        fetch_semaphore,
        pool,
    )
    .await
}

pub async fn get_mods(
    ids: &[i64],
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<Vec<CFProject>> {
    let body = CFModIdsBody {
        mod_ids: ids.to_vec(),
    };
    let res: CFDataVec<CFProject> = cf_fetch_json(
        Method::POST,
        &format!("{}/mods", CURSEFORGE_API_URL),
        Some(serde_json::to_value(&body)?),
        Some("curseforge/mods"),
        fetch_semaphore,
        pool,
    )
    .await?;
    Ok(res.data)
}

pub async fn get_mod_file(
    file_id: i64,
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<CFFile> {
    cf_fetch_json(
        Method::GET,
        &format!("{}/mods/files/{}", CURSEFORGE_API_URL, file_id),
        None,
        Some("curseforge/mods/files/:id"),
        fetch_semaphore,
        pool,
    )
    .await
}

pub async fn get_mod_files(
    mod_id: i64,
    game_version: Option<&str>,
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<Vec<CFFile>> {
    let mut url = format!(
        "{}/mods/{}/files?gameId={}",
        CURSEFORGE_API_URL, mod_id, CF_GAME_ID_MINECRAFT
    );
    if let Some(game_version) = game_version {
        url.push_str(&format!("&gameVersion={}", game_version));
    }

    let res: CFModFilesResponse = cf_fetch_json(
        Method::GET,
        &url,
        None,
        Some("curseforge/mods/:id/files"),
        fetch_semaphore,
        pool,
    )
    .await?;
    Ok(res.data)
}

pub async fn get_mod_description(
    id: i64,
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<String> {
    let res: CFDescriptionResponse = cf_fetch_json(
        Method::GET,
        &format!("{}/mods/{}/description", CURSEFORGE_API_URL, id),
        None,
        Some("curseforge/mods/:id/description"),
        fetch_semaphore,
        pool,
    )
    .await?;
    Ok(res.data)
}

pub async fn get_fingerprints(
    fingerprints: &[i64],
    fetch_semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<CFFingerprintsResponse> {
    let body = CFFingerprintsBody {
        fingerprints: fingerprints.to_vec(),
    };
    cf_fetch_json(
        Method::POST,
        &format!(
            "{}/fingerprints/{}",
            CURSEFORGE_API_URL, CF_GAME_ID_MINECRAFT
        ),
        Some(serde_json::to_value(&body)?),
        Some("curseforge/fingerprints"),
        fetch_semaphore,
        pool,
    )
    .await
}

/// Builds the direct CDN download URL for a file. CurseForge only provides
/// `download_url` for some responses; the CDN path can always be derived from
/// the file ID and name.
pub fn get_download_url(file: &CFFile) -> String {
    if let Some(url) = file.download_url.as_ref() {
        if !url.is_empty() {
            return url.to_string();
        }
    }
    let directory = file.id / 1000;
    format!(
        "{}/files/{}/{}/{}",
        CURSEFORGE_CDN_URL, directory, file.id, file.file_name
    )
}
