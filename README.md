# MCGUI Crafter

Desktop GUI editor for Minecraft machine screens. Design GUIs visually, animate progress bars, and export directly to Forge/Fabric/NeoForge Screen classes.

Built with **Tauri 2 + Svelte 5 + Rust**.

## Features

- WYSIWYG canvas editor with Minecraft-native coordinate system
- Drag-and-drop elements: slots, textures, text labels, progress bars, fluid tanks, energy bars
- Animation timeline: fill, cycle, pulse, toggle
- Pixel-art texture editor and numeric UV region editing for sprite sheets
- Generated Minecraft-like default GUI textures for new templates, with user-imported textures taking precedence
- Templates: furnace, crafting table, chest (9×3, 9×6), advanced machine, fluid tank, brewing stand, anvil, and a Custom Grid default 3×3 starter layout
- Parameterized custom grid generation is planned for a later template pass
- Font import supports project font selection and canvas preview; exported Minecraft runtime currently uses the platform text renderer unless custom runtime font support is added
- Start panel with recent projects, GUI size presets, and MCP endpoint status
- Persisted editor preferences for grid visibility, snap, grid sizing, default preset, and theme
- Keyboard shortcut reference and compact status notifications
- Export preview preflight with planned files, overwrite warnings, and missing-texture errors
- Pixel editor zoom controls: 1x, 2x, 4x, 8x, and fit
- Single-file `.mcgui` project format (zip archive)
- Built-in localhost Streamable HTTP MCP endpoint for AI-driven project manipulation
- Export to Forge, Fabric, and NeoForge Screen classes + texture atlases
- Arbitrary GUI sizes (not limited to vanilla Minecraft dimensions)

## Development

```bash
# Install dependencies
pnpm install

# Run in dev mode (Vite + Tauri)
pnpm tauri dev

# Build for production
pnpm tauri build
```

### Prerequisites

- Rust 1.75+ (`rustc`, `cargo`)
- Node.js 22.12+ or 20.19+
- pnpm
- Tauri system dependencies ([see docs](https://v2.tauri.app/start/prerequisites/))

## Opening Project Files

Passing a `.mcgui` path to the app opens that project on startup:

```bash
mc-gui-crafter /path/to/project.mcgui
```

If MCGUI Crafter is already running, the existing instance opens the project in
a new tab and focuses the main window instead of starting a second app window.

## Linux Wayland Startup

MCGUI Crafter sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` automatically on Linux
before GTK/WebKitGTK starts. This works around a known WebKitGTK/Wry/Tauri
Wayland crash that looks like:

```text
Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display.
```

If you need to test or override the behavior manually, launch with:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 ./mc-gui-crafter
```

If your compositor or GPU driver still fails, try forcing the X11 GTK backend
from an XWayland-capable session:

```bash
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 ./mc-gui-crafter
```

This issue is upstream in the Linux GTK/WebKitGTK graphics stack; keep
WebKitGTK, GTK, Mesa/NVIDIA drivers, and your compositor updated.

## Project Structure

```
src/                    # Svelte 5 frontend
├── lib/
│   ├── components/     # UI components (Canvas, Toolbar, etc.)
│   ├── stores/         # Svelte 5 runes state management
│   └── engine/         # Canvas rendering, hit testing, animation
└── App.svelte          # Root component

src-tauri/              # Rust backend
├── src/
│   ├── project/        # In-memory project model, element types
│   ├── format/         # .mcgui zip read/write
│   ├── animation/      # Animation timeline model
│   ├── export/         # Code generation (Forge, Fabric, NeoForge)
│   ├── mcp/            # MCP server implementation
│   ├── texture/        # PNG compositing, texture atlas
│   ├── templates/      # Built-in GUI templates
│   └── commands.rs     # Tauri IPC command handlers
└── Cargo.toml

docs/                   # Architecture docs and ADRs
```

## MCP Integration

MCGUI Crafter starts a localhost MCP endpoint from the running app instance. It mutates the same tabbed project sessions used by the editor UI, save/export, and backend undo/redo. The preferred port is `47381`; the app stores the selected port in `~/.config/mc-gui-crafter/config.json` and falls back to a free port only when the preferred port is busy.

```json
{
  "mcpServers": {
    "mc-gui-crafter": {
      "url": "http://127.0.0.1:47381/mcp"
    }
  }
}
```

The selected URL is shown in the start panel and available through the Tauri `mcp_status` command. See `docs/mcp.md` for client setup, the current tool list, and protocol notes.

## Feedback and Logs

Each app launch writes a JSONL session log under:

```text
~/.config/mc-gui-crafter/logs/session-*.jsonl
```

Logs capture UI actions, MCP tool calls, export preview warnings/errors, visible-size validation warnings, and user/AI feedback reports. When reporting a problem, attach:

- the latest `session-*.jsonl` file from the logs directory
- the `.mcgui` project file if it is safe to share
- any generated export directory or relevant `project_export_preview` output
- screenshots or `project_render` PNGs when the problem is visual

AI agents connected through MCP should call `session_report` with a short summary, severity, and reproduction details before asking the user to file an issue. After logging the report, ask the user to attach the latest session log to the issue.

See `docs/feedback.md` for the recommended issue format and privacy notes.

## License

MIT
