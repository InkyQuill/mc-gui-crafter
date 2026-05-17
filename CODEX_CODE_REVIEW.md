# CODEX Code Review

Date: 2026-05-17

Scope: full project audit against `docs/roadmap.md` Phases 1-5, because this directory is not a Git repository and there is no reviewable diff/base commit available.

## Verdict

Phases 1-5 are not complete. The repository contains a broad UI scaffold and Rust modules for most promised areas, but several core paths are either broken at runtime, frontend-only, or placeholder-quality. The current app is closer to a prototype than a completed main-functionality milestone.

## Verification

Commands run:

- `pnpm exec svelte-check --tsconfig ./tsconfig.json` -> failed with 11 errors and 17 warnings.
- `pnpm build` -> passed, with Svelte accessibility warnings.
- `cargo build` in `src-tauri/` -> passed, with warnings.
- `cargo test` in `src-tauri/` -> passed, but there are 0 tests.
- `git status --short` -> failed because `/home/inky/Development/gui-crafter` is not a Git repository.

## Critical Findings

### 1. New-project save/save-as cannot persist in Tauri

Files: `src/lib/components/Toolbar.svelte`, `src/lib/stores/project.svelte.ts`, `src-tauri/src/commands.rs`, `src-tauri/src/format/mod.rs`

The toolbar asks for a save path and only sets the frontend `project.projectPath`. `project.saveProject()` then calls `api.projectSave()` without passing that path. The backend `project_save` command calls `format::save_to_mcgui(project)`, and `save_to_mcgui` requires `project.project_path` to already be set. New projects set `project.project_path = None`, and no command exists to set it.

Impact: the Phase 1 deliverable "create a new project, place slots and textures, save and reopen" fails for new projects in the real Tauri backend. Browser fallback masks this because `mockInvoke("project_save")` always returns success.

Evidence:

- Frontend path assignment only: `src/lib/components/Toolbar.svelte:19-28`
- Save ignores the path: `src/lib/stores/project.svelte.ts:96-102`
- Backend save has no path parameter: `src-tauri/src/commands.rs:65-74`
- Backend requires `project.project_path`: `src-tauri/src/format/mod.rs:73-75`

Suggested fix: add `project_save_as(path)` or make `project_save(path?: string)` update `project.project_path` before writing. Then route toolbar save-as and autosave through that API.

### 2. Rust/TypeScript JSON contracts use incompatible enum casing

Files: `src-tauri/src/project/mod.rs`, `src/lib/types.ts`, `src/lib/engine/renderer.ts`, `src/lib/stores/project.svelte.ts`

Rust enums serialize as `Forge`, `Slot`, `LeftToRight`, etc. TypeScript expects lowercase/snake_case values like `forge`, `slot`, `left_to_right`. The frontend renderer switches on `el.type === "slot" | "texture" | ...`, so elements returned from the real backend will not render or edit correctly.

Impact: creating from templates or opening saved files through Tauri can return `type: "Slot"`/`"Texture"`, which does not match the frontend union or renderer cases. This breaks canvas rendering, property panels, export JSON compatibility, and MCP responses.

Evidence:

- Rust enums have no `rename_all`: `src-tauri/src/project/mod.rs:10-33`
- TypeScript unions are lowercase/snake_case: `src/lib/types.ts:1-5`
- Renderer expects lowercase values: `src/lib/engine/renderer.ts:360-376`
- Template elements are Rust enum variants: `src-tauri/src/templates/mod.rs:63-104`

Suggested fix: add `#[serde(rename_all = "snake_case")]` to `ModTarget`, `ElementType`, `FillDirection`, and `AnimationType`. Add round-trip tests for `.mcgui` save/load and Tauri command payloads.

### 3. Frontend edits are often not synced to backend, so save/export loses user work

Files: `src/lib/stores/project.svelte.ts`, `src/lib/components/PropertyPanel.svelte`, `src/lib/components/AnimationTimeline.svelte`, `src/lib/components/LayerPanel.svelte`

Only add/move/remove call backend APIs. Property edits, resize, z-order changes, animation creation/update/binding, and undo/redo mutate local Svelte state only. The backend project remains stale, and save/export read from the backend project.

Impact: users can edit a project visually, see `isDirty`, then save or export an older backend version. This undermines Phases 1, 2, 3, and 5.

Evidence:

- `updateElement` mutates only frontend state: `src/lib/stores/project.svelte.ts:180-200`
- `resizeElement` mutates only frontend state: `src/lib/stores/project.svelte.ts:251-268`
- animation methods mutate only frontend state: `src/lib/stores/project.svelte.ts:283-318`
- layer reorder mutates only frontend state: `src/lib/stores/project.svelte.ts:223-240`
- property panel calls only `project.updateElement`: `src/lib/components/PropertyPanel.svelte:1-10`

