# MCGUI Crafter — Roadmap

## Phase 1: Foundation (v0.1)

**Goal:** Functional scaffold with project I/O and basic canvas.

- [x] Tauri 2 + Svelte 5 + Vite project scaffold
- [x] `.mcgui` format read/write (Rust backend)
- [x] Canvas renderer: background texture, grid overlay, zoom/pan
- [x] Element palette: slot, texture, text (drag to place)
- [x] Property panel: coordinate editing for selected element
- [x] Basic toolbar: new, open, save
- [x] Coordinate readout on cursor hover
- [x] Project store with undo/redo

**Deliverable:** Can create a new project, place slots and textures, save and reopen.

---

## Phase 2: Editing & Templates (v0.2)

**Goal:** Productive editing with templates and the animation system.

- [x] Selection, drag-to-move, and resize handles on canvas
- [x] Multi-select and group elements
- [x] Layer panel (element tree, z-order, visibility toggle)
- [x] Template system: empty, furnace, 3×3 crafting, chest (9×3), chest (9×6)
- [x] Template picker dialog on "New Project"
- [x] Animation timeline: create/edit fill/progress animations
- [x] Animation preview on canvas (play with scrubber)
- [x] Data key binding for animations (`cook_progress`, `burn_time`, etc.)
- [x] Full undo/redo stack
- [x] Auto-save with `.mcgui.tmp` atomic rename
- [x] Recent projects list

**Deliverable:** Can scaffold from templates, position elements precisely, define animations.

---

## Phase 3: Texture Tools (v0.3)

**Goal:** Built-in texture import and pixel-art editing.

- [x] Texture import dialog (PNG, drag-and-drop from file system)
- [x] Asset library panel (browse imported textures, preview)
- [x] Pixel art editor (simple): pencil, eraser, color picker, fill
- [x] Color palette management (Minecraft color palette presets)
- [x] Texture replacement (swap asset on element)
- [x] UV region selector for sprite sheets
- [x] Canvas renderer uses actual textures

**Deliverable:** Can create or import textures and place them on the GUI without external tools.

---

## Phase 4: MCP Server (v0.4)

**Goal:** AI tools can create and edit GUI projects programmatically.

- [x] MCP server core (JSON-RPC 2.0 over Streamable HTTP-style localhost endpoint)
- [x] Core tool implementations (see ADR 003)
- [x] Shared project state between editor UI and MCP server
- [x] Real-time sync: MCP changes reflected in editor canvas
- [x] Tool schema documentation
- [x] HTTP transport option
- [x] Example JSON-RPC workflow in docs

**Deliverable:** AI tools can connect to the running app's web MCP endpoint and spawn/modify GUI projects.

---

## Phase 5: Export Pipeline (v0.5)

**Goal:** Produce moddable output for Minecraft mod loaders.

- [x] Texture atlas compositor (multiple textures → single PNG)
- [x] Layout JSON exporter (runtime data-driven format)
- [x] `GuiLayout` runtime library (Java, starter-only)
- [x] Forge Screen class codegen
- [x] Fabric Screen class codegen
- [x] NeoForge Screen class codegen
- [x] Export dialog with mod loader, package, mod ID, and class selection
- [x] Export preview (shows generated file tree before writing)
- [x] README.txt generation with integration instructions

**Deliverable:** Export a project and get a working Screen renderer that compiles in a Forge/Fabric project.

---

## Phase 6: Core UX Polish (v0.6)

- [x] Start panel with recent projects and MCP status
- [x] GUI size presets in New Project and Preferences
- [x] Grid visibility, grid size, and snap customization
- [x] Theme preference: dark and high contrast
- [x] Keyboard shortcuts reference
- [x] Status notifications for open/save/export/asset/MCP workflows
- [x] Backend export preview preflight
- [x] Pixel editor zoom controls
- [x] Core editor visual consistency pass

## Phase 6.x / Phase 7 Candidates

- [x] More starter templates: advanced machine, fluid tank, brewing stand, anvil, and Custom Grid as a default 3×3 starter layout
- [x] Minecraft visual fidelity pass: light/dark Minecraft-like themes, generated default GUI textures, and vanilla-aligned template slot metrics
- [x] Semantic slot roles and semantic group metadata for machine, player, hotbar, scrollable, virtual, upgrade, filter, ghost, and offhand slots
- [x] Scrollable inventory machine template with baked visible slots, scrollbar preview, and virtual slot grid metadata
- [x] Configurable simple vs modular code generation, including MCP and export-dialog overrides
- [x] MCP alpha ergonomics: bulk element/grid creation, compact asset metadata, default player inventory/hotbar grids, generated button visuals, and semantic preview warnings
- [x] MCP/UI polish v2: screenshot previews, button/toggle authoring, icon/tooltip metadata, overwrite previews, and validation polish
- [x] Editor UX polish: generated background elements, progress texture editing, persisted inspector dock, grouped Layers, reusable UV Editor, and UI/window layout reset
- [x] Parameterized custom grid generation through MCP `slot_grid_add`
- [ ] Expression support in animation bindings (`cook_time / total_cook_time`)
- [ ] Workspace/dock framework: movable and pinnable editor panels, workspace profiles, richer Asset/UV panes, and optional stacked/pinned Layers and Assets behavior
- [ ] Full runtime container/menu code generation for semantic inventories and virtual storage grids
- [ ] Resource pack export (Bedrock JSON UI) — research, not implemented
- [x] Font import for project font selection and canvas preview
- [ ] Custom runtime font support for exported Minecraft screens; exported runtime currently uses the platform text renderer
- [ ] CI/CD pipeline and installer builds (MSI, DMG, AppImage, deb)

---

## Phase 7: Community & Distribution (v1.0)

- [ ] Website with documentation
- [ ] Installer builds (MSI, DMG, AppImage, deb)
- [ ] MCP server marketplace listing
- [ ] Community template sharing (GitHub-based, import from URL)
- [ ] Integration guides for popular mods (Create, Thermal, Mekanism, etc.)
- [ ] Video tutorials

---

## Dependencies Between Phases

```
Phase 1 ──► Phase 2 ──► Phase 3 ──► Phase 5
                        │
                        └──► Phase 4 (parallel)

Phase 5 ──► Phase 6 ──► Phase 7
```

Phases 3 and 4 can be developed in parallel since they touch different systems (frontend pixel editor vs backend MCP).

---

## Status

| Phase | Status |
|-------|--------|
| 1: Foundation | Completed |
| 2: Editing & Templates | Completed |
| 3: Texture Tools | Completed |
| 4: MCP Server | Completed |
| 5: Export Pipeline | Completed |
| 6: Core UX Polish | Completed |
| 7: Community & Distribution | Not started |
