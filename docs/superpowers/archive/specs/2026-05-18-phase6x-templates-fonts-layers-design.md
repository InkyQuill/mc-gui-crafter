# Phase 6.x: Templates, Fonts & Texture Layers — Design Spec

## Overview

This spec covers the first sub-plan of the Phase 6.x / Phase 7 Candidates items from the roadmap:

1. **More templates**: advanced machine, fluid tank, brewing stand, anvil, custom grid
2. **Custom font import**: Minecraft bitmap font format + TTF/OTF support
3. **Texture layers**: bg, overlay, animatable sublayers composited into atlas at export
4. **Minecraft asset auto-detection**: PrismLauncher instances + Gradle dev workspaces

These features are designed together because they're interdependent — templates need layers, layers need export changes, fonts need asset loading.

---

## Data Model Changes

### Element `layer` property

New field on `Element` (`src-tauri/src/project/mod.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Background,   // composited into main gui texture
    Overlay,      // composited into overlay texture (renders above slots)
    Animatable,   // NOT composited; exported as individual sprite per element
}
```

Default is `Layer::Background` (via `#[serde(default)]` for backwards compatibility with existing `.mcgui` files). Templates set appropriate layers:
- Slot elements → `Background` (they're baked into the main gui texture)
- Static texture elements → `Background` or `Overlay` depending on visual intent
- Progress bars, fluid fills, energy bars → `Animatable`
- Text labels → `Background` or `Overlay`
- Background image → `Background`

### Font model

New file `src-tauri/src/font/mod.rs`:

```rust
pub struct FontAsset {
    pub id: String,              // "minecraft:default" or user-named
    pub source: FontSource,
}

pub enum FontSource {
    Minecraft { providers: Vec<BitmapProvider>, glyph_map: GlyphMap },
    Ttf { font_data: Vec<u8>, font_size: u32, glyph_map: GlyphMap },
}

pub struct BitmapProvider {
    pub file: String,            // PNG path relative to assets/
    pub ascent: i32,             // glyph baseline offset
    pub chars: Vec<String>,      // one string per row of the glyph sheet
    pub image_data: Vec<u8>,     // raw PNG bytes
    pub image_width: u32,
    pub image_height: u32,
}

pub type GlyphMap = HashMap<char, GlyphInfo>;

pub struct GlyphInfo {
    pub provider_index: usize,   // which BitmapProvider
    pub x: u32,                  // pixel position in provider image
    pub y: u32,
    pub width: u32,             // glyph width in pixels
    pub height: u32,            // glyph height (glyph_height, typically provider.ascent + descent)
    pub ascent: i32,
}
```

Font assets stored in `Project.fonts: Vec<FontAsset>`. Text elements reference a font via `element.font: Option<String>` (font ID).

### Project struct additions

```rust
pub struct Project {
    // ... existing fields ...
    pub fonts: Vec<FontAsset>,           // loaded font assets
}
```

---

## New Templates

All five new templates defined in `src-tauri/src/templates/mod.rs`:

### Advanced Machine (176×166)
- Background texture (`Background`)
- Title text (`Overlay`) — "{machine_name}"
- Input slot ×1
- Fuel slot ×1
- Output slot ×1
- Progress arrow (`Animatable`, left-to-right, data_key: `cook_time`)
- Fluid tank left (`Animatable`, bottom-to-top fill, data_key: `fluid_left`)
- Fluid tank right (`Animatable`, bottom-to-top fill, data_key: `fluid_right`)
- Energy bar (`Animatable`, bottom-to-top fill, data_key: `energy`)

### Fluid Tank (176×166)
- Background texture (`Background`)
- Title text (`Overlay`) — "{fluid_name}"
- Tank shell rectangle (slot border)
- Fluid fill (`Animatable`, bottom-to-top, data_key: `fluid_amount`)
- Input fluid slot
- Output fluid slot
- Capacity text (`Overlay`) — "{amount} / {capacity} mB"

### Brewing Stand (176×166)
- Background texture (`Background`)
- Title text (`Overlay`) — "{machine_name}"
- 3 bottle slots (bottom row)
- 1 ingredient slot (top)
- 1 blaze powder slot (fuel)
- 3 progress bubbles (`Animatable`, data_key: `brew_time`)
- Fuel gauge (`Animatable`, left-to-right, data_key: `fuel`)

### Anvil (176×166)
- Background texture (`Background`)
- Title text (`Overlay`) — "{item_name}"
- 2 input slots (left)
- 1 output slot (right)
- Level cost text (`Overlay`) — "{cost}"
- Progress arrow (`Animatable`, left-to-right, data_key: `repair_progress`)

### Custom Grid (configurable size)
- Background texture (`Background`)
- N×M slot grid (N, M configurable in dialog, default 3×3)
- Optional output slot (toggle in dialog)
- Optional progress arrow (`Animatable`, toggle in dialog)
- Optional player inventory slots (bottom 9×3, toggle in dialog)

The `NewProjectDialog.svelte` gains config options for Custom Grid: grid width, grid height, include output, include progress, include inventory.

---

## Font Pipeline

### New crate: `src-tauri/src/font/`

```text
src-tauri/src/font/
├── mod.rs           # FontAsset, parser entry point
├── parser.rs        # Minecraft font JSON parsing, provider resolution
├── rasterizer.rs    # TTF/OTF → bitmap atlas (ab_glyph)
└── glyph_map.rs     # GlyphMap construction from providers
```

### Bundled default font

The following files from the Minecraft 1.21.1 client jar are embedded in the binary:

- `assets/minecraft/font/default.json`
- `assets/minecraft/font/include/default.json`
- `assets/minecraft/font/include/space.json`
- `assets/minecraft/font/include/unifont.json`
- `assets/minecraft/textures/font/ascii.png`
- `assets/minecraft/textures/font/accented.png`
- `assets/minecraft/textures/font/nonlatin_european.png`
- `assets/minecraft/textures/font/ascii_sga.png`
- `assets/minecraft/textures/font/asciillager.png`

Embedded via `include_bytes!` or bundled as Tauri resources.

### Parser flow

```text
default.json → providers[]
  ├── "reference" → resolve include/<id>.json → recurse
  ├── "bitmap"   → load PNG, parse chars[] → build glyph map entries
  └── "space"    → add space mappings
```

Output: `Vec<BitmapProvider>` + `GlyphMap`

### TTF/OTF rasterization

Using the `ab_glyph` crate:

1. User imports `.ttf`/`.otf` file via asset library
2. Rasterize ASCII (U+0020–U+007E) + common extended ranges at the selected pixel size
3. Pack glyphs into a single atlas PNG
4. Store as `FontSource::Ttf` with the atlas image data and glyph map

### Text element rendering

- Canvas: `renderer.ts` looks up glyph positions from the glyph map (passed from Rust via IPC) and draws each character using PixiJS sprites
- Export: the export pipeline rasterizes text elements into their layer's atlas PNG using the Rust-side glyph map

### UI

- **Property panel**: Text elements get a font dropdown populated from `project.fonts`
- **Asset library**: "Import font" button → file picker for `.ttf`, `.otf`, or `.zip`/`.jar` (resource pack)
- **Preferences**: Default font selection (applies to new text elements)

---

## Minecraft Asset Auto-Detection

### Sources

1. **PrismLauncher instances**: `~/.local/share/PrismLauncher/instances/<name>/` → `minecraft/` subdirectory has assets
2. **Standard launcher**: `~/.minecraft/versions/<version>/<version>.jar`
3. **Gradle dev workspaces**: Scans `~/Development/minecraft/` (configurable) for Gradle projects → `src/main/resources/assets/<mod_id>/`

### JAR reading

Use Rust's `zip` crate to read `.jar` files (which are just ZIPs). Extract font JSON and PNG assets from `assets/minecraft/font/` and `assets/minecraft/textures/font/` paths.

### Asset import dialog

A new "Import from Minecraft" dialog that:
1. Auto-scans known paths for available sources
2. Shows a tree of discovered assets (textures, fonts)
3. User selects what to import
4. Assets are copied into the project's texture_data/font storage

---

## Export Pipeline Changes

### Layer-based compositing

Current behavior: all texture elements composited into one atlas PNG.

New behavior:

```text
elements.group_by(|e| e.layer)
  ├── Background → composite all Background elements → {resource_name}_gui.png
  ├── Overlay    → composite all Overlay elements    → {resource_name}_overlay.png
  └── Animatable → each element individually        → {element_id}.png
```

Text elements on Background/Overlay layers are rasterized into their atlas using the glyph map during compositing.

### Layout JSON format

```json
{
  "gui_size": { "width": 176, "height": 166 },
  "textures": {
    "background": "textures/gui/furnace_gui.png",
    "overlay": "textures/gui/furnace_overlay.png"
  },
  "elements": [
    {
      "id": "progress_arrow",
      "type": "progress",
      "layer": "animatable",
      "texture": "textures/gui/progress_arrow.png",
      "x": 79, "y": 35,
      "width": 22, "height": 15,
      "direction": "left_to_right",
      "animation": "arrow_fill"
    }
  ],
  "groups": [],
  "animations": [
    {
      "id": "arrow_fill",
      "type": "fill",
      "data_key": "cook_time",
      "direction": "left_to_right",
      "min_value": 0.0,
      "max_value": 100.0
    }
  ]
}
```

### GuiLayout.java changes

The generated `GuiLayout.java` class:

- Loads `background`, `overlay`, and per-animatable textures
- `renderTexture(graphics, left, top)` → draws background atlas
- `renderOverlay(graphics, left, top)` → draws overlay atlas
- `renderAnimatable(animationId, graphics, left, top, texture, value)` → draws scissor-clipped fill from the animatable sprite
- `renderProgress(...)` → updated to use dedicated animatable texture rather than fill rect

---

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/src/project/mod.rs` | Add `Layer` enum, `FontAsset`, `FontSource`, `BitmapProvider`, `GlyphInfo`, `GlyphMap` types; add `layer` field to `Element`; add `fonts` to `Project` |
| `src-tauri/src/templates/mod.rs` | Add 5 new template definitions |
| `src-tauri/src/font/mod.rs` | New: font module entry, FontAsset management |
| `src-tauri/src/font/parser.rs` | New: Minecraft font JSON parser |
| `src-tauri/src/font/rasterizer.rs` | New: TTF/OTF rasterization via ab_glyph |
| `src-tauri/src/font/glyph_map.rs` | New: GlyphMap construction |
| `src-tauri/src/export/mod.rs` | Layer-based compositing, multi-atlas output, updated codegen |
| `src-tauri/src/animation/mod.rs` | Minor: ensure Animation references align with new layer model |
| `src-tauri/src/commands.rs` | New commands: `load_font_from_path`, `import_minecraft_assets`, `list_minecraft_sources`, `get_glyph_map` |
| `src-tauri/src/lib.rs` | Register `font` module |
| `src-tauri/Cargo.toml` | Add `ab_glyph`, `zip` dependencies |
| `src/lib/types.ts` | Add `Layer` type, `FontAsset`, `GlyphInfo` interfaces |
| `src/lib/stores/project.svelte.ts` | Add `fonts` state |
| `src/lib/components/NewProjectDialog.svelte` | Custom Grid config, new templates in picker |
| `src/lib/components/PropertyPanel.svelte` | Font dropdown for text elements, layer picker |
| `src/lib/components/AssetLibrary.svelte` | Font import button, Minecraft asset import |
| `src/lib/components/ExportDialog.svelte` | Updated preview for new file tree |
| `src/lib/engine/renderer.ts` | Glyph-based text rendering, layer-aware rendering |

---

## Dependencies

- `ab_glyph` crate for TTF/OTF rasterization
- `zip` crate for reading Minecraft JAR files
- Bundled Minecraft 1.21.1 font assets (~50KB of PNGs + JSON)

---

## Backwards Compatibility

- **Element `layer` field**: `#[serde(default)]` — existing `.mcgui` files load with `Layer::Background` for all elements. Behavior is identical to before.
- **Fonts**: `Project.fonts` defaults to `vec![]`. On project load, if fonts is empty (old file), the bundled Minecraft default is auto-inserted. Text elements that reference a missing font ID fall back to the first available font.
- **Export**: Projects without `Animatable` elements produce the same output structure as before (just `_gui.png`). No overlay atlas is generated if no `Overlay` elements exist.

## Edge Cases

- **Empty `Project.fonts` on load**: The bundled Minecraft default is auto-added before text rendering or export.
- **Text element references a missing font ID**: The renderer and export overlay fall back to the first available font, which is the bundled default for projects without custom fonts. Property panel shows a warning icon on the font dropdown.
- **Custom Grid with 0×0**: Dialog enforces minimum 1×1 grid.
- **Minecraft asset sources not found**: Import dialog shows "No Minecraft installations detected" with a manual path browse option.
- **TTF with missing glyphs**: Characters not in the font render as the replacement character (U+FFFD) or a blank box.
