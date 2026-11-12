# CurseForge-First API & Cache Performance Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make CurseForge project pages, refresh checks, installs and updates feel immediate, while keeping Modrinth at least as fast as today.

**Architecture:** Remove request fan-out first (version pagination, duplicate refreshes, fingerprint 400s), then add a stale-first cache policy, a CF-specific concurrency/rate governor, and optimistic UI. The Rust backend stays the single source of truth for cached API data; TanStack Query handles in-memory deduplication and paint-first UI.

**Tech Stack:** Rust (`theseus`/`theseus_gui`), SQLite cache, Tauri IPC, Vue 3 + TanStack Query v5, CurseForge REST API, Modrinth API.

**Spec:** This document. Detailed research evidence lives in `%TEMP%\opencode\cf-perf-{measurements,backend,frontend,docs}.md`.

## Global Constraints

- Never raise the fixed CurseForge request cap without a 429 feedback loop. CF publishes no rate-limit contract.
- Never use `CacheBehaviour::Bypass` for a user refresh. A refresh forces the **update-check inputs**, not every metadata cache.
- Each content entry resolves through exactly one source authority (`entry.source`); one API failure must not abort the other source.
- Project pages and content tabs render cached data before any network call. Network results replace it.
- Cache keys must be canonical and bounded. Do not persist keystroke-prefix search responses.
- Preserve the existing content-table design and the project-type icon beside project names.
- Before delivery, clear the app cache as specified in Task 12.

## Review Focus

- A 1,000-file CurseForge project: first paint must use one page, not the whole history; version count must remain correct.
- A cold project page with an expired cache: no duplicate version sweep and no duplicate project request.
- Refresh while a background update check is running: table stays interactive, one list load, visible status.
- Install a CF mod with four dependencies while the pacer is busy: no duplicate file lookups, no dead button, progress visible.
- Offline/429/403: cached content still renders; 403 is never retried; 429 backs off globally.

## Research Evidence

Measured on 2026-11-12 from the user's `app.db` and launcher logs:

| Finding | Evidence |
|---|---|
| CF itself is fast | 110–175 ms per live call; 20-page JEI file sweep = 2.82 s / 1.63 MiB |
| Version fetch is unbounded | `get_mod_files` pageSize 50, up to 1,000 files = up to 20 sequential requests |
| Global pacer | 20 requests / 60 s process-wide; the 21st request sleeps up to 60 s |
| Fingerprints always fail | Cache id is a SHA-1; parser turns it into `[]`; CF returns 400 twice per content listing |
| CF project pages retry 404s | `cf-*/disclosures` requested 3–4× per page, adding 3.3–7.5 s of error state |
| Content list churn | `get_content_items` fired 11× in 17 min, sometimes twice in the same nanosecond |
| Cache is bloated | 118 MiB cached, 84% expired; 25,547 Modrinth version rows, 595 KB CF version blobs |
| Version cache is cold | 66 installed CF projects; only 12 of 28 current-format version rows cached |

