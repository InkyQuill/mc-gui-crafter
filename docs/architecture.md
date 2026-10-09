# Architecture

MCGUI Crafter is a Tauri 2 desktop application with a Svelte 5 frontend and a
PixiJS canvas. The Rust backend owns project sessions, persistence, undo/redo,
texture operations, exports, and a localhost MCP endpoint.

## State and mutations

`src-tauri/src/project/mod.rs` defines the project model and session manager.
Tauri commands (`src-tauri/src/commands.rs`) and MCP tools (`src-tauri/src/mcp/`)
operate on those shared sessions. Tools may target a `project_id`; otherwise
operations use the active session. Backend history is shared by UI and MCP edits.

`src/lib/stores/project.svelte.ts` mirrors the active backend project.
`src/lib/api.ts` wraps IPC and supplies browser-development mocks; mock behavior
is not evidence that native persistence or export works. Editor selection, zoom,
and drag state live in `editor.svelte.ts`; local preferences are separate from
saved project data.

Canvas interactions may preview changes locally, then commit through IPC.
Backend events refresh the mirror. Async operations must preserve the initiating
session identity when users switch tabs.

## Rendering and editing

`src/lib/engine/renderer.ts` draws textures, slots, text, animated elements,
grid/center axes and selection overlays. `Canvas.svelte` connects it to project
and editor state. PropertyPanel supports batch edits and mixed values; LayerPanel
manages selection and element/group presentation. AssetLibrary, PixelEditor and
UVEditor provide texture authoring. ProjectTabs exposes open sessions.

Coordinates use the main GUI's top-left origin with integer pixels. Attached
regions can extend beyond its bounds. `main_gui_center` controls exported screen
placement independently of visual bounds; see [ADR 004](adr/004-coordinate-system.md).

## Persistence, export and MCP

- `src-tauri/src/format/`: ZIP-based `.mcgui` read/write with temporary-file rename.
- `src-tauri/src/texture/`: compositing, texture rendering and validation.
- `src-tauri/src/export/`: preview and write from planned output files; Forge,
  Fabric and NeoForge scaffolds, layouts and textures.
- `src-tauri/src/templates/`: starter projects and generated default assets.
- `src-tauri/src/mcp/`: HTTP protocol, discovery and project tools.
- `src-tauri/src/session_log.rs`: JSONL session diagnostics.

See [MCP documentation](mcp.md), [feedback guidance](feedback.md), and
[release operation](releases.md). Remaining limitations and qualification work
are maintained in [backlog.md](backlog.md), not in historical review reports.
