use crate::api::Result;
use theseus::data::{
    BrowseSearchResponse, CFCategory, Project, SourceVersionFile, Version,
};
use theseus::prelude::*;

macro_rules! impl_cache_methods {
    ($(($variant:ident, $type:ty)),*) => {
        $(
            paste::paste! {
                #[tauri::command]
                pub async fn [<get_ $variant:snake>](id: &str, cache_behaviour: Option<CacheBehaviour>) -> Result<Option<$type>>
                {
                    Ok(theseus::cache::[<get_ $variant:snake>](id, cache_behaviour).await?)
                }

                #[tauri::command]
                pub async fn [<get_ $variant:snake _many>](
                    ids: Vec<String>,
                    cache_behaviour: Option<CacheBehaviour>,
                ) -> Result<Vec<$type>>
                {
                    let ids = ids.iter().map(|x| &**x).collect::<Vec<&str>>();
                    let entries =
                        theseus::cache::[<get_ $variant:snake _many>](&*ids, cache_behaviour).await?;

                    Ok(entries)
                }
            }
        )*
    }
}

impl_cache_methods!(
    (Project, Project),
    (ProjectV3, ProjectV3),
    (Version, Version),
    (User, User),
    (Team, Vec<TeamMember>),
    (Organization, Organization),
    (SearchResults, SearchResults),
    (SearchResultsV3, SearchResultsV3),
    (CurseforgeFile, SourceVersionFile),
    (CurseforgeFingerprints, CachedCFFingerprints)
);

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("cache")
        .invoke_handler(tauri::generate_handler![
            get_project,
            get_project_many,
            get_project_v3,
            get_project_v3_many,
            get_version,
            get_version_many,
            get_user,
            get_user_many,
            get_team,
            get_team_many,
            get_organization,
            get_organization_many,
            get_search_results,
            get_search_results_many,
            get_search_results_v3,
            get_search_results_v3_many,
            purge_cache_types,
            get_project_versions,
            get_curseforge_search_results,
            get_curseforge_project,
            get_curseforge_project_versions,
            get_curseforge_file_changelog,
            get_curseforge_categories,
            get_curseforge_file,
            get_curseforge_file_many,
            get_curseforge_fingerprints,
            get_curseforge_fingerprints_many,
            get_curseforge_description,
        ])
        .build()
}

#[tauri::command]
pub async fn purge_cache_types(cache_types: Vec<CacheValueType>) -> Result<()> {
    Ok(theseus::cache::purge_cache_types(&cache_types).await?)
}

#[tauri::command]
pub async fn get_project_versions(
    project_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Option<Vec<Version>>> {
    Ok(
        theseus::cache::get_project_versions(project_id, cache_behaviour)
            .await?,
    )
}

#[tauri::command]
pub async fn get_curseforge_search_results(
    id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Option<BrowseSearchResponse>> {
    Ok(theseus::cache::get_curseforge_search_results(id, cache_behaviour)
        .await?
        .map(|cached| {
            theseus::data::labrinth_map::search_response_to_browse(
                &cached.result,
            )
        }))
}

#[tauri::command]
pub async fn get_curseforge_project(
    id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Option<serde_json::Value>> {
    let Some(source) = theseus::cache::get_curseforge_project(id, cache_behaviour).await?
    else {
        return Ok(None);
    };
    let mut project = serde_json::to_value(theseus::data::labrinth_map::project_to_labrinth(&source)).unwrap_or_default();
    project["website_url"] =
        serde_json::Value::String(theseus::data::labrinth_map::canonical_project_url(&source));

    project["cf_members"] = serde_json::Value::Array(
        source
            .authors
            .iter()
            .enumerate()
            .map(|(index, author)| {
                let name = author.name.clone().unwrap_or_else(|| author.id.to_string());
                serde_json::json!({
                    "team_id": format!("cf-author-{}", author.id),
                    "user": {
                        "id": format!("cf-user-{}", author.id),
                        "username": name,
                        "avatar_url": author.avatar_url,
                        "bio": serde_json::Value::Null,
                        "created": "1970-01-01T00:00:00Z",
                        "role": "Author",
                        "badges": 0
                    },
                    "is_owner": index == 0,
                    "role": "Author",
                    "ordering": index as i64
                })
            })
            .collect(),
    );

    if let Some(versions) =
        theseus::cache::get_curseforge_project_versions(id, None).await?
    {
        project["versions"] = serde_json::Value::Array(
            versions
                .iter()
                .map(|version| {
                    serde_json::Value::String(format!("cf-{}", version.id))
                })
                .collect(),
        );
    }

    Ok(Some(project))
}

#[tauri::command]
pub async fn get_curseforge_project_versions(
    id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Option<Vec<Version>>> {
    let mut versions = theseus::cache::get_curseforge_project_versions(
        id,
        cache_behaviour,
    )
    .await?
    .map(|versions| {
        versions
            .iter()
            .map(theseus::data::labrinth_map::version_to_labrinth)
            .collect::<Vec<Version>>()
    });

    if let Some(versions) = versions.as_mut() {
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    }

    Ok(versions)
}

#[tauri::command]
pub async fn get_curseforge_file_changelog(mod_id: i64, file_id: i64) -> Result<Option<String>> {
    Ok(theseus::cache::get_curseforge_file_changelog(mod_id, file_id).await?)
}

#[tauri::command]
pub async fn get_curseforge_categories(
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Option<Vec<CFCategory>>> {
    Ok(theseus::cache::get_curseforge_categories(cache_behaviour).await?)
}

#[tauri::command]
pub async fn get_curseforge_description(
    id: &str,
) -> Result<Option<String>> {
    Ok(theseus::cache::get_curseforge_description(id).await?)
}
