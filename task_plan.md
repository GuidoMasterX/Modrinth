# Task Plan: CurseForge Alternate Source for Modrinth App

## Goal
Add CurseForge as an alternate metadata/package source to the Modrinth desktop launcher (app-frontend + app-lib), with Modrinth always preferred, per-instance source switching, CF-exclusive project recognition, color-coded UI (green Modrinth / dark orange CurseForge), CF modpack import & management, ad removal, pin-able filter tabs, and performance improvements.

## Source of truth
- Monorepo: apps/app-frontend (Vue 3), packages/app-lib (Tauri backend lib), packages/api-client, packages/ui
- User brief (m0001): CF source parity for browse, instance management, modpack import/export; Modrinth preferred.

## Phases

### Phase 1: Research � app architecture
**Status:** complete
Map how app-frontend/app-lib use the Modrinth API: search/browse, project detail pages, instance content management, modpack import/export, "unknown" project handling, ads, filter tabs. (See findings.md.)

### Phase 2: Research � CurseForge API and other launchers
**Status:** complete
CF API: user-supplied x-api-key; CDN downloads require key since 2026-07-16. Endpoints + modpack manifest format researched (findings.md).

### Phase 3: Clarifying questions + design approval
**Status:** complete
- ANSWERED: API access = user-supplied key in settings (user has personal key from console.curseforge.com). CDN downloads need key since 2026-07-16.

### Phase 4: Spec + implementation plan
**Status:** complete
docs/superpowers/specs/2026-09-06-curseforge-source-design.md + docs/superpowers/plans/2026-09-06-curseforge-source.md.

### Phase 5: Implementation
**Status:** in_progress
- [x] 5.1 Settings: curseforge_api_key + migration + sqlx prepare (commit feat(app-lib))
- [x] 5.2 api/curseforge module: structs/api/normalize + murmur2 util
- [x] 5.3 cache.rs CF types + fetch arms + Tauri commands (get_curseforge_*)
- [x] 5.4 Storage source dimension: ContentEntry source+cf_project_id/cf_version_id, FileMetadata source+cf ids, ContentSourceKind::CurseforgeModpack, Instance.preferred_source (migration 20260906130000; commit 66981eaf8)
- [x] 5.5 Fingerprint recognition (murmur2) wiring in content listing (detect_curseforge_metadata in list_content.rs; commit 66981eaf8). Note: ContentFilter::OnlySource deferred to 5.7 frontend filter work
- [x] 5.6 Phase 2 frontend: browse source switch, theming, pinned tabs (commit 275cc4d7c: curseforge-search.ts composable, use-browse-search activeSource + CF branches, sidebar/layout toggle + filter branches, Browse.vue CF search callback + categories, use-curseforge-key composable, --color-source-* tokens, pinned_browse_tabs settings auto-remember/restore; also CF sortField/sortOrder 'asc/desc' fix + CFAuthor in Rust)
- [ ] 5.7 Phase 3: instance source filter + CF key settings UI
- [ ] 5.8 Phase 4: project pages dual-source + switch-source
- [ ] 5.9 Phase 5: CF modpack import/management/updates
- [ ] 5.10 Phase 6: ads removal
- [ ] 5.11 Phase 7: verification + final report (confirm before push)

### Phase 6: Final report + push confirmation
**Status:** pending

## Next Step
Task 5.6: frontend browse source switch — Rust-side SourceProject DTOs + Tauri commands already exist (get_curseforge_search_results etc.); wire Browse.vue/use-browse-search to a source toggle (Modrinth default, CurseForge when API key set), CF categories via get_curseforge_categories, --color-source-* theming, pinned filter tabs in settings.

## Decisions Made
| Decision | Rationale |
|---------|-----------|
| Path classified: architectural | New subsystem (alternate source abstraction) touching app-frontend, app-lib, api-client |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
