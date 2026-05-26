# Phase 1-5 Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current prototype into a truthful Phase 1-5 baseline where create/edit/save/reopen/export/web-MCP paths operate on the same live project sessions and are covered by verification.

**Architecture:** Rust is the authoritative source for durable project data used by save, export, undo/redo, and MCP. The app owns multiple project sessions and exposes them in the UI as tabs; durable mutations target either an explicit project session or the active session. Svelte keeps a reactive UI mirror for rendering and editing, but every durable mutation goes through a typed API command and then updates the UI mirror from the command result. Browser-only mock mode remains a development convenience and must follow the same API contract.

**Tech Stack:** Tauri 2, Rust 2021, Svelte 5 runes, TypeScript, PixiJS 8, `.mcgui` ZIP archives, Streamable HTTP MCP hosted by the running app instance, Java code generation for Minecraft Forge/Fabric/NeoForge.

---

## Input Sources

Reviewed:

- `CODEX_CODE_REVIEW.md`
- `GEMINI_CODE_REVIEW.md`
- `DEEPSEEK_CODE_REVIEW.md`
- `docs/roadmap.md`
- `docs/architecture.md`
- `docs/mcp.md`
- `docs/adr/001-technology-stack.md`
- `docs/adr/002-project-format.md`
- `docs/adr/003-mcp-integration.md`
- `docs/adr/004-coordinate-system.md`
- `docs/adr/005-export-system.md`
- `README.md`

Reviewer consensus:

- Slot and other canvas rendering paths have concrete defects.
- Save after creating a new project is broken or misleading.
- Frontend/backend state divergence is the core architectural bug.
- Export is not ready to claim "working Screen renderer that compiles".
- MCP needs protocol/process validation before "complete" status.
- There are effectively no automated tests for the promised workflows.

Reviewer disagreement:

- Gemini rates Phases 1-5 as mostly complete; Codex rates them as substantially incomplete; DeepSeek is between them.
- The disagreement comes from treating UI presence as completion versus requiring durable, tested end-to-end behavior.
- This spec uses the stricter standard: a phase item is complete only when it survives the real Tauri backend, save/reopen, and relevant export/MCP paths.

## Resolved Product Decisions

1. **MCP process model:** MCP is a web MCP server hosted by the currently running app instance. It must mutate live app state and must not require opening a second GUI process or a separate stdio process.
2. **MCP shared live state:** MCP tools operate on Rust-owned project sessions. Tools may target an explicit project/session ID; otherwise they operate on the active tab.
3. **Multi-project editing:** The app opens several projects in one process using a tabbed interface. The Rust backend owns the session list, active session, dirty/path metadata, and undo/redo stacks.
4. **Export target scope:** Forge, Fabric, and NeoForge are all part of this remediation.
5. **Export integration contract:** Export should produce complete, compilable, immediately usable output where practical. It may require a small number of user fixes only for inherently app-specific game data wiring, and those hooks must be explicit in generated code/docs.
6. **Groups and UV selector priority:** Groups and UV sprite-region editing are in scope now, not deferred.
7. **Undo/redo authority:** Undo/redo is backend-based so both the Svelte UI and web MCP can use the same history and mutation semantics.
8. **Implementation model:** Remediation is subagent-driven. Because this workspace is not a Git repository, worktree and commit checkpoints cannot be used unless the project is later initialized as a repo.

## Acceptance Criteria

- `pnpm exec svelte-check --tsconfig ./tsconfig.json` passes with 0 errors.
- `pnpm build` passes.
- `cargo test` in `src-tauri/` passes with meaningful tests.
- `cargo build` in `src-tauri/` passes.
- A real Tauri workflow can create multiple tabbed projects, switch active tabs, add/edit elements, import/edit a PNG, save as `.mcgui`, reopen, and preserve elements/assets/animations/groups/UV regions.
- The canvas rerenders on element property changes, layer visibility changes, asset swaps, animation scrubber changes, and MCP sync changes.
- Backend undo/redo works for UI-driven and MCP-driven mutations.
- Export uses actual GUI dimensions and produces deterministic files for Forge, Fabric, and NeoForge.
- Generated Java is fixture-compiled where fixtures are available, or covered by deterministic syntax/structure tests when full Minecraft dependencies are unavailable.
- MCP is exposed through the running app's web transport, follows the current Streamable HTTP lifecycle, and mutates live project sessions.
- Roadmap and README describe the real status.

