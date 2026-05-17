# ADR 002: Single-File Project Format (`.mcgui`)

**Date:** 2026-05-17  
**Status:** Accepted

## Context

MCGUI Crafter projects contain:
- A GUI element tree with pixel-precise coordinates
- Embedded texture assets (PNGs)
- Animation timeline definitions
- Metadata (GUI size, mod target, project version)
- Optional template overrides

All data must be stored in a single file for easy sharing and version control.

## Decision

**ZIP archive with `.mcgui` extension**, containing a structured manifest and asset tree.

```
furnace.mcgui
├── manifest.json         # Project metadata
├── layout.json           # Element tree with absolute coordinates
├── animations.json       # Animation timeline definitions
└── textures/
    ├── background.png
    ├── slot.png
    ├── arrow_fill.png
    └── flame_sprite.png
```

### `manifest.json`

```json
{
  "version": 1,
  "name": "Furnace GUI",
  "gui_size": { "width": 176, "height": 166 },
  "mod_target": "forge",
  "created_at": "2026-05-17T...",
  "modified_at": "2026-05-17T...",
  "mcp_server": "mc-gui-crafter"
}
```

### `layout.json`

```json
{
  "elements": [
    {
      "id": "bg",
      "type": "texture",
      "x": 0, "y": 0,
      "width": 176, "height": 166,
      "asset": "textures/background.png"
    },
    {
      "id": "input_slot",
      "type": "slot",
      "x": 56, "y": 17,
      "size": 18
    },
    {
      "id": "progress_arrow",
      "type": "progress",
      "x": 79, "y": 35,
      "width": 22, "height": 15,
      "direction": "left_to_right",
      "animation": "arrow_fill"
    },
    {
      "id": "title",
      "type": "text",
      "x": 8, "y": 6,
      "font": "minecraft:default",
      "color": 0x404040,
      "content": "{machine_name}",
      "shadow": true
    }
  ],
  "groups": [
    {
      "id": "player_inventory",
      "x": 8, "y": 84,
      "elements": ["inv_slot_0", "inv_slot_1", "..."]
    }
  ]
}
```

### `animations.json`

```json
{
  "arrow_fill": {
    "type": "fill",
    "data_key": "cook_progress",
    "texture": "textures/arrow_fill.png",
    "direction": "left_to_right",
    "min_value": 0.0,
    "max_value": 1.0
  },
  "flame": {
    "type": "cycle",
    "data_key": "burn_time",
    "texture": "textures/flame_sprite.png",
    "frame_count": 8,
    "fps": 12,
    "triggers_on": "burn_time > 0"
  }
}
```

### Element Types

| Type | Properties | Use |
|------|-----------|-----|
| `texture` | `asset`, `x`, `y`, `width`, `height`, `uv?` | Static background / decoration |
| `slot` | `x`, `y`, `size` (default 18) | Item slot placeholder |
| `progress` | `x`, `y`, `width`, `height`, `direction`, `animation` | Animated fill bar |
| `text` | `x`, `y`, `content`, `font`, `color`, `shadow` | Label or data binding |
| `fluid_tank` | `x`, `y`, `width`, `height`, `animation` | Fluid level display |
| `energy_bar` | `x`, `y`, `width`, `height`, `animation` | Energy level indicator |

### Coordinate System

Top-left origin, Y increases downward. Coordinates are relative to the GUI's top-left corner (which is typically positioned at `(guiLeft, guiTop)` during rendering). All values in pixels.

## Why ZIP over JSON with base64

| Concern | ZIP | JSON+base64 |
|---------|-----|-------------|
| Binary assets | Native | 33% size increase |
| Git diffs | Possible with diff tools | Human-readable but huge |
| Partial reads | Yes (stream individual entries) | Must parse entire file |
| Standard tooling | Any zip tool | JSON parser only |
| Streaming writes | Yes | Must rewrite entire file |

## Why ZIP over SQLite

- SQLite is great for queryable data but the project data is tree-structured, not relational
- Binary blob storage in SQLite is awkward and not transparent to users
- ZIP is universally understood; users can peek inside with any archive tool
- No extra dependency for read/write

## Consequences

- The Rust backend uses `zip` crate for read/write
- Asset import copies the PNG into the zip archive (no external file references)
- Auto-save creates temporary `.mcgui.tmp` then atomically renames
- Version field enables forward-compatible migration logic
