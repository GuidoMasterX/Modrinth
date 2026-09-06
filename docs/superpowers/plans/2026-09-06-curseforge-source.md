# CurseForge Alternate Source Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add CurseForge as a second package source (search/browse, project pages, instance content, modpack import/management) to the desktop app, with user-supplied API key, source theming, pinned filter tabs, and full ads removal.

**Architecture:** All CF HTTP lives in Rust (`packages/app-lib`), mirroring the Modrinth cache layer (`state/cache.rs`) — a new `api/curseforge/` module with normalized `SourceProject`/`SourceVersion` DTOs, cached with `cf:`-prefixed keys. SQLite model gains a source dimension. Frontend consumes everything through the existing Tauri `plugin:cache`/`plugin:instance` command pattern; shared UI in `packages/ui` stays source-agnostic via injected providers, matching the Modrinth patterns exactly.

**Tech Stack:** Rust/Tauri/sqlx (app-lib), Vue 3 + TanStack Query + createContext DI (app-frontend, packages/ui), Tailwind + SCSS variables.

**Spec:** `docs/superpowers/specs/2026-09-06-curseforge-source-design.md`

## Global Constraints

- No shipped/embedded CF API key; key comes from settings (`curseforge_api_key`), sent as `x-api-key` header to `https://api.curseforge.com/v1` AND to `edge.forgecdn.net` downloads.
- All CF features degrade gracefully without a key (CF toggle hidden/disabled with settings prompt).
- Indentation: TAB everywhere. No heading comments in code. Doc comments OK.
- Follow existing patterns: HTTP in Rust, cache key = URL suffix, Tauri commands via `apps/app/src/api/*`, TanStack Query + createContext DI in frontend.
- CF features are no-ops when `curseforge_api_key` is unset — never break Modrinth paths.
- Modrinth always preferred; cross-source switch is opt-in per package.

---

## Phase 1 — Rust: settings, CF client, cache, storage model

### Task 1.1: Settings field for CF API key

**Files:**
- Modify: `packages/app-lib/src/state/settings.rs` (or wherever `Settings` struct lives — locate via `personalized_ads`)

**Interfaces:**
- Produces: `Settings.curseforge_api_key: Option<String>` (serde default None), getter used by CF client: `state::Settings::get().await.curseforge_api_key`

- [ ] Add `#[serde(default)] pub curseforge_api_key: Option<String>` to `Settings`, next to `personalized_ads`.
- [ ] `cargo check -p theseus` passes.

### Task 1.2: CF API client module

**Files:**
- Create: `packages/app-lib/src/api/curseforge/mod.rs` — module root, re-exports
- Create: `packages/app-lib/src/api/curseforge/structs.rs` — serde structs for CF API responses (Pagination, File, Project, Category, ModLoaderType) with `#[serde(rename_all = "camelCase")]`
- Create: `packages/app-lib/src/api/curseforge/api.rs` — request functions

**Interfaces (api.rs):**
```rust
pub const CURSEFORGE_API_URL: &str = "https://api.curseforge.com/v1";
const CF_GAME_ID: i64 = 432; // Minecraft

pub async fn cf_fetch_json<T: DeserializeOwned>(path: &str, query: &str) -> Result<T>
// reads key from Settings; Err("CurseForge API key not configured") if None
// sends headers: x-api-key, Accept: application/json

pub async fn search(query: &str, game_version: Option<&str>, class_id: Option<i64>, category_ids: &[i64], sort_field: Option<i64>, sort_order: &str, index: u32, page_size: u32) -> Result<CFSearchResponse>
// GET /mods/search?gameId=432&...
// classId: 6=Mod, 12=ResourcePack, 4471=Modpack, 6552=ShaderPack

pub async fn get_categories(class_id: i64) -> Result<Vec<CFCategory>> // GET /categories?gameId=432&classId=
pub async fn get_mod(id: i64) -> Result<CFProject>   // GET /mods/{id}
pub async fn get_mods(ids: &[i64]) -> Result<Vec<CFProject>> // POST /mods body {modIds}
pub async fn get_mod_file(id: i64) -> Result<CFFile> // GET /mods/files/{id}
pub async fn get_mod_files(mod_id: i64, game_version: Option<&str>) -> Result<Vec<CFFile>> // GET /mods/{id}/files
pub async fn get_fingerprints(fingerprints: &[i64]) -> Result<CFFingerprintResponse> // POST /fingerprints/432 body {fingerprints}
pub async fn get_download_url(file: &CFFile) -> Result<String>
// file.downloadUrl if present, else https://edge.forgecdn.net/files/{id/1000}/{id}/{filename} (strip leading zeros of id/1000)
```