## File Structure Plan

### Rust Backend

- Modify `src-tauri/src/project/mod.rs`
  - Add serde casing.
  - Add visibility, UV fields, and group model.
  - Introduce project session, session summary, active-session, snapshot, and undo/redo structs.

- Modify `src-tauri/src/commands.rs`
  - Add project tab/session commands.
  - Add save-as and missing element/animation/asset commands.
  - Add backend undo/redo commands.
  - Keep command return values typed and frontend-friendly.

- Modify `src-tauri/src/lib.rs`
  - Register all durable Tauri commands.
  - Start and stop the web MCP server as part of the normal app lifecycle.

- Modify `src-tauri/src/format/mod.rs`
  - Preserve path handling.
  - Add tests for `.mcgui` round-trips.

- Modify `src-tauri/src/mcp/mod.rs`
  - Replace accidental stdin MCP behavior with web MCP transport.
  - Fix initialization protocol.
  - Align tool schema with `docs/mcp.md` and ADR 003.
  - Add tests for JSON-RPC request/response handling and live session mutation.

- Modify `src-tauri/src/export/mod.rs`
  - Use real GUI dimensions.
  - Split Forge/Fabric/NeoForge code paths where types differ.
  - Remove placeholder claims.

- Modify `src-tauri/src/texture/mod.rs`
  - Decide and implement either composited final background or true packed atlas.
  - Fail loudly on missing referenced texture assets unless defaults are generated.

- Add Rust tests under existing modules or create:
  - `src-tauri/src/format/tests.rs`
  - `src-tauri/src/mcp/tests.rs`
  - `src-tauri/src/export/tests.rs`

### Frontend

- Modify `src/lib/api.ts`
  - Add typed wrappers for all Tauri commands.
  - Update mock backend to match the real API shape.
  - Remove direct `invoke` usage from components.

- Modify `src/lib/types.ts`
  - Match Rust serialized schema exactly.
  - Add sessions/tabs, `visible`, `uv`, groups, export config, command result types as needed.

- Modify `src/lib/stores/project.svelte.ts`
  - Represent tabbed project sessions and active session.
  - Route all durable mutations through `api.ts`.
  - Add a `revision`/`renderVersion` counter for reliable canvas rerendering.
  - Route undo/redo to backend commands.

- Modify `src/lib/stores/editor.svelte.ts`
  - Keep editor-only state here: selection, pan/zoom, preview scrubber, temporary drag state.

- Modify `src/lib/engine/renderer.ts`
  - Fix missing graphics child/return bugs.
  - Read element visibility and animation preview values.
  - Render actual texture sprites when available and deterministic placeholders when not.

- Modify `src/lib/components/Canvas.svelte`
  - Track `project.renderVersion`, GUI size, asset versions, and animation preview.

- Modify `src/lib/components/PropertyPanel.svelte`
  - Use backend-backed update command for all fields.

- Modify `src/lib/components/LayerPanel.svelte`
  - Persist z-order and visibility through project commands.

- Modify `src/lib/components/AnimationTimeline.svelte`
  - Persist animation create/update/delete/bind.
  - Store preview value in `editor` or a dedicated animation store that renderer can read.

- Modify `src/lib/components/AssetLibrary.svelte`
  - Persist pixel editor saves with `asset_update`.

- Modify `src/lib/components/PixelEditor.svelte`
  - Add zoom for larger textures.
  - Emit edited PNG data through backend update path.

- Modify `src/lib/components/ExportDialog.svelte`
  - Use `api.projectExport`.
  - Add preflight preview if Phase 5 preview remains in scope.

### Documentation

- Modify `docs/roadmap.md`
  - Replace optimistic status with verified status.

- Modify `README.md`
  - Match the chosen MCP mode and feature status.

- Modify `docs/mcp.md`
  - Match actual tool list and process mode.

