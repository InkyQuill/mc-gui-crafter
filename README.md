# MCGUI Crafter

Desktop GUI editor for Minecraft machine screens. Design GUIs visually, animate progress bars, and export directly to Forge/Fabric/NeoForge Screen classes.

Built with **Tauri 2 + Svelte 5 + Rust**.

## Features

- WYSIWYG canvas editor with Minecraft-native coordinate system
- Drag-and-drop elements: slots, textures, text labels, progress bars, fluid tanks, energy bars
- Animation timeline: fill, cycle, pulse, toggle
- Pixel-art texture editor and numeric UV region editing for sprite sheets
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
- Node.js 22+
- pnpm
- Tauri system dependencies ([see docs](https://v2.tauri.app/start/prerequisites/))

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

MCGUI Crafter starts a localhost MCP endpoint from the running app instance. It mutates the same tabbed project sessions used by the editor UI, save/export, and backend undo/redo.

```json
{
  "mcpServers": {
    "mc-gui-crafter": {
      "url": "http://127.0.0.1:{port}/mcp"
    }
  }
}
```

The selected port is available through the Tauri `mcp_status` command. See `docs/mcp.md` for the current tool list and protocol notes.

## License

MIT