Primary sources: [CurseForge REST API](https://docs.curseforge.com/rest-api/), [Modrinth API](https://docs.modrinth.com/api/), [TanStack Query defaults](https://tanstack.com/query/latest/docs/framework/vue/guides/important-defaults), [TanStack caching](https://tanstack.com/query/latest/docs/framework/vue/guides/caching), [RFC 5861](https://www.rfc-editor.org/rfc/rfc5861.html), [reqwest ClientBuilder](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html).

Key API facts:

- CF page size max 50; `index + pageSize <= 10,000`; `modLoaderType` must be paired with `gameVersion`.
- CF batch endpoints exist (`POST /v1/mods`, `POST /v1/mods/files`) but body caps are undocumented; use 50–100 ids per request.
- CF exposes no rate-limit headers, no ETag, no usable `Retry-After` in the observed responses.
- Modrinth supports `If-Modified-Since` (304) but no ETag; it has bulk endpoints and 300 req/min.
- `@tauri-apps/plugin-http` creates a new `reqwest::Client` per call. Keep API traffic in Rust with one shared client.

## Success Metrics

- Cold CF project page: **≤4 CF requests** for first paint; header <1 s warm, <5 s cold.
- 1,000-file project: page 0 paints in <2 s warm; “Load more” fetches the next page on demand; total count matches CF.
- Content refresh on a 150-mod CF instance: **≤3 CF requests** and a non-blocking status line; table never disappears.
- CF install with four dependencies: **≤8 CF requests**; install modal opens before the version list finishes.
- Fingerprint endpoint: **zero 400s** in a normal content session; CF fingerprint matching works.
- Cache after purge: <40 MiB, zero expired rows older than the retention window, no abandoned key schemes.
- Modrinth project/search/update request counts do not increase.

---

### Task 1: Fix CF fingerprint cache and stop the 400 loop

**Files:**
- Modify: `packages/app-lib/src/state/instances/commands/list_content.rs:1781-1844`
- Modify: `packages/app-lib/src/state/cache.rs:2757-2794`
- Modify: `packages/app-lib/src/state/instances/commands/source_link.rs:446-448`
- Test: `packages/app-lib/src/state/cache.rs` unit tests

**Interfaces:**
- `pub async fn get_curseforge_fingerprints(fingerprints: &[u32], cache_behaviour: Option<CacheBehaviour>, pool: &SqlitePool, fetch_semaphore: &FetchSemaphore) -> crate::Result<Option<CachedCFFingerprints>>`
- `fn fingerprint_cache_key(fingerprints: &[u32]) -> String` computes a stable SHA-1 over sorted unique fingerprints.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn fingerprint_cache_key_is_stable_and_distinct() {
    assert_eq!(fingerprint_cache_key(&[3, 1, 2]), fingerprint_cache_key(&[1, 2, 3]));
    assert_ne!(fingerprint_cache_key(&[1, 2]), fingerprint_cache_key(&[1, 3]));
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p theseus --lib fingerprint_cache_key_is_stable_and_distinct`

Expected: FAIL because `fingerprint_cache_key` does not exist.

- [ ] **Step 3: Implement the key helper and pass the actual fingerprint list into the CF fetch**

Change the cache signature. The cache key is only for storage; the HTTP body must use the fingerprint array. Update both call sites in `list_content.rs` and `source_link.rs`. Keep the result best-effort and warn-only.

- [ ] **Step 4: Run the test and a targeted content-list test**

Run: `cargo test -p theseus --lib fingerprint_cache_key_is_stable_and_distinct`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/app-lib/src/state/cache.rs packages/app-lib/src/state/instances/commands/list_content.rs packages/app-lib/src/state/instances/commands/source_link.rs
git commit -m "fix(curseforge): send real fingerprints to the API"
```

---

### Task 2: Cache local Murmur2 fingerprints by file stamp

**Files:**
- Modify: `packages/app-lib/src/state/instances/commands/list_content.rs:1790-1844`
- Modify: `packages/app-lib/src/state/instances/commands/sync_content_files.rs:99-118`
- Add: `CacheValueType::LocalFingerprint` in `packages/app-lib/src/state/cache.rs`
- Migration: `packages/app-lib/migrations/<date>_local-fingerprint.sql` if a new column/table is needed

**Interfaces:**
- `fn local_fingerprint_cache_key(path: &str, size: u64, modified_at_ns: i64) -> String`
- `pub async fn get_local_fingerprint(path: &Path, size: u64, modified_at_ns: i64, state: &State) -> crate::Result<Option<u32>>`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn fingerprint_stamp_changes_with_mtime() {
    assert_ne!(
        local_fingerprint_cache_key("mods/a.jar", 10, 1),
        local_fingerprint_cache_key("mods/a.jar", 10, 2)
    );
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p theseus --lib fingerprint_stamp_changes_with_mtime`

Expected: FAIL because the helper does not exist.

- [ ] **Step 3: Implement the cache and use it before reading the jar**

Reuse the existing `file_hash_cache_key` pattern. On a hit return the cached `u32`; on a miss read the file once, store it, and return it. A file CF does not recognize is negatively cached with the same stamp. `source_link.rs` uses the same helper.

- [ ] **Step 4: Run tests**

Run: `cargo test -p theseus --lib fingerprint_stamp_changes_with_mtime`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/app-lib/src/state/cache.rs packages/app-lib/src/state/instances/commands/list_content.rs packages/app-lib/src/state/instances/commands/sync_content_files.rs packages/app-lib/migrations
git commit -m "perf(curseforge): cache local fingerprints by file stamp"
```

---

### Task 3: Page-window CurseForge versions

**Files:**
- Modify: `packages/app-lib/src/api/curseforge/api.rs:285-335`
- Modify: `packages/app-lib/src/state/cache.rs:2426-2509`
- Modify: `packages/app-lib/src/api/cache.rs:84-96`
- Modify: `apps/app/src/api/cache.rs:176-195`
- Modify: `apps/app/build.rs:46-49`
- Modify: `apps/app-frontend/src/helpers/curseforge-project.ts:26-36`
- Test: Rust unit tests for page keys

**Interfaces:**
- `pub struct CFModFilesPage { pub files: Vec<CFFile>, pub total_count: u32, pub index: u32 }`
- `pub async fn get_mod_files_page(mod_id: i64, game_version: Option<&str>, mod_loader: Option<i64>, index: u32, fetch_semaphore: &FetchSemaphore, pool: &SqlitePool) -> crate::Result<CFModFilesPage>`
- `pub async fn get_curseforge_project_versions_page(project_id: &str, page: u32, cache_behaviour: Option<CacheBehaviour>) -> crate::Result<Option<CFProjectVersionsPage>>`
- `CFProjectVersionsPage { versions: Vec<SourceVersion>, total_count: u32, page: u32 }`
- Frontend: `getCfVersionsPage(projectId: string, page = 0, cacheBehaviour?: CacheBehaviour)`; `getCfVersions` becomes a page-0 compatibility wrapper.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn version_page_key_is_page_specific() {
    assert_ne!(version_page_key("238222", 0), version_page_key("238222", 1));
}

#[tokio::test]
async fn first_page_reports_total_count() {
    // use a fixture/mocked CF response here; assert total_count > files.len()
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p theseus --lib version_page_key_is_page_specific`

Expected: FAIL because the page API does not exist.

- [ ] **Step 3: Implement page 0 as the first-paint primitive**

`get_mod_files_page` makes exactly one CF request. `get_curseforge_project_versions_page` caches by `"{project_id}:{page}"`, stores `total_count`, and returns page 0 on the project page. Keep `get_mod_files` as the full-history helper only for the Versions tab’s explicit “load more” flow; remove the 1,000-file cap and page until `total_count` is reached there.

- [ ] **Step 4: Run tests**

Run: `cargo test -p theseus --lib version_page_key_is_page_specific`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/app-lib/src/api/curseforge/api.rs packages/app-lib/src/state/cache.rs packages/app-lib/src/api/cache.rs apps/app/src/api/cache.rs apps/app/build.rs apps/app-frontend/src/helpers/curseforge-project.ts
git commit -m "perf(curseforge): page version history instead of sweeping it"
```

---

### Task 4: Project page and versions tab first paint

**Files:**
- Modify: `apps/app-frontend/src/pages/project/Index.vue:936-990`
- Modify: `apps/app-frontend/src/pages/project/Versions.vue`
- Modify: `apps/app-frontend/src/pages/project/Description.vue:38-47`
- Modify: `packages/ui/src/components/project/ProjectSidebarDetails.vue:357-364`

**Interfaces:**
- `versionPage` ref and `loadMoreVersions()` in `Index.vue`.
- `ProjectSidebarDetails` query: `enabled: computed(() => !props.project.id.startsWith('cf-'))`, `retry: false`.

- [ ] **Step 1: Write the failing test / red check**

Use a manual check: open a cold CF project and confirm the current page performs a full sweep before the versions list. Record the request count from the Rust debug log. This is the red baseline.

- [ ] **Step 2: Await the project before starting versions**

In `fetchCfProjectData`, fetch `getCfProject` first, then `getCfVersionsPage(requestedId, 0, 'stale_while_revalidate')`. Do not start versions before the project row exists.

- [ ] **Step 3: Remove the duplicate full revalidation**

Delete the `getCfVersions(..., 'must_revalidate')` block at `Index.vue:987-995`. Revalidate page 0 only, in the background, once.

- [ ] **Step 4: Make the versions tab page-aware**

Show the first page immediately. Add a “Load more versions” control that requests page 1, 2, … and appends. Render `total_count` in the tab/header. Do not fetch the full history for the project page.

- [ ] **Step 5: Skip CF-only Modrinth endpoints**

Guard the disclosures query for `cf-` projects. A 404 is not retried; CF projects do not have Modrinth disclosures.

- [ ] **Step 6: Load the description first**

Remove the 250 ms debounce in `Description.vue` for CF projects or fetch the description concurrently with page-0 versions, so the most visible panel does not sit behind version pagination.

- [ ] **Step 7: Verify**

Run `pnpm prepr:frontend:app` and manually open a cold 1,000-file CF project. Expected: project header and page-0 versions visible with one project request and one file page; no disclosures requests; no full sweep.

- [ ] **Step 8: Commit**

```bash
git add apps/app-frontend/src/pages/project/Index.vue apps/app-frontend/src/pages/project/Versions.vue apps/app-frontend/src/pages/project/Description.vue packages/ui/src/components/project/ProjectSidebarDetails.vue
git commit -m "perf(curseforge): paint project page from the first version page"
```

---

### Task 5: Refresh is parallel, stale-first, and observable

**Files:**
- Modify: `apps/app-frontend/src/pages/instance/content/index.vue:1878-1974`
- Modify: `apps/app-frontend/src/pages/instance/layout.vue:287-332`
- Modify: `packages/ui/src/layouts/shared/content-tab/layout.vue:334-344, 1190-1205`
- Modify: `packages/app-lib/src/state/instances/commands/check_content_updates.rs:45-65, 501-640`

**Interfaces:**
- `RefreshContentOptions { mode: 'user' | 'background'; report?: (phase: RefreshPhase) => void }`
- `check_content_updates_with_cache_behaviours(..., options)` uses `MustRevalidate` only for `cf_project_latest` in user mode; file metadata stays `StaleWhileRevalidateSkipOffline`.

- [ ] **Step 1: Write the failing backend test**

Add a unit test that asserts a user refresh asks for `cf_project_latest` with `MustRevalidate` and `cf_file` with `StaleWhileRevalidateSkipOffline`.

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p theseus --lib user_refresh_uses_targeted_cache_behaviour`

Expected: FAIL.

- [ ] **Step 3: Remove `Bypass` from the frontend refresh**

Call `refresh_content_updates(instance.id, 'must_revalidate')` or the new options API. Never bypass project/file/description caches for a refresh.

- [ ] **Step 4: Run check and list reload in parallel**

Replace the serial `await refresh; await initProjects` with `Promise.all`. Keep the table visible. Add a status line: “Checking for updates…”, “No updates”, “N updates available”. Disable only the refresh button.

- [ ] **Step 5: Deduplicate content loads**

Invalidate `instanceKeys.content` only when the sync changed something. Give `contentSyncQuery` `staleTime: 30_000`, and collapse the page’s `loadInitialContent`/`initProjects` path to one fetch.

- [ ] **Step 6: Verify**

Run the backend test, `pnpm prepr:frontend:app`, then manual: refresh a 150-mod CF instance. Expected: one update-check pass, one list load, no table blank, visible status.

- [ ] **Step 7: Commit**

```bash
git add apps/app-frontend/src/pages/instance/content/index.vue apps/app-frontend/src/pages/instance/layout.vue packages/ui/src/layouts/shared/content-tab/layout.vue packages/app-lib/src/state/instances/commands/check_content_updates.rs
git commit -m "perf: make refresh parallel and stale-first"
```

---

### Task 6: Cache policy, canonical keys, and purge

**Files:**
- Modify: `packages/app-lib/src/state/cache.rs:158-190, 2997-3070`
- Migration: `packages/app-lib/migrations/<date>_cache-expiry-index.sql`
- Modify: `packages/ui/src/utils/curseforge-search.ts` (client debounce)
- Modify: `apps/app-frontend/src/layouts/shared/browse-tab/composables/use-browse-search.ts` (canonical params)

**Policy:**
| Data | TTL | Behaviour |
|---|---|---|
| `CurseforgeFile` | 30 days | immutable; availability checked at download |
| `CurseforgeProject` | 6 h | stale-while-revalidate |
| `CurseforgeProjectVersions` page | 30 min | SWR; page table owns freshness |
| `CurseforgeDescription` | 7 days | SWR |
| `CurseforgeSearchResults` | 45 s | debounce 300 ms, minimum 2 chars |
| `CurseforgeFileChangelog` | 30 days | immutable |
| `Modrinth Project` | 24 h + `If-Modified-Since` | 304 on revalidate |
| `Modrinth Search` | 60 s | canonical query |

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn cf_file_cache_expires_in_days_not_minutes() {
    assert!(CacheValueType::CurseforgeFile.expiry() >= 30 * 24 * 60 * 60);
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p theseus --lib cf_file_cache_expires_in_days_not_minutes`

Expected: FAIL.

- [ ] **Step 3: Implement TTLs and batch purge**

Add `CREATE INDEX cache_expires ON cache(expires)` and `CREATE INDEX cache_data_type ON cache(data_type)`. Purge in batches of 500 with `DELETE ... WHERE expires < ? LIMIT 500` until no rows. Remove abandoned CF key schemes during the first cleanup. Keep `VACUUM` out of the hot path; run it on the release cleanup task.

- [ ] **Step 4: Canonicalize search keys**

Debounce search input by 300 ms and require 2 characters before persisting. Normalize parameter order and sorted arrays. Do not cache single-letter prefixes.

- [ ] **Step 5: Batch reads, writes, and id lookups**

Chunk `POST /v1/mods` at 100 ids like `get_files`. Batch `curseforge_project_types_for` into one `SELECT ... WHERE id IN (SELECT value FROM json_each(?))`. Batch update-check upserts with the existing `json_each` pattern. Hoist `installed_cross_source` out of the per-instance loop and scope the source-link query to the project ids. Forward `cacheBehaviour` in the `_many` wrappers in `packages/app-lib/src/api/cache.rs:26-36` instead of dropping it. Store only the fields the launcher renders in `cf_project_latest_v1` (latest-file indexes plus resolved file metadata), not the full project payload.

- [ ] **Step 6: Use Modrinth conditional requests**

For Modrinth project/version GETs, store `Last-Modified` and send `If-Modified-Since`; treat 304 as a cache hit without rewriting the row. CF has no validators, so do not add conditional requests there.

- [ ] **Step 7: Verify**

Run the TTL test and inspect `SELECT data_type, COUNT(*), SUM(LENGTH(data)) FROM cache GROUP BY data_type` after a short session. Expected: no keystroke rows, cache growth bounded; no `_many` caller expects a bypass that is silently ignored.

- [ ] **Step 8: Commit**

```bash
git add packages/app-lib/src/state/cache.rs packages/app-lib/migrations packages/ui/src/utils/curseforge-search.ts apps/app-frontend/src/layouts/shared/browse-tab/composables/use-browse-search.ts
git commit -m "perf: cache immutable CurseForge data and purge expired rows"
```

---

### Task 7: Adaptive CF governor, separate semaphore, request coalescing

**Files:**
- Modify: `packages/app-lib/src/api/curseforge/api.rs:24-90`
- Modify: `packages/app-lib/src/state/mod.rs:300-325`
- Modify: `packages/app-lib/src/state/cache.rs:1313-1506`
- Reuse pattern: `packages/app-lib/src/util/fetch.rs:216-324`

**Interfaces:**
- `struct CfGovernor { capacity: usize, refill_per_minute: u32, backoff_until: Instant }`
- `async fn acquire_cf_slot(&self)`; on 429 set `backoff_until` and halve refill.
- `FetchSemaphore` for CF with 4 permits, separate from Modrinth/downloads.
- `single_flight: DashMap<(CacheValueType, String), Arc<Notify>>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn cf_governor_backs_off_after_429() {
    // first acquire succeeds; inject a 429; next acquire waits until backoff.
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p theseus --lib cf_governor_backs_off_after_429`

Expected: FAIL.

- [ ] **Step 3: Implement the governor and per-source semaphore**

Start conservative: capacity 30, refill 30/min, 4 concurrent. Never retry 403. Log every 429 with the endpoint and remaining backoff. Pass the CF semaphore through every CF call instead of the shared `api_semaphore`.

- [ ] **Step 4: Add single-flight around cache fetches**

First caller for a key performs the fetch; concurrent callers await the same `Notify`/`OnceCell`. Remove the entry on error. Bound the map by the cache key set or a short TTL.

- [ ] **Step 5: Verify**

Run the governor test and a cold project-page trace. Expected: page-0 request burst completes in one RTT; no global 60 s stall after a 20-page sweep.

- [ ] **Step 6: Commit**

```bash
git add packages/app-lib/src/api/curseforge/api.rs packages/app-lib/src/state/mod.rs packages/app-lib/src/state/cache.rs
git commit -m "perf(curseforge): adaptive governor and request coalescing"
```

---

### Task 8: Install/update request reduction

**Files:**
- Modify: `packages/app-lib/src/api/pack/install_curseforge.rs:106-314`
- Modify: `packages/app-lib/src/api/curseforge/normalize.rs:105-125`
- Modify: `packages/app-lib/src/state/instances/commands/apply_content_install.rs:704-1131`
- Modify: `packages/app-lib/src/state/instances/commands/apply_content_update.rs:97-215`
- Modify: `packages/app-lib/src/state/instances/commands/source_link.rs`

**Interfaces:**
- `SourceVersionFile` gains `download_url: Option<String>`, `is_available: Option<bool>`, `dependencies: Vec<CFFileDependency>`.
- `add_project_from_curseforge_file(..., pre_resolved: Option<&SourceVersionFile>) -> Result<String>`.
- `install_curseforge_project_with_dependencies` accepts a pre-resolved root file.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn resolved_cf_file_carries_download_metadata() {
    // construct a CFFile with allowModDistribution=false and assert normalized fields.
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p theseus --lib resolved_cf_file_carries_download_metadata`

Expected: FAIL.

- [ ] **Step 3: Carry file metadata and remove duplicate lookups**

Drop the live `get_mod_file` calls when the cached `SourceVersionFile` already has the answer. Re-check availability only at the download boundary. Return the resolved file from `add_project_from_curseforge_file` for dependency traversal.

- [ ] **Step 4: Batch dependency levels**

Resolve root dependencies in one `POST /mods`/`get_files` batch per BFS level, then install with bounded concurrency. Preserve the depth/visited guards and per-dependency error isolation.

- [ ] **Step 5: Parallelize modpack manifest fetches**

In `pack_from_manifest`, issue the `get_mods` and `get_files` chunks concurrently, bounded by the CF semaphore. Keep chunk size 100.

- [ ] **Step 6: Verify**

Run the unit test, then install a 4-dependency CF mod cold. Expected request count ≤8 and no duplicate file lookup in the trace.

- [ ] **Step 7: Commit**

```bash
git add packages/app-lib/src/api/pack/install_curseforge.rs packages/app-lib/src/api/curseforge/normalize.rs packages/app-lib/src/state/instances/commands/apply_content_install.rs packages/app-lib/src/state/instances/commands/apply_content_update.rs packages/app-lib/src/state/instances/commands/source_link.rs
git commit -m "perf(curseforge): resolve install files once and batch dependencies"
```

---

### Task 9: Install/update UX: open immediately, progress, optimistic rows

**Files:**
- Modify: `apps/app-frontend/src/providers/content-install.ts:941-1140`
- Modify: `apps/app-frontend/src/pages/project/Index.vue:1100-1125`
- Modify: `apps/app-frontend/src/pages/instance/content/index.vue:1140-1427`
- Modify: `packages/ui/src/layouts/shared/content-tab/components/modals/content-updater-modal/index.vue`

- [ ] **Step 1: Write the failing test / red check**

Manual: click Install on a cold CF project; confirm the modal waits for the full version list. Record the delay.

- [ ] **Step 2: Pass known data into install**

Project page passes `data.value` and page-0 versions into `install()`. The install modal opens immediately; it refreshes versions in the background if needed. Same for modpacks.

- [ ] **Step 3: Always clear `installing`**

Wrap the install path in `try/finally`; clear on duplicate-modpack early returns, no-version errors, and catch paths.

- [ ] **Step 4: Optimistic update/switch rows**

Patch `instanceKeys.content`/`linkedContent` before the backend call: version, `has_update: false`, `update_version_id: null`. Close the updater modal on confirm. Roll back on error.

- [ ] **Step 5: Stale-first updater and hover prefetch**

Remove `bypass` from `getUpdaterProjectVersions`; use cached versions with a 60 s staleTime. Prefetch on pointerenter of the update/switch affordance.

- [ ] **Step 6: Stop blocking Version.vue first paint**

Replace the top-level `await Promise.all([refreshEnrichment(), refreshChangelog()])` with fire-and-forget calls or Suspense-backed queries; render the version card from `props.versions` and fill dependencies/changelog with their existing loading states.

- [ ] **Step 7: Verify**

Run `pnpm prepr:frontend:app`, then manual install/update on a cold CF project. Expected: modal opens immediately, install button never sticks, row updates before the full list reload, version page paints without waiting for enrichment.

- [ ] **Step 8: Commit**

```bash
git add apps/app-frontend/src/providers/content-install.ts apps/app-frontend/src/pages/project/Index.vue apps/app-frontend/src/pages/project/Version.vue apps/app-frontend/src/pages/instance/content/index.vue packages/ui/src/layouts/shared/content-tab/components/modals/content-updater-modal/index.vue
git commit -m "perf: open install/update flows immediately and patch rows optimistically"
```

---

### Task 10: TanStack Query defaults, deduplication, and content sync

**Files:**
- Modify: `apps/app-frontend/src/main.js`
- Modify: `apps/app-frontend/src/pages/instance/query-options.ts`
- Modify: `apps/app-frontend/src/pages/instance/layout.vue`
- Modify: `apps/app-frontend/src/pages/instance/content/index.vue`
- Modify: `packages/ui/src/layouts/shared/content-tab/layout.vue`

**Global defaults:**
```ts
staleTime: 5 * 60_000
gcTime: 30 * 60_000
refetchOnWindowFocus: false
refetchOnReconnect: true
networkMode: 'offlineFirst'
retry: (count, error) => count < 2 && !isAuthOr429(error)
```

Per-class overrides: search 45 s, project 6 h, versions 30 min, changelog/description `Infinity`, tags `'static'`.

- [ ] **Step 1: Write the failing test**

Use the existing frontend test setup (or a query-options unit test) to assert the content query does not refetch on focus and uses `staleTime >= 30_000`.

- [ ] **Step 2: Run it and confirm it fails**

Run the app-frontend test command for that test file.

Expected: FAIL.

- [ ] **Step 3: Apply defaults and canonical keys**

Use `['cf','project',id]`, `['cf','project',id,'versions',page]`, `['cf','project',id,'description']`, `['cf','project',id,'dependencies']`. Share `get_install_candidates` between the project page and install modal.

- [ ] **Step 4: Reduce content sync churn**

`contentSyncQuery` returns `changed: boolean`; invalidate only when changed. Set `staleTime: 30_000`, `gcTime: 30 * 60_000`. Replace eager post-install full update checks with scoped invalidation.

- [ ] **Step 5: Verify**

Run frontend test + `pnpm prepr:frontend:app`; manually visit the content tab five times. Expected: one sync/list load, no window-focus refetch, no double-load.

- [ ] **Step 6: Commit**

```bash
git add apps/app-frontend/src/main.js apps/app-frontend/src/pages/instance/query-options.ts apps/app-frontend/src/pages/instance/layout.vue apps/app-frontend/src/pages/instance/content/index.vue packages/ui/src/layouts/shared/content-tab/layout.vue
git commit -m "perf: stale-first queries and single content sync"
```

---

### Task 11: Bulk update eligibility and progress

**Files:**
- Modify: `packages/ui/src/layouts/shared/content-tab/layout.vue:799-810, 976-1012, 1180-1191`
- Modify: `apps/app-frontend/src/pages/instance/content/index.vue:1120-1148`
- Modify: `packages/app-lib/src/state/instances/commands/apply_content_update.rs:496-756`

- [ ] **Step 1: Write the failing test / red check**

Manual: on a CF-only instance with updates, click “Update all”. Current result: nothing happens. Record it.

- [ ] **Step 2: Make the button honest**

Compute eligibility with the same filter as the candidates. If CF rows are excluded, show a disabled button with “CurseForge projects must be updated individually”; do not show an inert enabled button.

- [ ] **Step 3: Forward progress**

Accept `onProgress` in `bulkUpdateSelections`, subscribe to `install_job` progress, update each row as its item completes, and keep the selection bar visible with “Updating X of Y”.

- [ ] **Step 4: Verify**

Manual bulk update on a mixed instance. Expected: visible progress, rows clear individually, CF rows either supported or explicitly disabled.

- [ ] **Step 5: Commit**

```bash
git add packages/ui/src/layouts/shared/content-tab/layout.vue apps/app-frontend/src/pages/instance/content/index.vue packages/app-lib/src/state/instances/commands/apply_content_update.rs
git commit -m "perf: honest bulk update eligibility and progress"
```

---

### Task 12: Release cleanup, cold benchmark, and installer

**Files:**
- No source changes unless the cleanup reveals an issue.
- Update: `apps/app/Cargo.toml`, `packages/app-lib/Cargo.toml`, `apps/app-frontend/package.json`

- [ ] **Step 1: Close the launcher and clear all cache**

Preserve user data (`profiles`, `store`, instance files, settings). Clear only cache:

```powershell
$app = "$env:APPDATA\ModrinthApp"
# cache table in app.db
node -e "const {DatabaseSync}=require('node:sqlite');const db=new DatabaseSync(process.argv[1]);db.exec('DELETE FROM cache; VACUUM;');db.close()" "$app\app.db"
# cached downloads/metadata
Remove-Item "$app\caches\*" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$app\meta\*" -Recurse -Force -ErrorAction SilentlyContinue
```

- [ ] **Step 2: Verify the cache is empty**

```powershell
node -e "const {DatabaseSync}=require('node:sqlite');const db=new DatabaseSync(process.argv[1],{readOnly:true});console.log(db.prepare('SELECT COUNT(*) AS n FROM cache').get());db.close()" "$env:APPDATA\ModrinthApp\app.db"
```

Expected: `n = 0`; `app.db` drops below ~150 MB after vacuum.

- [ ] **Step 3: Cold benchmark**

Open a cold 300+ file CF project and a 150-mod CF instance. Record CF request count and wall time for project page, refresh, and one mod update. Expected targets from “Success Metrics”.

- [ ] **Step 4: Bump version and build**

Set `0.2.2-local`, run `cargo fmt --all`, `cargo clippy -p theseus -p theseus_gui --all-targets`, `cargo test -p theseus --lib`, `pnpm prepr:frontend:lib`, `pnpm prepr:frontend:app`, then build NSIS with the prod `.env`.

- [ ] **Step 5: Verify the installer**

Confirm `wss://api.modrinth.com`, no localhost, version `0.2.2-local`, and the installer exists under `target\release\bundle\nsis`.

- [ ] **Step 6: Dispatch the usual reviewer**

Run the pre-delivery reviewer on the whole branch. Fix any Critical/High/Medium findings, then commit and push.

```bash
git add -A
git commit -m "perf: make CurseForge pages, refresh, and installs responsive"
git push origin feat/curseforge-source
```

---

## Deliberately Not Doing

- Do not add a new caching framework or replace SQLite; the current cache is sufficient once request shape and TTLs are fixed.
- Do not raise the CF pacer constant before the 429 feedback loop exists.
- Do not fetch full CF file history on first paint or in install dialogs.
- Do not fetch CF descriptions/changelogs in list views.
- Do not remove Modrinth-only features or the existing content-table design.

## Execution Notes

- Use `git show b5ffbeb05` as the pre-change baseline.
- Keep the measured reports in `%TEMP%\opencode` available during implementation.
- Re-run the measurement script after each phase; do not rely on static request counts alone.
- The cache cleanup in Task 12 is mandatory before delivery and must be repeated after the final installer build.