- Add `docs/decisions/` or new ADRs if decisions above are changed materially.

## Task 1: Establish Shared Schema and Serialization Contract

**Files:**

- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/animation/mod.rs`
- Modify: `src/lib/types.ts`
- Test: Rust serde tests in `src-tauri/src/project/mod.rs` or `src-tauri/src/format/mod.rs`

- [ ] **Step 1: Write failing serde casing tests**

Add tests that serialize representative values and assert frontend-compatible casing:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_serializes_with_frontend_casing() {
        let element = Element {
            id: "slot_1".to_string(),
            element_type: ElementType::Slot,
            x: 8,
            y: 18,
            width: None,
            height: None,
            size: Some(18),
            asset: None,
            direction: Some(FillDirection::LeftToRight),
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: None,
        };

        let value = serde_json::to_value(&element).unwrap();
        assert_eq!(value["type"], "slot");
        assert_eq!(value["direction"], "left_to_right");
    }

    #[test]
    fn mod_target_serializes_with_frontend_casing() {
        assert_eq!(serde_json::to_value(ModTarget::Forge).unwrap(), "forge");
        assert_eq!(serde_json::to_value(ModTarget::Fabric).unwrap(), "fabric");
        assert_eq!(serde_json::to_value(ModTarget::NeoForge).unwrap(), "neoforge");
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `cargo test element_serializes_with_frontend_casing mod_target_serializes_with_frontend_casing`

Expected: tests fail because Rust currently serializes enum variants as `Slot`, `LeftToRight`, and `Forge`.

- [ ] **Step 3: Add serde casing attributes**

Implement:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModTarget {
    Forge,
    Fabric,
    NeoForge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    Texture,
    Slot,
    Progress,
    Text,
    FluidTank,
    EnergyBar,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FillDirection {
    LeftToRight,
    RightToLeft,
    BottomToTop,
    TopToBottom,
}
```

In `src-tauri/src/animation/mod.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnimationType {
    Fill,
    Cycle,
    Pulse,
    Toggle,
}
```

- [ ] **Step 4: Run schema tests**

Run: `cargo test`

Expected: serde tests pass.

- [ ] **Step 5: Confirm frontend types match**

Keep `src/lib/types.ts` casing as:

```ts
export type ElementType = "texture" | "slot" | "progress" | "text" | "fluid_tank" | "energy_bar";
export type FillDirection = "left_to_right" | "right_to_left" | "bottom_to_top" | "top_to_bottom";
export type ModTarget = "forge" | "fabric" | "neoforge";
```

## Task 2: Add Project Sessions/Tabs and Reliable Save Round-Trips

**Files:**

- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/format/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src/lib/components/Toolbar.svelte`
- Add/modify: tab UI component if the app does not already have one
- Test: Rust round-trip tests in `src-tauri/src/format/mod.rs`

- [ ] **Step 1: Add backend project-session manager**

Replace single `AppState.project: Mutex<Option<Project>>` with a Rust-owned session manager that tracks:

- `projects: Vec<ProjectSession>`
- `active_project_id: Option<String>`
- path, dirty flag, revision, and render-relevant metadata per session
- undo/redo stacks per session

`ProjectSession` should hold the durable `Project` plus session-only metadata. Commands target `project_id` when provided and fall back to the active session when omitted.

- [ ] **Step 2: Add tab/session commands**

Expose commands for:

- `project_new`
- `project_open`
- `project_close`
- `project_set_active`
- `project_list_sessions`
- `project_get_active`

Opening a second project creates another session/tab. Creating a new project does not discard other sessions.

- [ ] **Step 3: Add frontend tab model**

The Svelte store represents the backend session list and active session. The first viewport should show project tabs when more than one project is open, and switching tabs calls `project_set_active`.

- [ ] **Step 4: Add round-trip test for project path and archive contents**

Test must create a project with one texture asset, save to a temp path, reopen it, and assert:

- manifest exists and parses
- layout contains elements
- animations survive
- groups and UV regions survive
- texture bytes survive
- reopened `project_path` is set

Use `tempfile` if added, or write under `std::env::temp_dir()` with a UUID-based filename.

- [ ] **Step 5: Add backend save-as command**

Add command:

```rust
#[tauri::command]
pub fn project_save_as(
    state: State<AppState>,
    project_id: Option<String>,
    path: String,
) -> Result<serde_json::Value, String> {
    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve_mut(project_id.as_deref())?;
    session.project.project_path = Some(path.clone());
    format::save_to_mcgui(&session.project)?;
    session.project.is_dirty = false;

    Ok(serde_json::json!({
        "project_id": session.id,
        "status": "saved",
        "path": path,
        "is_dirty": false
    }))
}
```

Also update `project_save` to mark `is_dirty = false` after successful write.

- [ ] **Step 6: Register commands**

In `src-tauri/src/lib.rs`, add session and save commands to `generate_handler!`.

- [ ] **Step 7: Add frontend API wrapper**

In `src/lib/api.ts`:

```ts
export async function projectSaveAs(path: string, projectId?: string): Promise<{ project_id: string; status: string; path: string; is_dirty: boolean }> {
  const invoke = await getInvoke();
  return invoke("project_save_as", { path, projectId }) as Promise<{ project_id: string; status: string; path: string; is_dirty: boolean }>;
}
```

Update `mockInvoke` with the same command, session, and path semantics.

- [ ] **Step 8: Update store save methods**

`saveProject()` must prompt through caller only when path is missing. `saveProjectAs(path)` must call backend:

```ts
async saveProjectAs(path: string) {
  const result = await api.projectSaveAs(path);
  this.projectPath = result.path;
  this.isDirty = result.is_dirty;
  ProjectStore.addRecentProject(result.path);
  this.startAutoSave();
}
```

- [ ] **Step 9: Update toolbar save flow**

`handleSave()` should call `project.saveProjectAs(path)` when `project.projectPath` is null, and `project.saveProject()` otherwise.

- [ ] **Step 10: Verify**

Run:

- `cargo test`
- `pnpm exec svelte-check --tsconfig ./tsconfig.json`
- `pnpm build`

Expected: no save-related TypeScript errors and Rust round-trip tests pass.

## Task 3: Add Complete Tauri Mutation API and Remove Frontend-Only Durable Mutations

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src/lib/components/PropertyPanel.svelte`
- Modify: `src/lib/components/LayerPanel.svelte`
- Modify: `src/lib/components/AnimationTimeline.svelte`

- [ ] **Step 1: Add backend commands**

Add commands equivalent to MCP handlers:

- `element_get`
- `element_resize`
- `element_set_property`
- `element_duplicate`
- `element_reorder`
- `animation_create`
- `animation_update`
- `animation_remove`
- `animation_bind`
- `animation_unbind`
- `asset_update`

Each command must mutate `AppState.project`, set `is_dirty = true`, and return the updated entity or project summary.

- [ ] **Step 2: Register all commands**

Add them to `src-tauri/src/lib.rs`.

- [ ] **Step 3: Add typed frontend wrappers**

Do not use direct `invoke` in components. Add wrappers in `src/lib/api.ts` for every command.

- [ ] **Step 4: Route project store mutations through API**

Update:

- `updateElement` -> `api.elementSetProperty` or `api.elementUpdate`
- `resizeElement` -> `api.elementResize`
- `moveElementUp`/`moveElementDown` -> `api.elementReorder`
- `addAnimation`/`updateAnimation`/`removeAnimation` -> animation commands
- `bindAnimationToElement` -> animation bind/unbind command

- [ ] **Step 5: Add render revision**

In `ProjectStore`, add:

```ts
renderVersion = $state(0);

private touchRender() {
  this.renderVersion += 1;
}
```

Call `touchRender()` after any successful local mirror update.

- [ ] **Step 6: Verify no durable mutation remains frontend-only**

Search:

Run: `rg -n "Object.assign\\(|splice\\(|\\.animation =|\\.asset =|\\.width =|\\.height =" src/lib`

Expected: any remaining direct mutation is temporary editor state or inside a controlled post-command mirror update.

## Task 4: Fix Canvas Rendering and Reactive Updates

**Files:**