Suggested fix: define a single authoritative project store contract. Either push every mutating operation through Tauri commands, or save/export from a serialized frontend snapshot sent to Rust. Avoid split-brain state.

### 4. `svelte-check` fails

Files: `src/lib/engine/renderer.ts`, `src/lib/components/PixelEditor.svelte`, `src/lib/components/ExportDialog.svelte`, `src/lib/api.ts`, `src/lib/stores/project.svelte.ts`, `src/lib/components/LayerPanel.svelte`

The Svelte/TypeScript quality gate fails with 11 errors. The most serious one is `drawSlot` declaring `Container` but returning nothing, which also means slots do not render.

Impact: the app can pass Vite build but still has type and runtime defects. A completed Phase 1-5 codebase should not have a failing `svelte-check`.

Evidence:

- Missing return in `drawSlot`: `src/lib/engine/renderer.ts:379-389`
- Unused API/import/type errors in `src/lib/api.ts:164`, `src/lib/components/ExportDialog.svelte:3`, `src/lib/components/PixelEditor.svelte:11`, `src/lib/components/LayerPanel.svelte:30`
- Verification result: `svelte-check found 11 errors and 17 warnings in 8 files`

Suggested fix: make `svelte-check` part of the default verification script and fix all errors before continuing feature work.

## Major Findings

### 5. MCP server implementation is not a usable built-in MCP server

Files: `src-tauri/src/lib.rs`, `src-tauri/src/mcp/mod.rs`, `docs/adr/003-mcp-integration.md`

The app spawns a task that reads the Tauri process stdin forever and writes JSON-RPC to stdout. That is not a real "built-in MCP server available on launch" for a GUI app, and it risks blocking on app stdin. It also sends `notifications/initialized` before receiving `initialize`, which does not match normal MCP client flow. There is no HTTP transport despite the roadmap requiring it.

Impact: Phase 4 is only partially implemented and likely not connectable by Claude Desktop/AI tools in the way the README describes.

Evidence:

- Always starts stdio task during Tauri setup: `src-tauri/src/lib.rs:28-32`
- Directly reads process stdin and writes stdout: `src-tauri/src/mcp/mod.rs:39-82`
- Emits initialized before handshake: `src-tauri/src/mcp/mod.rs:44-55`
- HTTP transport does not exist in code, though roadmap requires it: `docs/roadmap.md:65`

Suggested fix: implement MCP as a separate `--mcp` mode or a managed sidecar process, use the MCP protocol lifecycle strictly, and add an HTTP/SSE or streamable HTTP transport only if still required.

### 6. MCP tool surface is incomplete versus ADR 003 and roadmap

Files: `src-tauri/src/mcp/mod.rs`, `docs/adr/003-mcp-integration.md`

Several documented tools are missing or only partly implemented. There are no group tools, no `asset_import` MCP tool, and no `project_export` MCP tool in `tools/list`. Resources only list templates and do not implement `resources/read`.

Impact: Phase 4 deliverable "AI tools can create and edit GUI projects programmatically" is materially incomplete.

Evidence:

- Tool definitions include no group tools or MCP `project_export`: `src-tauri/src/mcp/mod.rs:161-242`
- Execution handles no group tools, no `asset_import`, no `project_export`: `src-tauri/src/mcp/mod.rs:261-489`
- ADR requires group, asset import, export, GUI, and animation tools: `docs/adr/003-mcp-integration.md:29-84`

Suggested fix: reconcile `docs/adr/003-mcp-integration.md`, `docs/mcp.md`, and `get_tool_definitions()` into one tested schema, then implement handlers and protocol tests for each advertised tool.

### 7. Exported Java is placeholder-level and likely does not compile

Files: `src-tauri/src/export/mod.rs`

The export pipeline writes files, but generated Java is not production-ready:

- Forge screen hardcodes `imageWidth`/`imageHeight` to 256 instead of the project GUI size.
- Fabric screen hardcodes `backgroundWidth`/`backgroundHeight` to 256.
- `GuiLayout` uses Forge/Mojang classes (`PoseStack`, `GuiGraphics`, `ResourceLocation`) even for Fabric exports.
- Fabric screen calls `layout.renderTexture(context, x, y)` but `GuiLayout.renderTexture` accepts `GuiGraphics`, not Fabric `DrawContext`.
- Animation calls are explicit placeholder comments.
- Slot and text rendering are comments, not implementation.

Impact: Phase 5 deliverable "get a working Screen renderer that compiles in a Forge/Fabric project" is not satisfied.

Evidence:

