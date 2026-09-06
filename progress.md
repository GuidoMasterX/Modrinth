# Progress Log

## Session 1 — 2026-09-06
- Loaded brainstorming + planning-with-files skills. Classified task: architectural.
- Created planning files, todo list.
- Dispatched research agents (app architecture, modpack/unknown handling) + CurseForge web research.

## Session 2 - 2026-09-06 12:45
- Task 5.3 done: cache.rs CF integration (CacheValueType/CacheValue Curseforge* variants, fetch arms, deserialize arms, impl_cache_methods lists) + Tauri commands in apps/app (get_curseforge_*). Renamed CF* cache variants to Curseforge* (paste snake_case splits 'CF' to c_f_). theseus + theseus_gui cargo check clean, 30 lib tests pass. Committed.

## Session 3 (Phase 3 complete)
- Commit b27dd30eb: ContentItem gains package_source/cf_project_id/cf_version_id/external_url (Rust + UI types); list_content resolves CF projects/files via cache (get_curseforge_project_many/get_curseforge_file_many) so CF-recognized files show title/icon/version instead of unknown; Source metadata filter (Modrinth/CurseForge) in use-content-metadata-filters; ContentCardItem CurseForge chip (color-mix --color-source-curseforge); IntegrationsSettings.vue (Account > Integrations) with CF API key Input + console link, registered in AppSettingsModal; isExternal excludes recognized CF items. Verified: cargo check -p theseus + theseus_gui, pnpm prepr ui+app-frontend (11 tasks OK).

## Session 3 - Phase 6: ads removal
- Deleted ads.rs, ads_occlusion_*.rs, ads-consent/, helpers/ads.js, PromotionWrapper.vue, capabilities/ads.json.
- Removed ads plugin from main.rs/build.rs/event enum (AdsConsentRequired)/Settings.personalized_ads (+SQL renumber, sqlx prepare).
- App.vue: consent popup, holds, fullscreen ads logic, showAd/providePageContext showAds, watch, template removed. PageContext showAds/adConsentAvailable removed (ui + frontend + app).
- SurveyPopup/image-viewer-editor holds removed; PrivacySettings ads section removed; generated app-events hand-edited; locales ads keys removed from all files.
- Verified: cargo check both crates, cargo test 30 pass, pnpm prepr (app-frontend, ui, frontend) 17 tasks OK.