- Modify: `src/lib/engine/renderer.ts`
- Modify: `src/lib/components/Canvas.svelte`
- Modify: `src/lib/stores/editor.svelte.ts`
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Fix graphics containers**

Every method that creates `const container = new Container(); const g = new Graphics();` must add the graphics and return the container:

```ts
container.addChild(g);
return container;
```

Apply to:

- `drawSlot`
- `drawProgress`
- `drawFluidTank`
- `drawEnergyBar`

- [ ] **Step 2: Track render dependencies**

In `Canvas.svelte`, track:

```svelte
$effect(() => {
  void project.renderVersion;
  void project.guiSize.width;
  void project.guiSize.height;
  void editor.previewValue;
  if (renderer) renderer.render();
});
```

If `editor.previewValue` does not exist yet, add it in `editor.svelte.ts`.

- [ ] **Step 3: Use animation preview values**

Renderer should compute a preview ratio for progress-like elements:

```ts
private animationRatioFor(el: Element): number {
  if (!el.animation) return 1;
  return editor.previewValue;
}
```

Then draw progress/fluid/energy fill using that ratio.

- [ ] **Step 4: Add visibility support**

Element visibility is persistent. Add `visible?: boolean` to TypeScript and Rust. Renderer should skip `visible === false`.

- [ ] **Step 5: Verify**

Run:

- `pnpm exec svelte-check --tsconfig ./tsconfig.json`
- `pnpm build`

Expected: `drawSlot` no longer appears in TypeScript errors and canvas build succeeds.