- Forge hardcoded size: `src-tauri/src/export/mod.rs:162-166`, `src-tauri/src/export/mod.rs:194-195`
- Fabric hardcoded size: `src-tauri/src/export/mod.rs:223-227`, `src-tauri/src/export/mod.rs:255-256`
- Placeholder animation generation: `src-tauri/src/export/mod.rs:261-268`
- Slot/text rendering not implemented: `src-tauri/src/export/mod.rs:60-66`
- Generated shared runtime imports Forge-side classes: `src-tauri/src/export/mod.rs:15-17`

Suggested fix: generate per-loader runtime code or split renderer adapters by loader. Add fixture tests that run `javac` or Gradle compile against minimal Forge/Fabric projects.

### 8. Texture atlas compositor is not an atlas compositor

Files: `src-tauri/src/texture/mod.rs`, `docs/adr/005-export-system.md`

`composite_atlas` creates an image exactly the GUI size and overlays only elements of type `Texture` that have matching asset data. It does not pack multiple textures, does not record UV coordinates, does not render generated slot/progress/fluid/energy/text visuals, and silently skips missing template assets such as `textures/background.png`.

Impact: exported textures are often empty/transparent or incomplete. This fails the Phase 5 texture atlas requirement and makes generated screens visually wrong.

Evidence:

- Only texture elements are considered: `src-tauri/src/texture/mod.rs:10-18`
- No UV metadata is returned: `src-tauri/src/texture/mod.rs:31-35`
- ADR requires bin packing and UV coordinates: `docs/adr/005-export-system.md:34-43`

Suggested fix: decide whether export is "composited final background" or "packed atlas with UVs"; implement that model explicitly and include missing-asset errors instead of silent skips.

### 9. Pixel editor edits are not persisted to backend/project files

Files: `src/lib/components/PixelEditor.svelte`, `src/lib/components/AssetLibrary.svelte`, `src-tauri/src/commands.rs`

The pixel editor saves by returning a new data URL to `AssetLibrary`, which updates only the frontend `assetDataUrls` map. It does not update `project.texture_data` in Rust. There is no `asset_update` command.

Impact: Phase 3 "can create or import textures and place them on the GUI without external tools" is only partly true. Pixel edits are lost on save/export/reopen in Tauri.

Evidence:

- Pixel editor emits data URL only: `src/lib/components/PixelEditor.svelte:105-112`
- Asset library only updates frontend map: `src/lib/components/AssetLibrary.svelte:64-70`
- Backend commands include import/list/remove/get, but no update: `src-tauri/src/commands.rs:147-250`

Suggested fix: add `asset_update(name, png_bytes/base64)` and route pixel editor saves through it. Reload the backend asset list afterward.

### 10. Animation preview and export are not wired to actual rendering

Files: `src/lib/components/AnimationTimeline.svelte`, `src/lib/engine/renderer.ts`, `src-tauri/src/export/mod.rs`

The timeline exposes a scrubber/play button and binding UI, but the renderer does not read timeline `previewValue` or animation definitions. Progress/fluid/energy elements render fixed visual fills. Export emits placeholder comments for users to manually bind menu data.

Impact: Phase 2 animation preview and Phase 5 animation export are not complete.

Evidence:

- Renderer draws progress as a static indicator: `src/lib/engine/renderer.ts:427-455`
- Fluid/energy are hardcoded partial fills: `src/lib/engine/renderer.ts:475-500`
- Export placeholders: `src-tauri/src/export/mod.rs:261-268`

Suggested fix: store animation state centrally, have renderer evaluate animation bindings, and generate concrete exported calls from project animations.

## Medium Findings

### 11. Undo/redo is frontend-only and does not preserve backend state

Files: `src/lib/stores/project.svelte.ts`, `docs/architecture.md`

Architecture says the Rust backend maintains a command-based undo stack. Actual undo/redo is a frontend array of closures. Undo/redo does not call backend commands, so saving/exporting after undo/redo can use stale backend state.

Evidence:

- Frontend closure stack: `src/lib/stores/project.svelte.ts:12-16`, `src/lib/stores/project.svelte.ts:202-218`
- Architecture claims backend command stack: `docs/architecture.md:146-161`

Suggested fix: either move undo/redo to backend as documented or make the frontend the sole source of truth and serialize snapshots to backend.

### 12. Layer visibility is UI-only and not reflected on canvas/save/export

Files: `src/lib/components/LayerPanel.svelte`, `src/lib/engine/renderer.ts`

Layer visibility toggles only update a local `hiddenElements` set in the panel. The renderer never reads that set, and the project model has no visibility field.

Impact: the Phase 2 "visibility toggle" is cosmetic in the layer list and does not affect the canvas or output.

