# Findings

## Repo layout facts
- Monorepo: turborepo + pnpm. Apps: frontend (Nuxt website), app-frontend (desktop Vue 3), labrinth (Rust API). Packages: app-lib, api-client, ui, assets.
- (to be filled by research agents)

## CurseForge API
- Official API: api.curseforge.com, auth via `x-api-key` header; key generated at console.curseforge.com BUT returns 403 until Overwolf approves an application. ToS: key is non-transferable, must not be disclosed to third parties (forks historically violate this; Prism got an exception).
- **CDN downloads (edge.forgecdn.net) require a valid API key since July 16, 2026** (x-api-key header or ?api-key= query param); unauthenticated requests get 401. (blog.curseforge.com 2026-06-10)
- cfwidget.com JSON API (api.cfwidget.com): unofficial, no key, GET by project path or ID; data extracted from official API, updated on demand (last_fetch). No search endpoint; per-project lookup only.
- CF modpack zip: manifest.json {manifestType, manifestVersion, name, version, author, files:[{projectID,fileID}], minecraft:{version, modLoaders}}; dependencies via projectID references; overrides/ folder. Resolution of projectID/fileID → download URL requires the API.
- MurmurHash2 fingerprint used for file matching (modrinth uses sha1; CF fingerprint endpoint /fingerprints/432).
- Categories from CF website (mc-mods): Addons, Adventure and RPG, API and Library, Armor Tools Weapons, Automation, Biomes, Blood Magic, Buildcraft, Cosmetic, ... (explicit category slugs available).

## Other launchers' CF implementations
- Prism Launcher: approved key exception from Overwolf; some mods blocked → manual-download fallback flow. PolyMC forks embed keys (ToS gray area).
- Community scrapers exist but are fragile (Cloudflare).

## Key repo facts (Phase 1, verified)
- Browse: apps/app-frontend/src/pages/Browse.vue → use-browse-search.ts (packages/ui) → Rust cache search (packages/app-lib/src/state/cache.rs:1983) → MODRINTH_API_URL_V3. Cache key = query string. No source concept anywhere.
- Browse is a shared layout in packages/ui (browse-tab/: sidebar.vue, layout.vue, header.vue, providers/browse-manager.ts with injected `search` callback, variant 'app').
- Project pages: apps/app-frontend/src/pages/project/{Index,Versions,Gallery,Description}.vue; data via Rust cache helpers (helpers/cache.js); types from packages/api-client (Labrinth.Projects.v2/v3).
- Instance content: pages/instance/content/index.vue (vue-query) + packages/ui content-tab (ContentItem has `external`, `external_url`); filter composables include use-content-metadata-filters.ts with "External" filter.
- Rust DB (SQLite/sqlx): ContentEntry/ContentSet have `source_kind: ContentSourceKind` — enum = {Local, ModrinthModpack, ServerProject, ModrinthHosting, ImportedModpack, SharedInstance} (content_set.rs:9). This is provenance, NOT Modrinth-vs-CF source → CF needs a new dimension (likely new variants + per-entry CF project/file ids).
- Unknown/external handling: hash lookup `is_file_on_modrinth`; EmbeddedContentMetadata (name/version/icon from jar) fallback; external flag in UI. Extension point for CF linking.
- mrpack import: api/pack/install_from.rs (PackFormat) + install_mrpack.rs; export: api/instance/export_mrpack.rs. CF App import ALREADY exists: api/pack/import/curseforge.rs parses minecraftinstance.json (via import/mod.rs orchestration) — extension point for "link to published CF modpack".
- Ads: apps/app/src/api/ads.rs (938 lines, ads webview window + consent flow), helpers/ads.js, App.vue consent popup, settings.personalized_ads.
- Theming: packages/assets/styles/variables.scss (--color-platform-* precedent for per-entity accent colors); _CurseForgeIcon already exists in packages/assets.
- API client: packages/api-client supports external APIs via full base URL + skipAuth (per AGENTS.md) — CF module slot exists.
- State: TanStack Query + createContext DI providers (no Pinia). App does all API traffic in Rust (fetch.rs + cache.rs), not frontend.