## Task 5: Persist Pixel Editor Asset Updates

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/components/AssetLibrary.svelte`
- Modify: `src/lib/components/PixelEditor.svelte`

- [ ] **Step 1: Add `asset_update` backend command**

Command accepts `name` and `data_url` or base64 PNG bytes, validates PNG decode with `image::load_from_memory`, updates `project.texture_data`, keeps `project.assets`, and marks dirty.

- [ ] **Step 2: Add `api.assetUpdate`**

```ts
export async function assetUpdate(name: string, dataUrl: string): Promise<AssetImportResult> {
  const invoke = await getInvoke();
  return invoke("asset_update", { name, dataUrl }) as Promise<AssetImportResult>;
}
```

- [ ] **Step 3: Route PixelEditor save through backend**

`AssetLibrary` `onsaved` should call `api.assetUpdate`, update `assetDataUrls`, update `project.assets`, and touch render revision.

- [ ] **Step 4: Add pixel editor zoom**

Add local zoom state with 1x, 2x, 4x, 8x controls so 16x16 and 256x256 images are both usable.

- [ ] **Step 5: Verify save/reopen**

Manual workflow:

1. Import PNG.
2. Edit a pixel.
3. Save project.
4. Reopen project.
5. Confirm edited pixel remains.

## Task 6: Implement Backend Undo/Redo

**Files:**

- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `docs/architecture.md`

- [ ] **Step 1: Add backend history model**

Each project session owns an undo stack and redo stack. Use full project snapshots first unless tests prove the memory cost is unacceptable; this is lower risk than attempting inverse operations while the schema is still changing.

- [ ] **Step 2: Record history around durable mutations**

Every backend command that changes project data records the previous project snapshot, clears redo, updates dirty/revision metadata, and returns the updated session/project summary.

- [ ] **Step 3: Add undo/redo commands**

Expose:

- `project_undo(project_id?: string)`
- `project_redo(project_id?: string)`

Commands target an explicit session when provided; otherwise they target the active tab.

- [ ] **Step 4: Replace frontend history**

Remove frontend-only undo stacks. Toolbar handlers call backend undo/redo and refresh the active session mirror.

- [ ] **Step 5: Add tests**

Rust tests should cover add/update/remove, undo, redo, redo clearing after a new mutation, dirty flag behavior, and session isolation.

## Task 7: Replace stdio MCP with live web MCP

**Files:**

- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `docs/mcp.md`
- Modify: `README.md`
- Test: `src-tauri/src/mcp/mod.rs`

- [x] **Step 1: Remove stdio startup from normal GUI**

Normal `run()` must not spawn a stdin reader or write MCP protocol messages to stdout.

- [x] **Step 2: Add web MCP server lifecycle**

Start a local HTTP listener from the running app instance and keep its handle in `AppState`. Use Streamable HTTP semantics rather than the deprecated HTTP+SSE-only shape. Bind to localhost by default and expose the selected port through a Tauri command and UI/documentation.

- [x] **Step 3: Share live project sessions**

MCP handlers must use the same Rust project-session manager as Tauri commands. Tool calls mutate the active tab by default and accept `project_id` where ambiguity matters.

- [x] **Step 4: Fix initialization lifecycle**

Do not send `notifications/initialized` on startup. Wait for `initialize`, respond with capabilities, then accept client `notifications/initialized`.

- [x] **Step 5: Validate JSON-RPC version**

Reject requests where `jsonrpc != "2.0"` with `-32600 Invalid Request`.

- [x] **Step 6: Align advertised tools**

Make `docs/mcp.md`, `get_tool_definitions()`, and `execute_tool()` match exactly. Include or remove:

- `project_export`
- `asset_import`
- group tools
- UV update tools
- undo/redo tools
- `resources/read`

- [x] **Step 7: Add MCP tests**

Add tests for:

- initialize response
- tools/list contains expected tools
- tools/call unknown tool returns error
- no unsolicited initialized notification
- element mutation changes a live project session
- undo/redo through MCP changes the same backend state visible to Tauri commands

## Task 8: Repair Export Pipeline to Complete Usable Output

**Files:**

- Modify: `src-tauri/src/export/mod.rs`
- Modify: `src-tauri/src/texture/mod.rs`
- Modify: `src/lib/components/ExportDialog.svelte`
- Modify: `src/lib/api.ts`
- Test: `src-tauri/src/export/mod.rs`

- [x] **Step 1: Add export config dimensions**

Either pass `project` to codegen functions or add dimensions to `ExportConfig`:

```rust
pub struct ExportConfig {
    pub mod_id: String,
    pub package: String,
    pub class_name: String,
    pub output_dir: String,
    pub gui_width: u32,
    pub gui_height: u32,
}
```

- [x] **Step 2: Use real dimensions**

Generated Forge:

```java
this.imageWidth = 176;
this.imageHeight = 166;
```

Generated Fabric:

```java
this.backgroundWidth = 176;
this.backgroundHeight = 166;
```

Values come from `project.gui_size`.

- [x] **Step 3: Split loader-specific runtime code**

Do not generate a Forge-importing `GuiLayout.java` for Fabric or NeoForge. Either:

- generate `ForgeGuiLayout.java`, `FabricGuiLayout.java`, and `NeoForgeGuiLayout.java`, or
- generate a loader-neutral layout data parser plus loader-specific renderer adapter.

- [x] **Step 4: Remove false completion placeholders**

Animation generation should either:

- generate calls for known `project.animations`, or
- produce a clearly named hook section and document "manual data binding required".

Do not claim fully automated animation binding if it still needs app-specific game-state code. Any generated hook must compile before the user fills in custom behavior.

- [x] **Step 5: Fix texture compositor contract**

Recommended Phase 5 behavior:

- Create one GUI background PNG at `project.gui_size`.
- Composite texture elements at their GUI coordinates.
- Leave slots/text/progress as runtime-rendered elements in `layout.json`.
- Error if a texture element references a missing asset, except built-in template defaults that are generated.

- [ ] **Step 6: Add export preview**

Before writing files, `ExportDialog` should show the planned file tree based on settings. The actual export button then writes.

- [x] **Step 7: Add export tests**

Rust tests should assert:

- generated files list contains texture, layout JSON, Java, README
- generated Java contains actual dimensions
- Fabric export does not contain Forge-only imports
- NeoForge export has loader-specific imports and does not reuse Forge-only classes where NeoForge differs
- generated hook points compile in fixture or syntax tests

## Task 9: Implement Missing Roadmap Features

**Files:**

- Modify: `docs/roadmap.md`
- Modify: `src/lib/types.ts`
- Modify: `src-tauri/src/project/mod.rs`
- Optional Modify: `src/lib/components/LayerPanel.svelte`
- Optional Modify: `src/lib/components/PropertyPanel.svelte`

- [x] **Step 1: Groups**

- Add group create/ungroup commands.
- Add layer panel group actions for current multi-selection.
- Add group serialization round-trip test.

- [x] **Step 2: UV selector**

- Add `uv?: { x: number; y: number; width: number; height: number }` to element schema.
- Add property panel fields.
- Render sprite subregions in Pixi.
- Export UV data.

- Mark Phase 3 as incomplete and move UV selector to Phase 6.

- [ ] **Step 3: Recent projects**

Add UI for `ProjectStore.getRecentProjects()` or remove roadmap completion claim.

## Task 10: Make Verification First-Class

**Files:**

- Modify: `package.json`
- Add or modify Rust tests in `src-tauri/src/**`
- Optional add frontend unit test tooling only if project owner accepts dependency additions

- [x] **Step 1: Add check scripts**

In `package.json`:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "verify": "pnpm check && pnpm build"
  }
}
```

- [x] **Step 2: Add Rust test coverage**

Minimum Rust tests:

- serde casing
- `.mcgui` save/load round trip
- web MCP initialize/tools list and live mutation behavior
- export generated dimension strings
- texture compositor missing asset behavior

- [x] **Step 3: Run full verification**

Run:

- `pnpm verify`
- `cargo test`
- `cargo build`

Expected:

- no TypeScript/Svelte errors
- Rust tests pass with non-zero test count
- no critical warnings left in touched modules

## Task 11: Update Documentation to Match Reality

**Files:**

- Modify: `docs/roadmap.md`
- Modify: `docs/architecture.md`
- Modify: `docs/mcp.md`
- Modify: `README.md`
- Optional Add: `docs/adr/006-mcp-process-model.md`
- Optional Add: `docs/adr/007-project-state-authority.md`

- [x] **Step 1: Update roadmap status**

Use statuses that reflect verified behavior:

```markdown
| Phase | Status |
|-------|--------|
| 1: Foundation | Remediation in progress |
| 2: Editing & Templates | Remediation in progress |
| 3: Texture Tools | In progress |
| 4: MCP Server | Remediation in progress |
| 5: Export Pipeline | In progress |
| 6: Polish & Advanced | Not started |
| 7: Community & Distribution | Not started |
```

- [x] **Step 2: Update README feature language**

Keep "planned" versus "implemented" explicit. NeoForge is in this remediation scope, so docs must not describe it as Phase 6-only once implementation lands.

- [x] **Step 3: Update architecture state ownership**

Document the actual chosen source of truth and undo/redo strategy.

- [x] **Step 4: Update MCP docs**

Document the running-app web MCP endpoint, local binding behavior, session targeting semantics, and active-tab fallback. Do not document a separate `--mcp` process unless it is explicitly reintroduced later for a different workflow.

Docs must state that live GUI sync is supported through the shared Rust project-session manager.

## Execution Order

1. Task 1: schema contract.
2. Task 2: project-session/tab foundation plus save/save-as and `.mcgui` round-trip.
3. Task 3: complete mutation API and eliminate state divergence.
4. Task 6: backend undo/redo authority.
5. Task 7: live web MCP protocol and session targeting.
6. Task 4: canvas rendering and rerender correctness.
7. Task 5: asset update persistence.
8. Task 9: groups/UV implementation.
9. Task 8: Forge/Fabric/NeoForge export correctness.
10. Task 10: verification.
11. Task 11: docs.

## Remaining Implementation Questions

- Pick an initial local web MCP port strategy: fixed default with retry, or OS-assigned port surfaced through UI/API.
- Decide whether MCP session selection should be sticky per MCP client session or always derive from the current active tab when `project_id` is omitted.
- Define the minimum fixture strategy for generated Java compilation without pulling full Minecraft dependencies during normal tests.

## Self-Review

- Spec coverage: all high-confidence findings from Codex, DeepSeek, and Gemini are covered by tasks or remaining implementation questions.
- Placeholder scan: product decisions from Pavel are resolved explicitly; remaining questions are implementation-shaping details.
- Type consistency: Rust serde casing, TypeScript unions, and command names are aligned in the plan.