- [ ] Register `pub mod curseforge;` in `packages/app-lib/src/api/mod.rs`.
- [ ] Unit test for `get_download_url` fallback path construction (pure fn, no HTTP).
- [ ] `cargo check -p theseus` passes.

### Task 1.3: Normalized DTOs

**Files:**
- Create: `packages/app-lib/src/api/curseforge/normalize.rs`

**Interfaces:**
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "source")]
pub enum Source { Modrinth, CurseForge }

pub struct SourceProject { source: Source, id: String, slug: Option<String>, title: String, description: String, body_url: String, icon_url: Option<String>, categories: Vec<String>, downloads: u64, follows: u64, updated: Option<String>, date_created: Option<String>, website_url: Option<String>, project_type: ProjectType, gallery: Vec<SourceGalleryItem> }

pub struct SourceVersion { source: Source, id: String, project_id: String, version_number: String, changelog: Option<String>, game_versions: Vec<String>, loaders: Vec<String>, date_published: Option<String>, version_type: String, files: Vec<SourceVersionFile>, dependencies: Vec<String> }

pub struct SourceVersionFile { id: String, filename: String, url: String, primary: bool, size: u64, hashes: Vec<(String, String)> }

impl SourceProject { pub fn from_cf(cf: CFProject, cf_categories: &[CFCategory]) -> Self }
impl SourceVersion { pub fn from_cf(cf: CFFile, changelog: Option<String>) -> Self }
```
- Map CF fields: `name→title`, `summary→description`, `links.websiteUrl→website_url/body_url`, `logo.thumbnailUrl→icon_url`, `dateModified→updated`, `dateCreated`, `downloadCount→downloads`, `slug→slug`, categories via id→name lookup against cached CF categories.

- [ ] Unit tests: CF JSON fixture → `SourceProject` mapping (category names resolved), CF file → `SourceVersion`.
- [ ] `cargo test -p theseus curseforge` passes.

### Task 1.4: Cache integration

**Files:**
- Modify: `packages/app-lib/src/state/cache.rs` — add `CacheValue` variants and Tauri-facing fns, mirroring `get_search_results_v3` (line ~1983) / `get_project` patterns:
  - `CacheValue::CFSearchResults { search: String, result: CFSearchResponse }`
  - `CacheValue::CFProject { id: String, result: SourceProject }`
  - `CacheValue::CFVersions { id: String, result: Vec<SourceVersion> }`
  - `CacheValue::CFCategories { class_id: i64, result: Vec<CFCategory> }`
  - `CacheValue::CFFileInfo { id: String, result: SourceVersionFile }`
- Key format: `cf:<endpoint?>:<query>` (e.g. `cf:mods/search:...`), stored in same SQLite cache, same stale/`must_revalidate` policy as Modrinth.

**Interfaces (exposed via `apps/app/src/api/cache.rs` plugin commands, same macro pattern):**
```rust
get_cf_search_results(params: String, force: bool) -> CFSearchResponse
get_cf_categories(class_id: i64) -> Vec<CFCategory>
get_cf_project(id: i64) -> SourceProject
get_cf_project_versions(id: i64) -> Vec<SourceVersion>
get_cf_file(file_id: i64) -> SourceVersionFile
get_cf_fingerprints(fingerprints: Vec<i64>) -> CFFingerprintResponse
```
- [ ] `cargo check` on app-lib + app workspace passes; no Modrinth behavior changed.

### Task 1.5: Storage model — source dimension

**Files:**
- Modify: `packages/app-lib/src/state/instances/model/content_entry.rs` — add `cf_project_id: Option<i64>`, `cf_version_id: Option<i64>` (file id)
- Modify: `packages/app-lib/src/state/instance_types.rs` (`FileMetadata`): add `source: Source`, `cf_project_id: Option<i64>`
- Modify: `packages/app-lib/src/state/instances/model/content_set.rs` — `ContentSourceKind` gains `CurseforgeModpack`
- Modify: `packages/app-lib/src/state/instances/model/instance.rs` — `Instance` gains `preferred_source: Source` (default `Modrinth`)
- Modify: SQLite migrations — new columns with defaults; enum stored as TEXT so new variant needs no schema change (verify)
- Modify: `packages/app-lib/src/state/instances/commands/list_content.rs` — `ContentFilter` gains `OnlySource(Source)`; content items populate `source`/`external`/`external_url` (CF project website URL)

**Interfaces:**
- Produces: `ContentItem.source: Source` (serde camelCase → frontend `item.source`), `ContentFilter::OnlySource(Source)`, `Instance.preferred_source`
- [ ] Migration applies cleanly on a copy of a dev DB (`sqlx migrate run` via app startup).
- [ ] `cargo check` passes.

### Task 1.6: Fingerprint recognition for unknown files

**Files:**
- Create: `packages/app-lib/src/util/murmur2.rs`
```rust
pub fn murmur2(data: &[u8]) -> i64
// CF variant: seed 1, treat bytes as signed, skip 0x20 (space), 31-bit arithmetic (u32 mul/rotate with i64 wrap handling), final avalanche
```
- Modify: instance content listing — for `FileMetadata`-less files with a `.jar`, compute Murmur2 over the jar (CF rule: strip all 0x09/0x0a/0x0d/0x20 bytes) and call `get_fingerprints`; on exact match, populate `cf_project_id` + `source: CurseForge`, name/icon from `get_mod`.
- [ ] Unit test: known CF fingerprint vector (e.g. empty input → 0; "hello" vector) matches.
- [ ] Wire into list_content path behind a cheap check (only for unknown, non-disabled jars).

## Phase 2 — Frontend: browse source switch, theming, pinned tabs

### Task 2.1: Source theming tokens

**Files:**
- Modify: `packages/assets/styles/variables.scss` — add near `--color-platform-*`:
```scss
--color-source-modrinth: #00af54;
--color-source-curseforge: #f16436;
```
(light/dark/oled variants as needed for contrast)
- Produces: `var(--color-source-modrinth)`, `var(--color-source-curseforge)` + matching `-raised`/contrast variants if the platform pattern has them.

### Task 2.2: Browse source toggle + CF categories

**Files:**
- Modify: `packages/ui/src/layouts/shared/browse-tab/providers/browse-manager.ts` — add optional `sources?: Source[]` + `activeSource` state to `BrowseManagerContext`; `search` callback signature gains `source: Source`
- Modify: `packages/ui/src/layouts/shared/browse-tab/composables/use-browse-search.ts` — when source=CurseForge, build CF param set (classId from project type: 6/12/4471/6552; CF category ids; CF sort field ids) instead of Modrinth facets; query key becomes `['search', source, params]`
- Modify: `packages/ui/src/layouts/shared/browse-tab/header.vue` — segmented Modrinth/CurseForge toggle (CF side tinted `--color-source-curseforge`, shows key-setup prompt when `sources` absent)
- Modify: `apps/app-frontend/src/pages/Browse.vue` — provide CF search callback routing to `get_cf_search_results`; provide CF categories merged into filter sidebar when active (via `get_cf_categories` → same `FilterOption` shape in `packages/ui/src/utils/search.ts`)
- Modify: `apps/app-frontend/src/helpers/settings-state.js` (or wherever settings are read) — expose `curseforge_api_key` so `Browse.vue` knows whether CF is enabled

### Task 2.3: Pinned filter tabs

**Files:**
- Modify: `packages/ui/src/layouts/shared/browse-tab/sidebar.vue` + header — selected filter tabs persist per project type into settings (`pinned_browse_tabs`); on mount, restore pinned tabs and apply them to the URL params automatically (no dropdown reselection)
- Modify: settings state — new `pinned_browse_tabs: Record<string, string[]>` with default `'mod': []` etc.

## Phase 3 — Instance content: source filter + recognition display

### Task 3.1: Source filter tab

**Files:**
- Modify: `packages/ui/src/layouts/shared/content-tab/composables/use-content-metadata-filters.ts` — add "Source" filter: `isModrinth(item) = item.source !== 'curseforge'`, `isCurseforge(item)`, `isExternal(item)` (existing)
- Modify: `packages/ui/src/layouts/shared/content-tab/ContentCardItem.vue` — source badge: green "Modrinth" / orange "CurseForge" chip using source tokens; CF items keep CF accent on hover/active
- Modify: `apps/app-frontend/src/pages/instance/content/index.vue` — map `item.source` from backend `ContentItem`

### Task 3.2: CF key settings UI

**Files:**
- Modify: settings page component for API keys (locate existing key/token settings section in `apps/app-frontend/src/pages/settings/`) — text field for `curseforge_api_key` with help text linking console.curseforge.com
- Produces: settings saved → CF browse toggle + CF features activate

## Phase 4 — Project pages render both sources + switch-source

### Task 4.1: Source-aware project data layer

**Files:**
- Create: `apps/app-frontend/src/composables/projects/use-source-project.ts` — given `{ projectId?: string, cfProjectId?: number }`, TanStack Query resolves a `SourceProject` (Modrinth via `get_project_v3`, CF via `get_cf_project`) and its versions (`get_project_versions` / `get_cf_project_versions`) uniformly
- Modify: `apps/app-frontend/src/pages/project/{Index,Versions,Description,Gallery}.vue` — consume normalized shape; render from `SourceProject`/`SourceVersion` when source=curseforge (body/description fetched lazily: CF `GET /mods/{id}/description` added to Task 1.2 as `get_mod_description(id) -> String`)

### Task 4.2: Switch-source button + cross-source matching

**Files:**
- Create: `packages/app-lib/src/api/curseforge/match.rs`
```rust
pub async fn find_cross_source_match(p: &SourceProject) -> Option<SourceProject>
// Modrinth→CF: query CF search by exact slug/title + game version filter; fingerprint fallback on a primary file hash
// CF→Modrinth: Modrinth search by slug/title
```
- Expose Tauri command `find_cross_source_match` (plugin:cache).
- Modify: project pages header — button "View on CurseForge"/"View on Modrinth" (orange/green accent) when a match exists; else "Open website" fallback link to the other site's search. Switch preserves tab (description/versions/gallery).
- [ ] Unit test for match fallback ordering.

## Phase 5 — CF modpack import, management, updates

### Task 5.1: CF .zip import

**Files:**
- Create: `packages/app-lib/src/api/pack/install_cfzip.rs` — parse `manifest.json` (`{name, version, author, files:[{projectID,fileID}], minecraft:{version, modLoaders:[{id:"forge-...",primary}]}}`), resolve files via CF API, download with x-api-key, extract `overrides/` like mrpack overrides, create instance with `preferred_source = Curseforge`, `ContentSourceKind::CurseforgeModpack`, `InstanceLink` if `projectID` maps to a published modpack (`get_mod(projectID)` projectType==4471)
- Modify: `packages/app-lib/src/api/pack/install_from.rs` — `CreatePackLocation`/format sniff: detect `manifest.json` → CF zip path
- Modify: `apps/app-frontend/src/providers/setup/file-picker.ts` — accept `.zip` with manifest.json for modpack creation

### Task 5.2: CF App import → linked modpack

**Files:**
- Modify: `packages/app-lib/src/api/pack/import/curseforge.rs` — when parsed instance has an `InstalledModpack` with a known CF projectID/fileID, set `InstanceLink` to that project, `preferred_source = Curseforge`, `ContentSourceKind::CurseforgeModpack`
- Produces: `get_linked_modpack_info` returns CF modpack info with `source: CurseForge`

### Task 5.3: Linked CF modpack management parity

**Files:**
- Modify: `packages/app-lib/src/state/instances/commands/check_content_updates.rs` — for `source: CurseForge` entries, latest version = first CF file matching instance game version+loader (CF `get_mod_files`), compare fileID
- Modify: linked-modpack content listing — expose CF modpack `files[]` as installable content (resolve each `projectID/fileID` → `SourceVersion`)
- Modify: frontend linked-modpack UI (pages/instance) — render CF modpack name/version/icon, "Update" via CF path; version switch = re-resolve CF modpack file
- Export: CF-format export intentionally out of scope (mrpack export unchanged; exporting a CF instance emits mrpack with external file URLs where licenses permit)

## Phase 6 — Ads removal

**Files:**
- Delete: `apps/app/src/api/ads.rs`, ads occlusion modules (`ads_occlusion_windows.rs`/`ads_occlusion_macos.rs`), their registration in `apps/app/src/main.rs`/`lib.rs` invoke handlers
- Delete: `apps/app-frontend/src/helpers/ads.js`
- Modify: `apps/app-frontend/src/App.vue` — remove ads consent popup + `ads_consent_required` event handling + init calls
- Modify: `apps/app-frontend/src/providers/setup/app-event-codec.ts` — drop ads events
- Modify: `apps/app-frontend/src/providers/setup/image-viewer-editor.ts` + modal ad hold/release call sites — remove
- Modify: settings — remove `personalized_ads` setting (add a settings migration that ignores/drops the key)
- [ ] `rg -i "ads" apps/app apps/app-frontend` returns no functional references (icon assets aside).

## Phase 7 — Verification & final report

- [ ] `cargo clippy -p theseus -p app` clean; `cargo test -p theseus` green
- [ ] `pnpm prepr` (frontend web + app) green
- [ ] Manual smoke checklist: browse Modrinth unchanged; CF toggle with key set; CF search/categories/sort; install CF mod into instance; instance Source filter; unknown jar recognized via fingerprint; switch-source on a dual-listed project; CF .zip import; CF App import sets preferred source; CF modpack update flow; no-key degradation
- [ ] Structure/optimization review pass (bugfixes from main-repo issues) — separate follow-up task list after CF ships
- [ ] Final report → confirm → push to fork
