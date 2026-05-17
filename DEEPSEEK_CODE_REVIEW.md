# MCGUI Crafter — Deep Code Review

**Review date:** 2026-05-17
**Reviewer:** DeepSeek (via pi agent harness)
**Scope:** Phases 1–5 (Foundation, Editing & Templates, Texture Tools, MCP Server, Export Pipeline)
**Build status:** ✅ Rust compiles (5 minor warnings), ✅ Vite builds (1.73s, 369KB JS bundle)

---

## Executive Summary

The project has a solid architecture with substantial working code across all five phases. The Rust backend compiles cleanly and the Svelte 5 frontend builds without errors. However, there are **critical gaps** between the architectural documentation and what's actually implemented, particularly around undo/redo, canvas rendering fidelity, and the save workflow. Below is a detailed per-phase audit with severity ratings.

---

## Phase 1: Foundation — 75% Complete

### ✅ What's Done

| Item | Status | Notes |
|------|--------|-------|
| Tauri 2 + Svelte 5 + Vite scaffold | ✅ | Compiles and builds. Modern Svelte 5 mount() API used correctly. |
| `.mcgui` format (Rust) | ✅ | Full zip read/write with manifest.json, layout.json, animations.json, textures/. Atomic rename via `.tmp`. |
| Canvas renderer (PixiJS v8) | ✅ | Working GridOverlay (major 18px, minor 2px), background fill, element rendering. |
| Element palette | ✅ | Slot, texture, text with keyboard shortcuts (V/H/S/T/X). |
| Property panel | ✅ | X/Y editing, type-specific props. |
| Toolbar: New, Open, Save | ⚠️ | **Save flow broken** — see issue #1 below. |
| Coordinate readout | ✅ | StatusBar + canvas cursorLabel. |
| Undo/redo | ⚠️ | Implemented but **frontend-only** — see issue #2 below. |

### 🔴 Critical Issues

