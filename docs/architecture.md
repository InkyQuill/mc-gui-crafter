# MCGUI Crafter — Architecture

## Overview

MCGUI Crafter is a Tauri 2 desktop application for designing Minecraft machine GUIs. It provides a visual WYSIWYG editor, a pixel-art texture editor, an animation timeline, and a built-in MCP server for AI-driven project manipulation. Projects are saved as `.mcgui` zip archives and exported to Forge/Fabric/NeoForge Screen classes and texture atlases.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        MCGUI Crafter                            │
│                                                                 │
│  ┌────────────────────┐  ┌──────────────────────────────────┐   │
│  │   Svelte Frontend  │  │       Rust Backend (Tauri)       │   │
│  │                    │  │                                   │   │
│  │  ┌──────────────┐  │  │  ┌───────────┐  ┌─────────────┐  │   │
│  │  │ Canvas Editor│◄─┼──┼─►│  Project  │  │    MCP      │  │   │
│  │  │  (PixiJS)    │  │  │  │  Manager  │  │   Server    │  │   │
│  │  └──────────────┘  │  │  └─────┬─────┘  └──────┬──────┘  │   │
│  │                    │  │        │               │          │   │
│  │  ┌──────────────┐  │  │  ┌─────┴─────┐  ┌──────┴──────┐  │   │
│  │  │  Property    │◄─┼──┼─►│  Format   │  │   Export    │  │   │
│  │  │  Panel       │  │  │  │  (.mcgui) │  │  Pipeline   │  │   │
│  │  └──────────────┘  │  │  └───────────┘  └─────────────┘  │   │
│  │                    │  │                                   │   │
│  │  ┌──────────────┐  │  │  ┌───────────┐  ┌─────────────┐  │   │
│  │  │  Animation   │◄─┼──┼─►│  Texture  │  │   Template  │  │   │
│  │  │  Timeline    │  │  │  │ Composer  │  │   Library   │  │   │
│  │  └──────────────┘  │  │  └───────────┘  └─────────────┘  │   │
│  │                    │  │                                   │   │
│  │  ┌──────────────┐  │  └───────────────────────────────────┘   │
│  │  │  Pixel       │  │                                          │
│  │  │  Editor      │  │  External:                                │
│  │  └──────────────┘  │  ┌──────────────────────────────────┐    │
│  │                    │  │  AI Tools (Claude, Continue, …)  │    │
│  │  Svelte stores:    │  │  ┌────────────────────────────┐  │    │
│  │  project.svelte.ts │──┼──┤  MCP Client (HTTP /mcp)   ├──┼────┤
│  │  editor.svelte.ts  │  │  └────────────────────────────┘  │    │
│  │  animation.svelte.ts│  └──────────────────────────────────┘    │
│  └────────────────────┘                                           │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow

```
User Interaction            Tauri IPC              Rust Backend
───────────────    ───────────────────────    ──────────────────
                   
Drag slot on canvas  ──►  invoke("element_move", {id, x, y})
                                              │
                                              ├──► Project state updated
                                              │
                                              ◄──► Canvas re-render ◄──┘
                                              
Save project  ────────►  invoke("project_save")
                                              │
                                              └──► Write .mcgui zip
                                              
AI tool connects  ───►  MCP HTTP /mcp ────────►  Tool handler
                                              │
                                              ├──► Mutate project state
                                              │
                                              ◄──► Result
```

## Rust Backend Crates

| Crate | Purpose |
|-------|---------|
| `src-tauri/src/project/` | In-memory project model, undo/redo |
| `src-tauri/src/format/` | `.mcgui` zip read/write |
| `src-tauri/src/animation/` | Animation timeline model |
| `src-tauri/src/export/` | Code generation for Forge/Fabric/NeoForge |
| `src-tauri/src/mcp/` | MCP server, tool dispatch |
| `src-tauri/src/texture/` | PNG compositing, texture atlas build |
| `src-tauri/src/templates/` | Built-in template definitions |
| `src-tauri/src/commands.rs` | Tauri IPC command handlers |

## Frontend Component Tree

```
App.svelte
├── Toolbar.svelte            # New, open, save, export, undo, redo, preferences, help
├── ProjectTabs.svelte        # Open project sessions
├── StartPanel.svelte         # Empty-state launcher, recent projects, MCP status
├── Canvas.svelte             # WYSIWYG GUI editor (PixiJS/Canvas)
│   ├── GridOverlay           # Coordinate grid + snap
│   ├── ElementRenderer       # Renders each element in the tree
│   └── SelectionHandle       # Drag/resize handles for selected element
├── ElementPalette.svelte     # Drag source: slot, texture, text, progress…
├── PropertyPanel.svelte      # Edit selected element properties, UV, animation binding
├── LayerPanel.svelte         # Tree view of all elements, z-order, visibility, grouping
├── AssetLibrary.svelte       # Import, preview, edit, and remove texture assets
├── AnimationTimeline.svelte  # Keyframe editor at bottom
├── PixelEditor.svelte        # Pixel-art editor with zoom controls
├── NewProjectDialog.svelte   # Template and GUI preset selection
├── ExportDialog.svelte       # Export settings, backend preview, and preflight errors
├── PreferencesDialog.svelte  # Local editor preferences
├── ShortcutsDialog.svelte    # Shortcut reference
└── StatusMessages.svelte     # Toast-style status/error feedback
```

## State Management (Svelte 5 runes)

```typescript
// project.svelte.ts - global project state
class ProjectStore {
  sessions = $state<ProjectSessionSummary[]>([]);
  activeProjectId = $state<string | null>(null);
  elements = $state<Element[]>([]);
  groups = $state<Group[]>([]);
  animations = $state<Animation[]>([]);
  assets = $state<string[]>([]);
  guiSize = $state({ width: 176, height: 166 });
  modTarget = $state<"forge"|"fabric"|"neoforge">("forge");
  projectPath = $state<string | null>(null);
  isDirty = $state(false);
  
  // Derived
  elementById = $derived(new Map(elements.map(e => [e.id, e])));
  selectedElement = $state<string | null>(null);
}

// editor.svelte.ts - editor UI state
class EditorStore {
  zoom = $state(1);
  tool = $state<"select" | "pan" | "slot" | "texture" | "text">("select");
  mousePos = $state({ x: 0, y: 0 });
}

// preferences.svelte.ts - local UI preferences
class PreferencesStore {
  values = $state({
    showGrid: true,
    snapToGrid: true,
    majorGridSize: 18,
    minorGridSize: 2,
    snapSize: 1,
    defaultPreset: "vanilla_chest",
    theme: "dark",
  });
}
```

## Canvas Rendering Pipeline

```
1. Grid Overlay        — renders coord grid at current zoom
2. Background Texture  — renders the base GUI background
3. Static Textures     — renders non-animated texture elements
4. Slots               — renders slot placeholders (gray square + border)
5. Text Labels         — renders text elements with font rendering
6. Animated Elements   — renders progress bars, fluid tanks, etc.
7. Selection Overlay   — renders handles on selected element
8. Cursor Crosshair    — renders coordinate readout at cursor position
```

## Undo/Redo

The Rust backend owns project history per open project session. Durable UI and MCP mutations record snapshots in the active or explicitly targeted session, then update the Svelte mirror from backend state. Undo and redo therefore apply equally to UI-driven and MCP-driven edits.
