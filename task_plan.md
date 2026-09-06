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
**Status:** complete
- [x] 5.1 Settings: curseforge_api_key + migration + sqlx prepare (commit feat(app-lib))
- [x] 5.2 api/curseforge module: structs/api/normalize + murmur2 util
- [x] 5.3 cache.rs CF types + fetch arms + Tauri commands (get_curseforge_*)
- [x] 5.4 Storage source dimension: ContentEntry source+cf_project_id/cf_version_id, FileMetadata source+cf ids, ContentSourceKind::CurseforgeModpack, Instance.preferred_source (migration 20260906130000; commit 66981eaf8)
- [x] 5.5 Fingerprint recognition (murmur2) wiring in content listing (detect_curseforge_metadata in list_content.rs; commit 66981eaf8)
- [x] 5.6 Browse source switch, theming, pinned tabs (commit 275cc4d7c)
- [x] 5.7 Instance source filter + CF key settings UI (commit b27dd30eb: IntegrationsSettings.vue, ContentItem package_source/cf ids/external_url, source metadata filter, CF chip in ContentCardItem)
- [x] 5.8 Project pages dual-source + switch-source + CF install (commit 9c39368cd; switch-source button + CF modpack install commit 19982c621)
- [x] 5.9 CF modpack import/management/updates (commit 19982c621: install_curseforge.rs, InstanceLink::CurseforgeModpack, check/apply CF updates, CF App import preferred_source, .zip manifest import, linked modpack UI parity)
- [x] 5.10 Ads removal (commit 9566166b1: ads.rs + occlusion + helpers/ads.js + consent UI + personalized_ads + PromotionWrapper)
- [x] 5.11 Verification sweep (cargo test 30/30, prepr:lib/app green) + review pass: 11 fixes committed (c129a1a26: Tauri arg types, CF search camelCase, update channel/loader filtering, quilt loader parse, duplicate-pack detection, switch-version cleanup)

### Phase 6: Final report + push confirmation
**Status:** in_progress

## Next Step
Write final report and get user confirmation before pushing feat/curseforge-source to the GitHub fork (9 commits, b54470d8a..c129a1a26).

## Decisions Made
| Decision | Rationale |
|---------|-----------|
| Path classified: architectural | New subsystem (alternate source abstraction) touching app-frontend, app-lib, api-client |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