**#1: Save fails after New Project (no Save As flow)**
- `project.newProject()` sets `this.projectPath = null`.
- `project.saveProject()` checks `if (this.projectPath)` → returns early, never saves.
- The toolbar Save button silently does nothing. User must manually use Save As (which isn't a toolbar option).
- **Fix:** Add a "Save As" button to toolbar, or auto-prompt when `projectPath` is null.

**#2: Undo/redo is not command-based (architectural gap)**
- Architecture doc specifies a Rust-side `enum Command` with `apply()`/`undo()` methods.
- Actual implementation: frontend-only undo stack with closures stored in `UndoEntry[]`.
- Elements modified via MCP won't be undoable.
- Element props edited in PropertyPanel have undo entries, but they use `Object.assign` shallow copy — nested objects would leak references.
- **Severity:** Medium. Works for single-user editing but breaks the architecture's design.

### 🟡 Minor Issues

- Zoom defaults to 2× (architecture says 1). Cosmetic.
- `EditorStore.zoom` has no max clamp on the renderer side; zoom=8 means 8× which is fine but handle hit tests don't scale with zoom.
- Canvas `ResizeObserver` + `$effect` render dependencies may cause double-renders on resize.
- The `element_add` Tauri command deserializes the full `Element` struct including server-assigned fields — potential for ID collision if frontend sends duplicates. The backend trusts the frontend for IDs.

---

## Phase 2: Editing & Templates — 65% Complete

### ✅ What's Done

| Item | Status | Notes |
|------|--------|-------|
| Selection + drag-to-move | ✅ | Hit testing works in reverse z-order. Multi-drag for multi-selection. |
| Resize handles | ✅ | 4-corner resize with corner-specific logic in renderer. |
| Multi-select (Shift+click) | ✅ | Shift-key additive selection in pointerdown handler. |
| Layer panel | ✅ | Element tree with z-order, visibility toggle (hiddenElements), up/down reordering. |
| Templates | ✅ | empty, furnace, crafting_3x3, chest_9x3, chest_9x6. |
| Template picker dialog | ✅ | Modal dialog on New Project. |
| Animation timeline UI | ✅ | Add/edit/remove animations, bind to elements, playback scrubber. |
| Data key binding | ✅ | `data_key` field on animations, editable in timeline. |
| Full undo/redo stack | ✅ | Working push/pop with description labels. |

### 🔴 Critical Issues

**#3: Canvas does not render animation previews**
- The `AnimationTimeline` has a scrubber (`previewValue` 0–100%) that updates reactively, but **no one reads this value in the renderer**.
- `GuiRenderer.render()` draws elements statically — progress bars always show at 0%, fluid tanks always at 50%, energy bars always at 40%. These are hardcoded preview values.
- The architecture diagram shows "Animated Elements → renders progress bars, fluid tanks, etc." in the rendering pipeline, but the renderer has no connection to animation state.
- **Fix:** Pass `previewValue` from AnimationTimeline to the renderer; modify `drawProgress`, `drawFluidTank`, `drawEnergyBar` to use it.

**#4: Element groups are not creatable**
- Architecture doc describes `Group` with `id`, `x`, `y`, `elements: String[]`.
- The `Group` type exists in `types.ts` and Rust `project/mod.rs`.
- `layouts.json` serialization includes groups.
- But there's **no UI to create groups** — no "Group" button, no multi-select → group action, no canvas grouping.
- `project.groups` is always empty after template apply.
- **Fix:** Add a "Group" action and group rendering in the canvas.

### 🟡 Minor Issues

- Auto-save runs every 60s but there's **no `.mcgui.tmp` atomic rename on the frontend side** — the Rust backend does it, but the frontend just calls `saveProject()` which invokes `project_save`. This is actually fine since `save_to_mcgui` uses `.tmp` internally.
- Recent projects are stored in `localStorage` but **there's no UI** to display them — no "Recent Projects" list in toolbar or New Project dialog.
- The `moveElementUp`/`moveElementDown` methods directly mutate the `elements` array with `splice` — this works but won't be re-doable (no undo entry pushed). Layer panel reordering bypasses undo/redo.
- `hiddenElements` in LayerPanel is a `$state(Set)` but elements are **still rendered on canvas** — the visibility toggle only sets a CSS class in the layer list. The renderer ignores hidden state.

---

## Phase 3: Texture Tools — 55% Complete

### ✅ What's Done

| Item | Status | Notes |
|------|--------|-------|
| Texture import (PNG) | ✅ | Tauri dialog → Rust `asset_import` command. Browser fallback uses FileReader. |
| Asset library panel | ✅ | Grid display with thumbnails, edit/remove buttons. |
| Pixel art editor | ✅ | Pencil, eraser, eyedropper, flood fill. Canvas 2D API. |
| Minecraft color palette | ⚠️ | 22 hardcoded colors. Limited but functional. |

### 🔴 Critical Issues

**#5: Canvas renderer doesn't use imported textures for rendering**
- `drawTexture()` checks `assetDataUrls.get(el.asset)` — if found, renders a PixiJS `Sprite`.
- But this only works if the texture was loaded through the Svelte component's `assetDataUrls` map.
- On project open, `project.openProject()` loads assets into `assetDataUrls`. ✅
- On MCP `element_add` with an asset, the renderer won't have the data URL unless the element was placed from the UI. ❌
- Also, the `drawSlot` method creates a `Graphics` object but **never adds it to the container** — it creates the container, draws graphics, but there's no `container.addChild(g)`. This means **slots render as empty containers**.
- **Fix:** `drawSlot` needs `container.addChild(g)`. Same pattern is used for `drawProgress` (also missing `container.addChild(g)`).

**#6: Texture replacement/swap is incomplete**
- Property panel has an asset dropdown, but selecting a different asset only changes the `asset` field — the old asset data URL remains bound to that element's canvas rendering. The renderer uses `assetDataUrls.get(el.asset)` which will correctly pick up the new asset name, so this actually does work... as long as the new asset was imported previously. ✅
- But there's no visual feedback that the swap happened (no re-render trigger). The `$effect` in Canvas.svelte only tracks `elements.length`, not element property changes. **Properties changed in PropertyPanel won't trigger a re-render.**
- **Fix:** Canvas `$effect` needs to track element deep changes, or use a render-tick counter.

### 🟡 Minor Issues

- **No UV region selector for sprite sheets.** The architecture specifies this as a Phase 3 deliverable. Not started.
- Pixel editor saves to `canvas.toDataURL()` but on the Rust side, there's no `asset_update` command to persist edited pixel data — changes are only in-memory until project save.
- `assetImport` in Rust decodes the image to get dimensions, then re-encodes via base64 for the frontend. This double-decodes images. For large textures this is wasteful but functionally correct.
- The pixel editor always uses the `<canvas>` element's natural size for the full image — if you edit a 16×16 texture, the canvas stays 16×16 which is good, but larger textures (256×256) would have a massive canvas. No zoom control in the pixel editor.

---

## Phase 4: MCP Server — 80% Complete

### ✅ What's Done

| Item | Status | Notes |
|------|--------|-------|
| JSON-RPC 2.0 over stdio | ✅ | Line-delimited protocol, properly parsed. |
| All 24 tool implementations | ✅ | project (4), element (9), animation (5), asset (2), gui (3). |
| Shared state via Tauri AppState | ✅ | Single `Mutex<Option<Project>>` shared between commands and MCP. |
| Real-time sync (`project-changed`) | ✅ | Event emitted after mutating tools, listened in App.svelte. |
| Tool schemas for AI providers | ✅ | Complete `get_tool_definitions()` with properties/required. |
| Template resources | ✅ | Resources exposed at `template://{name}`. |

### 🔴 Critical Issues

**#7: MCP protocol non-conformance**
- The server sends a `notifications/initialized` message on startup using JSON-RPC 2.0 `method` field. The MCP protocol uses the **`method` field for the actual MCP method** (e.g., `initialize`, `tools/list`), not for notification types.
- Additionally, the server sends this notification **without waiting for client `initialize` request**. The MCP spec requires client-initiated initialization with capability negotiation.
- **Fix:** Remove the auto-sent notification. Wait for client to send `initialize`, then respond with server capabilities. Only after `initialized` notification from client should the server consider the session ready.

**#8: Single project state bottleneck**
- `AppState.project` is `Mutex<Option<Project>>` — only one project at a time.
- The Tauri `State` API only provides borrowed access: `state.project.lock().unwrap()`.
- During an MCP tool call, the mutex is locked, blocking the UI thread from reading project data (e.g., canvas re-render triggered by `project-changed` event causing `syncFromBackend()` which calls `elementList` command). **This creates a deadlock risk** since both MCP and commands use the same `Mutex`.
- **Fix:** Use `tokio::sync::RwLock` instead of `std::sync::Mutex`, or separate the read path from the write path.

### 🟡 Minor Issues

- **No HTTP transport option** (listed as Phase 4 deliverable). Not implemented.
- `project_summary` tool tries to serialize the entire `Project` including `texture_data` (which is `#[serde(skip)]` so it's fine) but the serialization may be slow for large projects.
- The `project-changed` event emits for EVERY mutating tool call, even if the tool failed or was a no-op. This causes unnecessary `syncFromBackend()` calls.
- The `JsonRpcRequest.jsonrpc` field is never validated (it always says `jsonrpc: "2.0"` but is parsed and ignored).

---

## Phase 5: Export Pipeline — 60% Complete

### ✅ What's Done

| Item | Status | Notes |
|------|--------|-------|
| Texture atlas compositing | ✅ | `composite_atlas()` composes all texture elements into a single PNG. |
| Layout JSON exporter | ✅ | Writes `gui_size`, `elements`, `animations` to JSON. |
| GuiLayout.java runtime | ⚠️ | Generated but with hardcoded dimensions. |
| Forge Screen codegen | ⚠️ | Generated but with hardcoded 256×256. |
| Fabric Screen codegen | ⚠️ | Generated but with hardcoded 256×256. |
| Export dialog | ✅ | Full form with mod loader, package, class name, output dir. |
| README generation | ✅ | Includes integration steps. |

### 🔴 Critical Issues

**#9: Generated Screen classes hardcode dimensions to 256×256**
```java
// In generate_forge_screen() and generate_fabric_screen():
this.imageWidth = 256;
this.imageHeight = 256;
```
This ignores the project's actual `gui_size`. A 176×166 furnace GUI would get a 256×256 screen, breaking mouse coordinate mapping and background rendering.
- **Fix:** Pass `project.gui_size` through `ExportConfig` and use it in the generated code.

**#10: NeoForge export silently falls through to Forge**
- The `export_project` function:
```rust
let screen_code = match target {
    "fabric" => generate_fabric_screen(config),
    _ => generate_forge_screen(config),  // neoForge also gets Forge
};
```
- NeoForge uses different class names in recent versions (`GuiGraphics` vs `PoseStack`). The Forge template may not compile under NeoForge without modifications.
- **Fix:** Add a `generate_neoforge_screen()` or at minimum document the limitation.

**#11: GuiLayout.java is not templated properly**
- `generate_gui_layout_java()` returns a raw string with `{PACKAGE}` substitution, but other placeholders (`{width}`, `{height}`, `{cls}`) are never replaced.
- The `getWidth()` and `getHeight()` methods use Java streams but receive `elems` as parameter instead of using `this.elements`.
- The `renderBg` method skips texture elements by checking `el.type.equals("texture")`, but the JSON field name is `type` (not a concern for JSON parsing but confusing).
- `renderProgress` assumes `anim.direction` is a direct string like `"left_to_right"`, but the JSON has it nested.
- **Fix:** Properly parameterize the template or use a proper Java code generation library (e.g., JavaPoet for Rust doesn't exist, but string templating can be improved).

### 🟡 Minor Issues

- **Export preview is shown after export**, not before (architecture says "before writing"). The dialog shows generated files as a post-export result list instead of a pre-flight preview.
- **`composite_atlas` uses nearest-neighbor resizing** for texture elements that have different `width`/`height` than the source PNG. This is correct for pixel art but should be documented.
- The `composite_atlas` error message has a typo: `"Failed to encode PNV: {e}"` — should be `PNG`.
- Export doesn't generate a `Container`/`Menu`/`ScreenHandler` class — only the Screen class. The README says to manually create it.
- No export for Fabric's `ScreenHandler` or Forge's `Menu` — the user must write container logic by hand.

---

## Cross-Cutting Concerns

### 🔴 Architecture Gaps

**#12: No Rust-side command registration for many operations**
The `invoke_handler` in `lib.rs` only registers these commands:
```rust
commands::project_new, project_open, project_save, project_summary,
template_list, asset_import, asset_list, asset_remove, asset_get_data_url,
project_export, element_add, element_move, element_remove, element_list,
```
Missing from Tauri commands (only accessible via MCP):
- `element_resize`
- `element_set_property`
- `element_duplicate`
- `element_get`
- Any animation commands
- `project_export` (exists as command but frontend's ExportDialog imports `@tauri-apps/api/core` directly for `invoke`)

This means the **PropertyPanel and ExportDialog use direct IPC rather than the api.ts abstraction layer**, and several frontend operations (duplicate, resize through handles) only modify local state without syncing to the backend.

**#13: Frontend <-> Backend state divergence**
The frontend maintains its own copy of `elements[]` in `project.elements`. When the MCP modifies elements, the `project-changed` event triggers `syncFromBackend()` which overwrites local state. But during drag/resize operations, the frontend mutates local state live and only persists to backend on `pointerup`. If an MCP change arrives mid-drag, state is lost.

### 🟡 Code Quality Issues

**#14: Slot rendering is broken**
```typescript
// renderer.ts - drawSlot()
private drawSlot(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    // ... draws on g ...
    // MISSING: container.addChild(g);
}
```
The `drawSlot`, `drawProgress`, `drawFluidTank`, and `drawEnergyBar` methods all create `Graphics` objects but never add them to the returned `Container`. These elements render as invisible containers with no visual output. Only `drawTexture` and `drawText` work correctly.

**#15: Canvas reactive $effect only tracks element count**
```svelte
// Canvas.svelte
$effect(() => {
    void project.elements.length;  // only tracks length changes
    void project.guiSize.width;
    void project.guiSize.height;
    if (renderer) renderer.render();
});
```
If you change an element's `x`, `y`, `content`, or `color` in the PropertyPanel, the canvas won't re-render until you add/remove an element or change the GUI size. Property edits that don't use the drag/resize system leave the canvas stale.

**Fix:** Use a render counter tick that increments on any element mutation, or use Svelte 5 `$inspect`/`$state` deep tracking.

**#16: Mock backend (api.ts) is incomplete**
The browser mock for development lacks:
- `assetImport`, `assetList`, `assetRemove`, `assetGetDataUrl`
- `project_export`
- All animation commands
- Actually applying templates
This means the app can't be fully tested in `vite dev` mode without Tauri.

### 🟢 Minor/Nitpicks

- Several Rust `use` imports are unused (`Rgba` in texture/mod.rs, `jsonrpc` field).
- `ExportConfig` parameter is unused in animation call generators.
- `AssetInfo` struct in `project/mod.rs` is defined but never used (API returns `serde_json::Value` instead).
- MCP server has no graceful shutdown — there's no signal handling. When the app closes, the stdio reader loop breaks on pipe close, which is acceptable but may leave logs.
- No tests anywhere — zero `#[test]` functions in Rust, zero unit tests in TypeScript.
- `vite-env.d.ts` exists but is empty — should reference `@tauri-apps/api` or Svelte types.

---

## Phase Completion Summary

| Phase | Weight | % Complete | Key Gaps |
|-------|--------|------------|----------|
| 1: Foundation | 20% | **75%** | Save flow broken, undo/redo not command-based |
| 2: Editing & Templates | 25% | **65%** | No animation preview on canvas, groups not creatable, visibility toggle ignored by renderer |
| 3: Texture Tools | 15% | **55%** | Slot/progress rendering broken, no UV selector, no pixel data persistence |
| 4: MCP Server | 20% | **80%** | Protocol non-conformance, potential deadlock, no HTTP transport |
| 5: Export Pipeline | 20% | **60%** | Hardcoded dimensions, NeoForge not distinct, GuiLayout.java bugs |
| **Overall** | 100% | **~67%** | |

---

## Priority Fixes (What to address first)

1. **[P0] Fix `drawSlot`, `drawProgress`, `drawFluidTank`, `drawEnergyBar`** — add `container.addChild(g)` in renderer. Currently invisible.
2. **[P0] Fix Canvas reactive re-render** — track element property changes, not just array length.
3. **[P0] Fix Save after New Project** — either set projectPath on save dialog, or add Save As button.
4. **[P1] Fix MCP protocol initialization** — wait for client `initialize` request, send proper response.
5. **[P1] Fix MCP/Tauri mutex deadlock risk** — use `RwLock` or separate read command from write.
6. **[P1] Wire animation preview into canvas renderer** — pass scrubber value to drawProgress/drawFluidTank/drawEnergyBar.
7. **[P2] Fix export hardcoded 256×256 dimensions** — use actual gui_size.
8. **[P2] Register missing Tauri commands** — element_resize, element_set_property, element_duplicate, animation commands.
9. **[P2] Add groups UI** — create/manage element groups.
10. **[P3] Add NeoForge distinct export target** — don't fall through to Forge.
11. **[P3] Add UV region selector for sprite sheets** — Phase 3 deliverable.
12. **[P3] Add tests** — at minimum a few integration tests for the Tauri commands.