Evidence:

- Local hidden set: `src/lib/components/LayerPanel.svelte:6-20`
- Renderer iterates all `project.elements`: `src/lib/engine/renderer.ts:351-357`

Suggested fix: add `visible` to the element model or an editor-only visibility map shared with the renderer, then decide whether hidden layers export.

### 13. Template backgrounds reference assets that do not exist

Files: `src-tauri/src/templates/mod.rs`, `src/lib/engine/renderer.ts`

Templates insert a background texture element with `asset: "textures/background.png"`, but no texture data is created. The renderer falls back to a checkerboard placeholder, and export silently skips the missing asset.

Impact: templates are not actual Minecraft GUI templates; they are slot layouts with missing background assets.

Evidence:

- Template background asset reference: `src-tauri/src/templates/mod.rs:63-67`
- Texture fallback placeholder: `src/lib/engine/renderer.ts:391-424`
- Export compositor silently skips missing texture data: `src-tauri/src/texture/mod.rs:15-27`

Suggested fix: embed default template texture assets or generate deterministic template backgrounds during export.

### 14. Recent projects exist only as localStorage helpers and are not visible in UI

Files: `src/lib/stores/project.svelte.ts`, `src/lib/components/NewProjectDialog.svelte`, `src/lib/components/Toolbar.svelte`

`ProjectStore.getRecentProjects()` and `addRecentProject()` exist, but no component renders or opens from the recent-project list.

Impact: Phase 2 "Recent projects list" is not complete.

Evidence: `src/lib/stores/project.svelte.ts:348-358`

Suggested fix: add recent-project UI to the toolbar/open menu or new-project dialog and validate paths before opening.

### 15. No automated tests cover project format, commands, MCP, export, or renderer contracts

Files: whole project

`cargo test` reports 0 tests. There are no frontend tests. The riskiest code paths have no coverage: `.mcgui` round-trip, enum serialization, save-as, MCP tools, export output, texture compositing, and frontend/backend sync.

Impact: regressions are very likely, and several current regressions would have been caught by minimal tests.

Suggested first tests:

- Rust `.mcgui` save/load round-trip with assets and animations.
- Serde JSON casing snapshot tests.
- Tauri command unit tests for new/save-as/open/list.
- Export fixture snapshot plus Java compile smoke test.
- Frontend store tests for mutations syncing to backend API calls.

## Phase Completion Matrix

| Phase | Claimed goal | Observed status |
|---|---|---|
| Phase 1 Foundation | scaffold, `.mcgui`, canvas, basic toolbar, place/save/reopen | Partially implemented, but save-as is broken, slot rendering has a missing return, backend/frontend enum contract is broken, and property changes do not persist. |
| Phase 2 Editing & Templates | selection/drag/resize/layers/templates/animations/undo/autosave/recent | Mostly UI scaffold. Resize, z-order, visibility, animations, undo/redo, autosave, and recent projects are incomplete or frontend-only. |
| Phase 3 Texture Tools | import, asset library, pixel editor, palette, texture replacement, UV selector, actual textures | Import/list is partly implemented. Pixel edits are not persisted. UV region selector is absent. Template and missing textures fall back to placeholders. |
| Phase 4 MCP Server | JSON-RPC over stdio, all tools, shared state, sync, docs, HTTP | Partial custom JSON-RPC implementation. Protocol lifecycle and process model are questionable. Missing group/export/import/HTTP/resource-read pieces. |
| Phase 5 Export Pipeline | atlas, layout JSON, runtime Java, Forge/Fabric/NeoForge, preview, README | Writes files, but generated Java is placeholder/likely uncompilable. NeoForge not exposed. No preview tree before writing. Atlas is not really an atlas. |

## Documentation Drift

- `docs/roadmap.md` still marks Phases 1-5 as "Not started", contradicting the current claim and code state.
- `README.md` says features are "planned" but also describes MCP startup as if it exists.
- `docs/architecture.md` documents backend undo/redo and `animation.svelte.ts`, neither of which exists as implemented.
- ADR 005 says NeoForge is in scope conceptually, while export UI only allows Forge/Fabric.

## Recommended Next Steps

1. Fix the data contract first: serde `rename_all`, save-as path handling, and a single source of truth for project mutations.
2. Make `svelte-check` pass and add it to the default verification command.
3. Add round-trip tests for `.mcgui` and command-level integration tests before expanding features.
4. Reclassify roadmap checkboxes honestly, then complete one vertical workflow: new project -> add/edit elements/assets/animations -> save -> reopen -> export -> compile generated sample.
5. Treat MCP and export as incomplete subsystems until they have protocol/fixture tests.
