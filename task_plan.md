# Task Plan: CurseForge Alternate Source for Modrinth App

## Goal
Add CurseForge as an alternate metadata/package source to the Modrinth desktop launcher (app-frontend + app-lib), with Modrinth always preferred, per-instance source switching, CF-exclusive project recognition, color-coded UI (green Modrinth / dark orange CurseForge), CF modpack import & management, ad removal, pin-able filter tabs, and performance improvements.

## Source of truth
- Monorepo: apps/app-frontend (Vue 3), packages/app-lib (Tauri backend lib), packages/api-client, packages/ui
- User brief (m0001): CF source parity for browse, instance management, modpack import/export; Modrinth preferred.

## Phases

### Phase 1: Research — app architecture
**Status:** in_progress
Map how app-frontend/app-lib use the Modrinth API: search/browse, project detail pages, instance content management, modpack import/export, "unknown" project handling, ads, filter tabs.

### Phase 2: Research — CurseForge API & other launchers
**Status:** pending
CF API access model (api.curseforge.com requires approved key; alternatives: cfwidget, curse.tools), endpoints for search/categories/project/files, modpack manifest format, how Prism/ATLauncher/gdlauncher integrate.

### Phase 3: Clarifying questions + design approval
**Status:** in_progress
- ANSWERED: API access = user-supplied key in settings (user has personal key from console.curseforge.com). CDN downloads need key since 2026-07-16.

### Phase 4: Spec + implementation plan
**Status:** pending
Write design doc to docs/superpowers/specs/2026-09-06-curseforge-source-design.md, then writing-plans.

### Phase 5: Implementation
**Status:** pending
Sub-phases TBD after design.

### Phase 6: Final report + push confirmation
**Status:** pending

## Next Step
Run codebase research agents and CurseForge web research.

## Decisions Made
| Decision | Rationale |
|---------|-----------|
| Path classified: architectural | New subsystem (alternate source abstraction) touching app-frontend, app-lib, api-client |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
