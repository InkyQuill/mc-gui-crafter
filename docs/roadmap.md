# MCGUI Crafter — Roadmap

## Phase 1: Foundation (v0.1)

**Goal:** Functional scaffold with project I/O and basic canvas.

- [ ] Tauri 2 + Svelte 5 + Vite project scaffold
- [ ] `.mcgui` format read/write (Rust backend)
- [ ] Canvas renderer: background texture, grid overlay, zoom/pan
- [ ] Element palette: slot, texture, text (drag to place)
- [ ] Property panel: coordinate editing for selected element
- [ ] Basic toolbar: new, open, save
- [ ] Coordinate readout on cursor hover
- [ ] Project store with undo/redo (single-level for now)

**Deliverable:** Can create a new project, place slots and textures, save and reopen.

---

## Phase 2: Editing & Templates (v0.2)

**Goal:** Productive editing with templates and the animation system.

- [ ] Selection, drag-to-move, and resize handles on canvas
- [ ] Multi-select and group elements
- [ ] Layer panel (element tree, z-order, visibility toggle)
- [ ] Template system: empty, furnace, 3×3 crafting, chest (9×3), chest (9×6)
- [ ] Template picker dialog on "New Project"
- [ ] Animation timeline: create/edit fill/progress animations
- [ ] Animation preview on canvas (play with scrubber)
- [ ] Data key binding for animations (`cook_progress`, `burn_time`, etc.)
- [ ] Full undo/redo stack
- [ ] Auto-save with `.mcgui.tmp` atomic rename
- [ ] Recent projects list

**Deliverable:** Can scaffold from templates, position elements precisely, define animations.

---

## Phase 3: Texture Tools (v0.3)

**Goal:** Built-in texture import and pixel-art editing.

- [ ] Texture import dialog (PNG, drag-and-drop from file system)
- [ ] Asset library panel (browse imported textures, preview)
- [ ] Pixel art editor (simple): pencil, eraser, color picker, fill
- [ ] Color palette management (Minecraft color palette presets)
- [ ] Texture replacement (swap asset on element)
- [ ] UV region selector for sprite sheets
- [ ] Canvas renderer uses actual textures (not gray placeholders)

**Deliverable:** Can create or import textures and place them on the GUI without external tools.

---

## Phase 4: MCP Server (v0.4)

**Goal:** AI tools can create and edit GUI projects programmatically.

- [x] MCP server core (JSON-RPC 2.0 over Streamable HTTP-style localhost endpoint)
- [ ] All tool implementations (see ADR 003)
- [ ] Shared project state between editor UI and MCP server
- [ ] Real-time sync: MCP changes reflected in editor canvas
- [ ] Tool schema documentation for AI providers
- [ ] HTTP transport option
- [ ] Example AI prompts and workflows in docs

**Deliverable:** Can connect Claude Desktop to MCGUI Crafter and spawn/modify GUI projects via natural language.

---

## Phase 5: Export Pipeline (v0.5)

**Goal:** Produce moddable output for Minecraft mod loaders.

- [ ] Texture atlas compositor (multiple textures → single PNG)
- [ ] Layout JSON exporter (runtime data-driven format)
- [ ] `GuiLayout` runtime library (Java, starter-only)
- [ ] Forge Screen class codegen
- [ ] Fabric Screen class codegen
- [ ] Export dialog with mod loader, mappings, package, mod ID selection
- [ ] Export preview (shows generated file tree before writing)
- [ ] README.txt generation with integration instructions

**Deliverable:** Export a project and get a working Screen renderer that compiles in a Forge/Fabric project.

---

## Phase 6: Polish & Advanced Features (v0.6–v0.8)

- [ ] More templates: advanced machine, fluid tank, brewing stand, anvil, custom grid
- [ ] Expression support in animation bindings (`cook_time / total_cook_time`)
- [ ] Animation types: cycle (sprite sheet), pulse, toggle
- [ ] NeoForge export target
- [ ] Resource pack export (Bedrock JSON UI) — research
- [ ] Custom font import (Minecraft bitmap font format)
- [ ] GUI size presets with smart inventory placement
- [ ] Grid snap customization
- [ ] Dark/light theme
- [ ] Keyboard shortcuts reference
- [ ] CI/CD pipeline (Tauri bundling for all platforms)

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
| 6: Polish & Advanced | Not started |
| 7: Community & Distribution | Not started |
