# CurseForge Alternate Source — Design Spec

**Date:** 2026-09-06
**Status:** Approved (user approved design in chat, 2026-09-06)

## Goal

Add CurseForge as an alternate package source (mods, resource packs, shaders, modpacks) to the Modrinth desktop app, with Modrinth always preferred. Per-package switching between Modrinth and CurseForge pages, CF-exclusive mod recognition, CF modpack import/management parity, color-coded source theming (green = Modrinth, dark orange = CurseForge), pinned filter tabs, full ads removal, and a performance/bugfix pass.

## Constraints

- **API access:** user-supplied CurseForge API key (from console.curseforge.com), stored in app settings. No shipped/embedded key. All CF features degrade gracefully (hidden/disabled) when no key is set.
- CF CDN downloads (edge.forgecdn.net) require the same `x-api-key` header (post-2026-07-16).
- Match existing Modrinth API patterns: all HTTP in Rust (`packages/app-lib`), cached via `state/cache.rs`, exposed through Tauri `plugin:cache` commands.
- MurmurHash2 fingerprints (CF `/fingerprints/432`) used for file→project matching.

## Architecture

### 1. Rust source layer (`packages/app-lib`)

- New `api/curseforge/` module: `search`, `categories`, `project`, `project_versions`, `file`, `fingerprints` against `https://api.curseforge.com/v1`, authenticated via `x-api-key` from settings; `x-api-key` also sent on forgecdn downloads.
- CF responses cached in `state/cache.rs` with `cf:`-prefixed cache keys, same invalidation model as Modrinth entries.
- Normalized DTOs: `SourceProject` / `SourceVersion` with a `source: Modrinth | CurseForge` discriminator, mapping CF fields (links, files, thumbnails) onto shapes the frontend already consumes.
- Storage: `ContentEntry`/`FileMetadata` gain `source` + CF ids (`cf_project_id`, `cf_file_id`); `ContentSourceKind` gains `CurseforgeModpack`; `Instance` gains `preferred_source` (set to `curseforge` when importing a CF App instance or CF modpack zip).
- Unknown-file recognition: MurmurHash2 fingerprint lookup maps unknown jars to CF projects.
- SQLite migrations for new columns/enum values.

### 2. Frontend — Discover Content

- Source toggle (Modrinth / CurseForge) in browse header; when CurseForge selected, CF categories and sorts replace Modrinth ones (reference: curseforge.com/minecraft category layout).
- `--color-source-modrinth` (green) / `--color-source-curseforge` (dark orange) theme tokens in `variables.scss`; source accents applied to badges, active toggle, and project cards per source.
- Pinned filter tabs: filter/sidebar tabs persist in settings; no manual dropdown reselection after navigation.

### 3. Frontend — Instance Management

- New "Source" filter tab in instance content separating Modrinth / CurseForge / External items.
- Package details pages (description, versions, gallery) render either source via normalized DTOs, with a switch-source button when a cross-source match exists (match by name/slug + game version; fingerprint fallback; external website link fallback when no match).

### 4. CF modpacks

- CF App modpack import (existing `api/pack/import/curseforge.rs`) extended: store modpack `projectID/fileID` as an `InstanceLink`, set `preferred_source = curseforge`.
- CF `.zip` (manifest.json + overrides) import added alongside `.mrpack`.
- All Modrinth modpack management features ported: linked modpack content view, per-file updates via CF API, version/override handling.
- mrpack export stays Modrinth-format; CF-format export is out of scope.

### 5. Ads removal

- Delete `apps/app/src/api/ads.rs`, ads occlusion, `helpers/ads.js`, consent popup in `App.vue`, `personalized_ads` setting, and all hold/release call sites.

### 6. Perf/bugfix pass

- After CF implementation: review code structure for optimization patches and bugfixes drawn from issues opened on the main Modrinth repo.

## Error handling

- Missing/invalid API key: CF source toggle hidden or shown disabled with a settings prompt; Modrinth paths untouched.
- CF API failures surface through the existing notification manager; cached data served stale when possible.
- Blocked/restricted CF downloads fall back to a manual-download prompt with the file page URL.

## Testing / verification

- Rust: `cargo check`/`cargo clippy` on app-lib; unit tests for CF DTO mapping, MurmurHash2, cross-source matching.
- Frontend: `pnpm prepr` (web + app) from repo root.
- Manual smoke: browse with/without key, install CF mod, CF zip import, update flow.
