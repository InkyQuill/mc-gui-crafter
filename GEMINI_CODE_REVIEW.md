# GEMINI Code Review — MCGUI Crafter

**Date:** Sunday, May 17, 2026
**Scope:** Phases 1–5 Verification

## Executive Summary
The project is in a highly advanced state. Contrary to the "Not started" status in `docs/roadmap.md`, Phases 1–5 are largely **implemented and functional**. The architecture is clean, using Svelte 5 runes for state management and a robust Rust backend for I/O and MCP services.

However, some critical rendering bugs and minor stubs in the export pipeline were identified that need immediate attention before a "Phase 5 Complete" sign-off.

---

## Phase Audit Results

### Phase 1: Foundation (v0.1) — **95% Complete**
- [x] Tauri 2 + Svelte 5 + Vite project scaffold.
- [x] `.mcgui` format read/write (Rust backend + Zip archive).
- [x] Canvas renderer: zoom/pan, grid, element rendering.
- [x] Element palette: placement of slots, textures, text.
- [x] Property panel: coordinate editing.
- [x] Basic toolbar: project I/O.
- [x] Project store with full undo/redo.
- [!] **CRITICAL BUG:** `src/lib/engine/renderer.ts`: `drawSlot` method is missing `container.addChild(g)` and `return container`, causing slots to be invisible on canvas.

### Phase 2: Editing & Templates (v0.2) — **90% Complete**
- [x] Selection, drag-to-move, and resize handles.
- [x] Layer panel (functional but basic).
- [x] Template system (Furnace, Crafting, Chests).
- [x] Animation timeline: preview and scrubber.
- [x] Auto-save (atomic rename with `.tmp`).
- [x] Recent projects list (localStorage).
- [ ] **MISSING:** Multi-select (the store supports it, but the renderer/selection logic needs refinement for multi-drag).

### Phase 3: Texture Tools (v0.3) — **80% Complete**
- [x] Texture import.
- [x] Asset library (sidebar).
- [x] Pixel art editor (Pencil, Eraser, Eyedropper, Fill).
- [x] Color palette management (Minecraft presets).
- [x] Texture replacement on elements.
- [ ] **STUBBED:** UV region selector for sprite sheets is missing; textures are treated as full images.

### Phase 4: MCP Server (v0.4) — **100% Complete**
- [x] MCP server core (JSON-RPC over stdio).
- [x] Full tool implementation (30+ tools for every aspect of the app).
- [x] Shared project state between UI and MCP.
- [x] Real-time sync: UI emits and reacts to MCP changes via events.

### Phase 5: Export Pipeline (v0.5) — **75% Complete**
- [x] Texture atlas compositor (composed PNG).
- [x] Layout JSON exporter.
- [x] `GuiLayout` runtime library (Java).
- [x] Forge/Fabric Screen class codegen.
- [!] **STUBBED:** `generate_animation_calls` in `export/mod.rs` only generates a placeholder comment. Actual binding of animations to Menu data is not yet automated.
- [!] **STUBBED:** `GuiLayout.java` rendering logic for `text` and `slot` is currently empty.

---

## Detailed Findings

### 1. Rendering Engine (`src/lib/engine/renderer.ts`)
The `drawSlot` method is broken. It prepares the Graphics object but never adds it to the container or returns it.
```typescript
// Current implementation
private drawSlot(el: Element): Container {
  const container = new Container();
  const g = new Graphics();
  // ... drawing calls ...
  // MISSING: container.addChild(g);
  // MISSING: return container;
}
```

### 2. Export Pipeline (`src-tauri/src/export/mod.rs`)
The Java codegen is impressive but incomplete. The `GuiLayout.java` class is a great start for a runtime library, but the `renderBg` method needs to handle `slot` rendering (at least as a debug overlay) and `text` rendering using Minecraft's FontRenderer.

### 3. MCP Server (`src-tauri/src/mcp/mod.rs`)
This is the strongest part of the backend. It's fully featured and provides a deep integration for AI tools. One minor improvement would be to add a "Read Resource" tool to allow AIs to read the actual `.mcgui` file content for context.

### 4. Roadmap Status
The `docs/roadmap.md` file is severely outdated and shows "Not started" for all phases. This should be updated immediately to reflect the actual progress.

---

## Recommendations

1.  **Fix Slot Rendering:** Immediately patch `renderer.ts` to restore slot visibility.
2.  **Flesh out Export Logic:** Implement the animation binding logic in `export/mod.rs` so the exported screens are more than just skeletons.
3.  **Update Roadmap:** Sync `roadmap.md` with current reality.
4.  **Sprite Sheet Support:** Implement the UV selector to allow using segments of a single texture atlas (common in MC modding).
5.  **Multi-select Polish:** Improve the UI feedback when multiple elements are selected and ensure drag-and-drop works reliably for the entire selection group.

**Overall Status:** **STABLE PROTOTYPE**. Ready for Phase 6 (Polish) after the identified bugs and stubs are addressed.
