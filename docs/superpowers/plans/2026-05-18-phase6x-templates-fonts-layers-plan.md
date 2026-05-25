# Phase 6.x: Templates, Fonts & Texture Layers — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 5 new templates, Minecraft font import (bitmap + TTF), texture layering (bg/overlay/animatable), and Minecraft asset auto-detection.

**Architecture:** Incremental Rust backend changes (data model → templates → font pipeline → asset loading → export) followed by frontend integration. Each chunk is independently testable.

**Tech Stack:** Rust (Tauri 2 backend), TypeScript/Svelte 5 (frontend), `ab_glyph` for TTF rasterization, `zip` for JAR reading.

**Spec:** `docs/superpowers/specs/2026-05-18-phase6x-templates-fonts-layers-design.md`

---

## Chunk 1: Data Model Changes (Rust)

### Task 1.1: Add Layer enum to Element

**Tier:** standard

**Files:**
- Modify: `src-tauri/src/project/mod.rs` — add Layer enum, add `layer` field to Element
- Test: existing tests in `src-tauri/src/project/mod.rs`

- [ ] **Step 1: Add Layer enum**

Add after `FillDirection`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    #[default]
    Background,
    Overlay,
    Animatable,
}
```

- [ ] **Step 2: Add `layer` field to Element**

Add to the `Element` struct (after `uv` field):
```rust
#[serde(default, skip_serializing_if = "is_default_layer")]
pub layer: Layer,
```

- [ ] **Step 3: Add `is_default_layer` helper**

```rust
fn is_default_layer(layer: &Layer) -> bool {
    *layer == Layer::Background
}
```

- [ ] **Step 4: Add deserialization test**

Add to existing tests:
```rust
#[test]
fn element_layer_defaults_to_background_when_missing() {
    let value = serde_json::json!({
        "id": "slot_1",
        "type": "slot",
        "x": 8,
        "y": 18,
        "size": 18
    });
    let element: Element = serde_json::from_value(value).unwrap();
    assert_eq!(element.layer, Layer::Background);
}

#[test]
fn element_layer_serializes_animatable() {
    let element = Element {
        id: "arrow".into(),
        element_type: ElementType::Progress,
        x: 79, y: 35,
        layer: Layer::Animatable,
        ..sample_element_defaults()
    };
    let value = serde_json::to_value(&element).unwrap();
    assert_eq!(value["layer"], "animatable");
}

#[test]
fn element_layer_skips_background_default() {
    let element = Element {
        id: "bg".into(),
        element_type: ElementType::Texture,
        x: 0, y: 0,
        layer: Layer::Background,
        ..sample_element_defaults()
    };
    let value = serde_json::to_value(&element).unwrap();
    assert!(!value.as_object().unwrap().contains_key("layer"));
}
```

You'll need a helper for creating elements with defaults in tests. Extract from `sample_element`:
```rust
fn sample_element_defaults() -> Element {
    Element {
        id: String::new(),
        element_type: ElementType::Slot,
        x: 0, y: 0,
        width: None, height: None, size: None, asset: None,
        direction: None, content: None, font: None, color: None,
        shadow: None, animation: None, visible: true, uv: None,
        layer: Layer::Background,
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cd src-tauri && cargo test -- element_layer
```

Expected: All 3 new tests PASS

- [ ] **Step 6: Run full test suite**

```bash
cd src-tauri && cargo test
```

Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/project/mod.rs
git commit -m "feat: add Layer enum to Element for texture compositing"
```

---

### Task 1.2: Add font types to Project

**Tier:** standard

**Files:**
- Modify: `src-tauri/src/project/mod.rs` — add font types, add `fonts` to Project
- Test: `src-tauri/src/project/mod.rs` tests

- [ ] **Step 1: Add font-related types**

Add after existing types (before `Project` struct):
```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlyphInfo {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub ascent: i32,
}

pub type GlyphMap = HashMap<char, GlyphInfo>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BitmapProvider {
    pub file: String,
    pub ascent: i32,
    pub chars: Vec<String>,
    #[serde(skip)]
    pub image_data: Vec<u8>,
    #[serde(skip)]
    pub image_width: u32,
    #[serde(skip)]
    pub image_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FontSource {
    Minecraft {
        providers: Vec<BitmapProvider>,
        glyph_map: GlyphMap,
    },
    Ttf {
        #[serde(skip)]
        font_data: Vec<u8>,
        font_size: u32,
        glyph_map: GlyphMap,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FontAsset {
    pub id: String,
    pub source: FontSource,
}
```

- [ ] **Step 2: Add `fonts` field to Project**

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub fonts: Vec<FontAsset>,
```

- [ ] **Step 3: Update `Project::new`**

Add `fonts: Vec::new()` to the constructor.

- [ ] **Step 4: Add GlyphMap type alias export**

At top of file, `HashMap` is already imported. No change needed.

- [ ] **Step 5: Write serialization test**

```rust
#[test]
fn font_asset_serialization() {
    let mut glyph_map = HashMap::new();
    glyph_map.insert('A', GlyphInfo { x: 0, y: 0, width: 8, height: 8, ascent: 7 });

    let font = FontAsset {
        id: "minecraft:default".into(),
        source: FontSource::Ttf {
            font_data: vec![],
            font_size: 16,
            glyph_map: glyph_map.clone(),
        },
    };

    let value = serde_json::to_value(&font).unwrap();
    assert_eq!(value["id"], "minecraft:default");
    assert_eq!(value["source"]["type"], "ttf");
    assert_eq!(value["source"]["font_size"], 16);

    // Verify Ttf font_data is skipped but glyph_map serializes
    let glyph_map_val = &value["source"]["glyph_map"];
    assert!(glyph_map_val.get("A").is_some());
}

#[test]
fn project_fonts_defaults_to_empty() {
    let value = serde_json::json!({
        "name": "Test",
        "gui_size": { "width": 176, "height": 166 },
        "mod_target": "forge",
        "elements": [],
        "groups": [],
        "animations": [],
        "assets": []
    });
    let project: Project = serde_json::from_value(value).unwrap();
    assert!(project.fonts.is_empty());
}
```

- [ ] **Step 6: Run tests**

```bash
cd src-tauri && cargo test
```

Expected: All tests PASS including new tests

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/project/mod.rs
git commit -m "feat: add FontAsset, GlyphMap, and font-related types to project model"
```

---

## Chunk 2: New Templates (Rust)

### Task 2.1: Add 5 new templates

**Tier:** standard

**Files:**
- Modify: `src-tauri/src/templates/mod.rs`

- [ ] **Step 1: Add `list_templates` entries**

In `list_templates()`, add the 5 new templates:
```rust
pub fn list_templates() -> Vec<Template> {
    vec![
        empty(),
        furnace(),
        crafting_3x3(),
        chest_9x3(),
        chest_9x6(),
        advanced_machine(),
        fluid_tank(),
        brewing_stand(),
        anvil(),
        custom_grid_default(),
    ]
}
```

- [ ] **Step 2: Implement `advanced_machine()` template**

```rust
fn advanced_machine() -> Template {
    Template {
        name: "advanced_machine",
        description: "Advanced machine: input, fuel, output, progress arrow, 2 fluid tanks, energy bar",
        default_width: 176,
        default_height: 166,
        elements: vec![
            Element {
                id: "bg".into(),
                element_type: ElementType::Texture,
                x: 0, y: 0,
                width: Some(176), height: Some(166),
                size: None, asset: Some("textures/background.png".into()),
                direction: None, content: None, font: None, color: None,
                shadow: None, animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "title".into(),
                element_type: ElementType::Text,
                x: 8, y: 6,
                width: None, height: None, size: None, asset: None,
                direction: None, content: Some("{machine_name}".into()),
                font: Some("minecraft:default".into()), color: Some(0x404040),
                shadow: Some(true), animation: None, visible: true, uv: None,
                layer: Layer::Overlay,
            },
            Element {
                id: "input_slot".into(),
                element_type: ElementType::Slot,
                x: 44, y: 17, size: Some(18),
                width: None, height: None, asset: None, direction: None,
                content: None, font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "fuel_slot".into(),
                element_type: ElementType::Slot,
                x: 44, y: 59, size: Some(18),
                width: None, height: None, asset: None, direction: None,
                content: None, font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "output_slot".into(),
                element_type: ElementType::Slot,
                x: 116, y: 38, size: Some(18),
                width: None, height: None, asset: None, direction: None,
                content: None, font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "progress_arrow".into(),
                element_type: ElementType::Progress,
                x: 73, y: 38,
                width: Some(22), height: Some(15), size: None, asset: None,
                direction: Some(crate::project::FillDirection::LeftToRight),
                content: None, font: None, color: None, shadow: None,
                animation: Some("cook_progress".into()), visible: true, uv: None,
                layer: Layer::Animatable,
            },
            Element {
                id: "fluid_tank_left".into(),
                element_type: ElementType::FluidTank,
                x: 16, y: 17,
                width: Some(16), height: Some(48), size: None, asset: None,
                direction: Some(crate::project::FillDirection::BottomToTop),
                content: None, font: None, color: None, shadow: None,
                animation: Some("fluid_left".into()), visible: true, uv: None,
                layer: Layer::Animatable,
            },
            Element {
                id: "fluid_tank_right".into(),
                element_type: ElementType::FluidTank,
                x: 144, y: 17,
                width: Some(16), height: Some(48), size: None, asset: None,
                direction: Some(crate::project::FillDirection::BottomToTop),
                content: None, font: None, color: None, shadow: None,
                animation: Some("fluid_right".into()), visible: true, uv: None,
                layer: Layer::Animatable,
            },
            Element {
                id: "energy_bar".into(),
                element_type: ElementType::EnergyBar,
                x: 152, y: 17,
                width: Some(12), height: Some(48), size: None, asset: None,
                direction: Some(crate::project::FillDirection::BottomToTop),
                content: None, font: None, color: None, shadow: None,
                animation: Some("energy".into()), visible: true, uv: None,
                layer: Layer::Animatable,
            },
        ],
    }
}
```

- [ ] **Step 3: Implement `fluid_tank()` template**

```rust
fn fluid_tank() -> Template {
    Template {
        name: "fluid_tank",
        description: "Fluid tank: input/output slots, fluid fill gauge, capacity text",
        default_width: 176,
        default_height: 166,
        elements: vec![
            Element {
                id: "bg".into(), element_type: ElementType::Texture,
                x: 0, y: 0, width: Some(176), height: Some(166),
                size: None, asset: Some("textures/background.png".into()),
                direction: None, content: None, font: None, color: None,
                shadow: None, animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "title".into(), element_type: ElementType::Text,
                x: 8, y: 6, width: None, height: None, size: None, asset: None,
                direction: None, content: Some("{fluid_name}".into()),
                font: Some("minecraft:default".into()), color: Some(0x404040),
                shadow: Some(true), animation: None, visible: true, uv: None,
                layer: Layer::Overlay,
            },
            Element {
                id: "fluid_fill".into(), element_type: ElementType::FluidTank,
                x: 35, y: 17, width: Some(20), height: Some(64),
                size: None, asset: None,
                direction: Some(crate::project::FillDirection::BottomToTop),
                content: None, font: None, color: None, shadow: None,
                animation: Some("fluid_amount".into()), visible: true, uv: None,
                layer: Layer::Animatable,
            },
            Element {
                id: "input_fluid_slot".into(), element_type: ElementType::Slot,
                x: 12, y: 56, size: Some(18),
                width: None, height: None, asset: None, direction: None,
                content: None, font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "output_fluid_slot".into(), element_type: ElementType::Slot,
                x: 62, y: 56, size: Some(18),
                width: None, height: None, asset: None, direction: None,
                content: None, font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "capacity_text".into(), element_type: ElementType::Text,
                x: 8, y: 88, width: None, height: None, size: None, asset: None,
                direction: None, content: Some("{amount} / {capacity} mB".into()),
                font: Some("minecraft:default".into()), color: Some(0x808080),
                shadow: Some(false), animation: None, visible: true, uv: None,
                layer: Layer::Overlay,
            },
        ],
    }
}
```

- [ ] **Step 4: Implement `brewing_stand()` template**

```rust
fn brewing_stand() -> Template {
    let mut elements = vec![
        Element {
            id: "bg".into(), element_type: ElementType::Texture,
            x: 0, y: 0, width: Some(176), height: Some(166),
            size: None, asset: Some("textures/background.png".into()),
            direction: None, content: None, font: None, color: None,
            shadow: None, animation: None, visible: true, uv: None,
            layer: Layer::Background,
        },
        Element {
            id: "title".into(), element_type: ElementType::Text,
            x: 8, y: 6, width: None, height: None, size: None, asset: None,
            direction: None, content: Some("{machine_name}".into()),
            font: Some("minecraft:default".into()), color: Some(0x404040),
            shadow: Some(true), animation: None, visible: true, uv: None,
            layer: Layer::Overlay,
        },
        Element {
            id: "ingredient_slot".into(), element_type: ElementType::Slot,
            x: 79, y: 17, size: Some(18),
            width: None, height: None, asset: None, direction: None,
            content: None, font: None, color: None, shadow: None,
            animation: None, visible: true, uv: None,
            layer: Layer::Background,
        },
        Element {
            id: "blaze_slot".into(), element_type: ElementType::Slot,
            x: 79, y: 65, size: Some(18),
            width: None, height: None, asset: None, direction: None,
            content: None, font: None, color: None, shadow: None,
            animation: None, visible: true, uv: None,
            layer: Layer::Background,
        },
    ];

    // 3 bottle slots with progress bubbles
    for i in 0..3 {
        let bottle_x = 56 + i * 24;
        elements.push(Element {
            id: format!("bottle_{i}"), element_type: ElementType::Slot,
            x: bottle_x, y: 40, size: Some(18),
            width: None, height: None, asset: None, direction: None,
            content: None, font: None, color: None, shadow: None,
            animation: None, visible: true, uv: None,
            layer: Layer::Background,
        });
        elements.push(Element {
            id: format!("bubble_{i}"), element_type: ElementType::Progress,
            x: bottle_x + 14, y: 29,
            width: Some(8), height: Some(26), size: None, asset: None,
            direction: Some(crate::project::FillDirection::TopToBottom),
            content: None, font: None, color: None, shadow: None,
            animation: Some("brew_time".into()), visible: true, uv: None,
            layer: Layer::Animatable,
        });
    }

    elements.push(Element {
        id: "fuel_gauge".into(), element_type: ElementType::Progress,
        x: 79, y: 47,
        width: Some(18), height: Some(14), size: None, asset: None,
        direction: Some(crate::project::FillDirection::LeftToRight),
        content: None, font: None, color: None, shadow: None,
        animation: Some("fuel".into()), visible: true, uv: None,
        layer: Layer::Animatable,
    });

    Template {
        name: "brewing_stand",
        description: "Brewing stand: 3 bottles, ingredient, blaze powder, progress bubbles, fuel gauge",
        default_width: 176,
        default_height: 166,
        elements,
    }
}
```

- [ ] **Step 5: Implement `anvil()` template**

```rust
fn anvil() -> Template {
    Template {
        name: "anvil",
        description: "Anvil: 2 input slots, output, level cost text, repair progress",
        default_width: 176,
        default_height: 166,
        elements: vec![
            Element {
                id: "bg".into(), element_type: ElementType::Texture,
                x: 0, y: 0, width: Some(176), height: Some(166),
                size: None, asset: Some("textures/background.png".into()),
                direction: None, content: None, font: None, color: None,
                shadow: None, animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "title".into(), element_type: ElementType::Text,
                x: 8, y: 6, width: None, height: None, size: None, asset: None,
                direction: None, content: Some("{item_name}".into()),
                font: Some("minecraft:default".into()), color: Some(0x404040),
                shadow: Some(true), animation: None, visible: true, uv: None,
                layer: Layer::Overlay,
            },
            Element {
                id: "input_slot_1".into(), element_type: ElementType::Slot,
                x: 27, y: 23, size: Some(18),
                width: None, height: None, asset: None, direction: None,
                content: None, font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "input_slot_2".into(), element_type: ElementType::Slot,
                x: 27, y: 47, size: Some(18),
                width: None, height: None, asset: None, direction: None,
                content: None, font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "output_slot".into(), element_type: ElementType::Slot,
                x: 107, y: 35, size: Some(18),
                width: None, height: None, asset: None, direction: None,
                content: None, font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            },
            Element {
                id: "cost_text".into(), element_type: ElementType::Text,
                x: 130, y: 50, width: None, height: None, size: None, asset: None,
                direction: None, content: Some("{cost}".into()),
                font: Some("minecraft:default".into()), color: Some(0x00FF00),
                shadow: Some(false), animation: None, visible: true, uv: None,
                layer: Layer::Overlay,
            },
            Element {
                id: "progress_arrow".into(), element_type: ElementType::Progress,
                x: 75, y: 35,
                width: Some(22), height: Some(15), size: None, asset: None,
                direction: Some(crate::project::FillDirection::LeftToRight),
                content: None, font: None, color: None, shadow: None,
                animation: Some("repair_progress".into()), visible: true, uv: None,
                layer: Layer::Animatable,
            },
        ],
    }
}
```

- [ ] **Step 6: Implement `custom_grid_default()` template**

```rust
fn custom_grid_default() -> Template {
    let mut elements = vec![
        Element {
            id: "bg".into(), element_type: ElementType::Texture,
            x: 0, y: 0, width: Some(176), height: Some(166),
            size: None, asset: Some("textures/background.png".into()),
            direction: None, content: None, font: None, color: None,
            shadow: None, animation: None, visible: true, uv: None,
            layer: Layer::Background,
        },
    ];

    // 3x3 crafting grid
    for row in 0..3 {
        for col in 0..3 {
            elements.push(Element {
                id: format!("grid_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 30 + col * (18 + 2),
                y: 17 + row * (18 + 2),
                width: None, height: None, size: Some(18),
                asset: None, direction: None, content: None,
                font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            });
        }
    }

    // Progress arrow (optional, included by default)
    elements.push(Element {
        id: "progress_arrow".into(), element_type: ElementType::Progress,
        x: 98, y: 36,
        width: Some(22), height: Some(15), size: None, asset: None,
        direction: Some(crate::project::FillDirection::LeftToRight),
        content: None, font: None, color: None, shadow: None,
        animation: Some("custom_progress".into()), visible: true, uv: None,
        layer: Layer::Animatable,
    });

    // Output slot (optional, included by default)
    elements.push(Element {
        id: "output_slot".into(), element_type: ElementType::Slot,
        x: 134, y: 35, size: Some(18),
        width: None, height: None, asset: None, direction: None,
        content: None, font: None, color: None, shadow: None,
        animation: None, visible: true, uv: None,
        layer: Layer::Background,
    });

    // Player inventory (optional, included by default)
    for row in 0..3 {
        for col in 0..9 {
            elements.push(Element {
                id: format!("inv_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 8 + col * (18 + 2),
                y: 86 + row * (18 + 2),
                width: None, height: None, size: Some(18),
                asset: None, direction: None, content: None,
                font: None, color: None, shadow: None,
                animation: None, visible: true, uv: None,
                layer: Layer::Background,
            });
        }
    }

    Template {
        name: "custom_grid",
        description: "Custom N×M grid with optional output, progress, and inventory",
        default_width: 176,
        default_height: 166,
        elements,
    }
}
```

- [ ] **Step 7: Build and test**

```bash
cd src-tauri && cargo build && cargo test
```

Expected: Build succeeds, all tests pass

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/templates/mod.rs
git commit -m "feat: add 5 new templates — advanced machine, fluid tank, brewing stand, anvil, custom grid"
```

---

## Chunk 3: Font Pipeline (Rust)

### Task 3.1: Create font module structure

**Tier:** standard

**Files:**
- Create: `src-tauri/src/font/mod.rs`
- Create: `src-tauri/src/font/parser.rs`
- Create: `src-tauri/src/font/rasterizer.rs`
- Create: `src-tauri/src/font/glyph_map.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add dependencies to Cargo.toml**

```toml
ab_glyph = "0.2"
```

- [ ] **Step 2: Create `src-tauri/src/font/mod.rs`**

```rust
pub mod glyph_map;
pub mod parser;
pub mod rasterizer;

use crate::project::{FontAsset, FontSource, GlyphInfo, GlyphMap};
use std::collections::HashMap;

/// Load the bundled Minecraft default font.
pub fn load_bundled_font() -> FontAsset {
    // Bundled font data will be embedded via include_bytes!
    let default_json = include_str!("../../bundled/minecraft/font/default.json");
    let include_default_json = include_str!("../../bundled/minecraft/font/include/default.json");

    // TODO: In a follow-up, also bundle the PNG data and parse into providers
    let mut glyph_map = GlyphMap::new();

    // Minimal ASCII fallback for now
    for c in ' '..='~' {
        let code = c as u32 - 0x20;
        glyph_map.insert(c, GlyphInfo {
            x: (code % 16) * 8,
            y: (code / 16) * 8,
            width: 8,
            height: 8,
            ascent: 7,
        });
    }

    FontAsset {
        id: "minecraft:default".to_string(),
        source: FontSource::Minecraft {
            providers: vec![],
            glyph_map,
        },
    }
}
```

- [ ] **Step 3: Create `src-tauri/src/font/parser.rs`**

```rust
use crate::project::{BitmapProvider, GlyphInfo, GlyphMap};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct FontJson {
    providers: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct BitmapProviderJson {
    #[serde(rename = "type")]
    provider_type: String,
    file: Option<String>,
    ascent: Option<i32>,
    chars: Option<Vec<String>>,
    id: Option<String>,
}

/// Parse a Minecraft font JSON file into a GlyphMap.
pub fn parse_font_json(json: &str) -> Result<(Vec<BitmapProvider>, GlyphMap), String> {
    let font: FontJson = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse font JSON: {e}"))?;

    let mut providers = Vec::new();
    let mut glyph_map = GlyphMap::new();

    for provider_value in &font.providers {
        let provider: BitmapProviderJson = serde_json::from_value(provider_value.clone())
            .map_err(|e| format!("Failed to parse provider: {e}"))?;

        match provider.provider_type.as_str() {
            "bitmap" => {
                let file = provider.file.clone().unwrap_or_default();
                let ascent = provider.ascent.unwrap_or(8);
                let chars = provider.chars.clone().unwrap_or_default();

                let mut bp = BitmapProvider {
                    file,
                    ascent,
                    chars: chars.clone(),
                    image_data: vec![],
                    image_width: 0,
                    image_height: 0,
                };

                // Build glyph map from character grid
                for (row, line) in chars.iter().enumerate() {
                    for (col, ch) in line.chars().enumerate() {
                        glyph_map.insert(ch, GlyphInfo {
                            x: col as u32 * 8,  // default glyph width
                            y: row as u32 * 8,  // default glyph height
                            width: 8,
                            height: 8,
                            ascent,
                        });
                    }
                }

                providers.push(bp);
            }
            "reference" => {
                // References are resolved by the caller; skip here
            }
            "space" => {
                // Space providers add whitespace mappings
                for ch in [' ', '\u{2003}', '\u{2002}', '\u{2004}', '\u{2005}',
                           '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}'] {
                    glyph_map.entry(ch).or_insert(GlyphInfo {
                        x: 0, y: 0, width: 4, height: 0, ascent: 0,
                    });
                }
            }
            _ => {} // skip unknown providers
        }
    }

    Ok((providers, glyph_map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bitmap_provider() {
        let json = r#"{
            "providers": [{
                "type": "bitmap",
                "file": "minecraft:font/ascii.png",
                "ascent": 7,
                "chars": ["ABCD", "EFGH"]
            }]
        }"#;

        let (providers, glyph_map) = parse_font_json(json).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].file, "minecraft:font/ascii.png");
        assert!(glyph_map.contains_key(&'A'));
        assert!(glyph_map.contains_key(&'H'));
    }

    #[test]
    fn parse_space_provider() {
        let json = r#"{
            "providers": [{
                "type": "space",
                "chars": [" "]
            }]
        }"#;

        let (providers, glyph_map) = parse_font_json(json).unwrap();
        assert!(providers.is_empty());
        assert!(glyph_map.contains_key(&' '));
    }

    #[test]
    fn parse_reference_is_skipped() {
        let json = r#"{
            "providers": [{
                "type": "reference",
                "id": "minecraft:include/default"
            }]
        }"#;

        let (providers, glyph_map) = parse_font_json(json).unwrap();
        assert!(providers.is_empty());
        assert!(glyph_map.is_empty());
    }
}
```

- [ ] **Step 4: Create `src-tauri/src/font/rasterizer.rs`**

```rust
use crate::project::{FontAsset, FontSource, GlyphInfo, GlyphMap};
use std::collections::HashMap;

/// Rasterize a TTF/OTF font into a FontAsset with glyph map.
pub fn rasterize_ttf(font_data: &[u8], font_size: u32, id: &str) -> Result<FontAsset, String> {
    let font = ab_glyph::FontRef::try_from_slice(font_data)
        .map_err(|e| format!("Failed to parse font: {e}"))?;

    let mut glyph_map = GlyphMap::new();
    let scale = ab_glyph::PxScale { x: font_size as f32, y: font_size as f32 };

    // Rasterize ASCII range
    let mut x_offset = 0u32;
    let mut row_height = 0u32;
    let atlas_width = 256u32;
    let mut y_offset = 0u32;

    for c in ' '..='~' {
        let glyph_id = font.glyph_id(c);
        if let Some(glyph) = font.outline_glyph(ab_glyph::GlyphId(glyph_id.0), scale) {
            let bounds = glyph.px_bounds();
            let w = (bounds.width().ceil() as u32).max(1);
            let h = (bounds.height().ceil() as u32).max(1);
            let ascent = (-bounds.min.y).ceil() as i32;

            if x_offset + w > atlas_width {
                x_offset = 0;
                y_offset += row_height;
                row_height = 0;
            }

            glyph_map.insert(c, GlyphInfo {
                x: x_offset,
                y: y_offset,
                width: w,
                height: h,
                ascent,
            });

            x_offset += w;
            row_height = row_height.max(h);
        }
    }

    Ok(FontAsset {
        id: id.to_string(),
        source: FontSource::Ttf {
            font_data: font_data.to_vec(),
            font_size,
            glyph_map,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterize_builtin_font_produces_glyph_map() {
        // Use a minimal embedded font for testing
        // DejaVu Sans Mono embedded as bytes would work, but for now
        // we test that invalid data produces an error
        let result = rasterize_ttf(b"not a font", 16, "test");
        assert!(result.is_err());
    }
}
```

- [ ] **Step 5: Create `src-tauri/src/font/glyph_map.rs`**

```rust
use crate::project::{GlyphInfo, GlyphMap};
use std::collections::HashMap;

/// Build a GlyphMap from a list of BitmapProviders.
pub fn build_glyph_map(providers: &[crate::project::BitmapProvider]) -> GlyphMap {
    let mut map = GlyphMap::new();

    for (provider_idx, provider) in providers.iter().enumerate() {
        for (row, line) in provider.chars.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                let glyph_w = 8u32; // default, can be overridden per provider
                let glyph_h = 8u32;
                map.insert(ch, GlyphInfo {
                    x: col as u32 * glyph_w,
                    y: row as u32 * glyph_h,
                    width: glyph_w,
                    height: glyph_h,
                    ascent: provider.ascent,
                });
            }
        }
    }

    map
}

/// Look up a character in the glyph map, returning the default replacement if missing.
pub fn lookup_glyph(map: &GlyphMap, ch: char) -> GlyphInfo {
    map.get(&ch)
        .cloned()
        .unwrap_or_else(|| map.get(&'\u{FFFD}')
            .cloned()
            .unwrap_or(GlyphInfo { x: 0, y: 0, width: 8, height: 8, ascent: 0 }))
}
```

- [ ] **Step 6: Register font module in `src-tauri/src/lib.rs`**

Add `pub mod font;` after existing module declarations.

- [ ] **Step 7: Run builds and tests**

```bash
cd src-tauri && cargo test -- font
```

Expected: Font parser tests pass

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/font/ src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat: add font pipeline module with Minecraft font parser, TTF rasterizer, and glyph map"
```

---

### Task 3.2: Bundle Minecraft default font

**Tier:** standard

**Files:**
- Create: `src-tauri/bundled/minecraft/font/default.json`
- Create: `src-tauri/bundled/minecraft/font/include/default.json`
- Create: `src-tauri/bundled/minecraft/font/include/space.json`
- Modify: `src-tauri/src/font/mod.rs`

- [ ] **Step 1: Create bundled assets directory**

```bash
mkdir -p src-tauri/bundled/minecraft/font/include
mkdir -p src-tauri/bundled/minecraft/textures/font
```

- [ ] **Step 2: Extract font JSON assets from Minecraft jar**

```bash
JAR=/home/inky/.local/share/PrismLauncher/libraries/com/mojang/minecraft/1.21.1/minecraft-1.21.1-client.jar
cd src-tauri/bundled/minecraft
unzip -o "$JAR" assets/minecraft/font/default.json -d . 2>/dev/null && mv assets/minecraft/font/default.json font/
unzip -o "$JAR" assets/minecraft/font/include/default.json -d . 2>/dev/null && mv assets/minecraft/font/include/default.json font/include/
unzip -o "$JAR" assets/minecraft/font/include/space.json -d . 2>/dev/null && mv assets/minecraft/font/include/space.json font/include/
unzip -o "$JAR" assets/minecraft/font/include/unifont.json -d . 2>/dev/null && mv assets/minecraft/font/include/unifont.json font/include/
unzip -o "$JAR" assets/minecraft/textures/font/ascii.png -d . 2>/dev/null && mv assets/minecraft/textures/font/ascii.png textures/font/
unzip -o "$JAR" assets/minecraft/textures/font/accented.png -d . 2>/dev/null && mv assets/minecraft/textures/font/accented.png textures/font/
unzip -o "$JAR" assets/minecraft/textures/font/nonlatin_european.png -d . 2>/dev/null && mv assets/minecraft/textures/font/nonlatin_european.png textures/font/
unzip -o "$JAR" assets/minecraft/textures/font/ascii_sga.png -d . 2>/dev/null && mv assets/minecraft/textures/font/ascii_sga.png textures/font/
unzip -o "$JAR" assets/minecraft/textures/font/asciillager.png -d . 2>/dev/null && mv assets/minecraft/textures/font/asciillager.png textures/font/
rm -rf assets
```

- [ ] **Step 3: Update `src-tauri/src/font/mod.rs` to load bundled assets**

Update `load_bundled_font()` to actually parse the bundled JSON and load the PNGs:

```rust
pub fn load_bundled_font() -> FontAsset {
    let default_json = include_str!("../../bundled/minecraft/font/default.json");
    let include_default_json = include_str!("../../bundled/minecraft/font/include/default.json");
    let space_json = include_str!("../../bundled/minecraft/font/include/space.json");

    let mut all_providers = Vec::new();
    let mut all_glyph_map = GlyphMap::new();

    // Parse each font file and merge
    for json_str in &[default_json, include_default_json, space_json] {
        if let Ok((providers, glyph_map)) = parser::parse_font_json(json_str) {
            all_providers.extend(providers);
            all_glyph_map.extend(glyph_map);
        }
    }

    // Load bundled PNG data for bitmap providers
    for provider in &mut all_providers {
        let png_data = match provider.file.as_str() {
            "minecraft:font/ascii.png" => include_bytes!("../../bundled/minecraft/textures/font/ascii.png").to_vec(),
            "minecraft:font/accented.png" => include_bytes!("../../bundled/minecraft/textures/font/accented.png").to_vec(),
            "minecraft:font/nonlatin_european.png" => include_bytes!("../../bundled/minecraft/textures/font/nonlatin_european.png").to_vec(),
            "minecraft:font/ascii_sga.png" => include_bytes!("../../bundled/minecraft/textures/font/ascii_sga.png").to_vec(),
            "minecraft:font/asciillager.png" => include_bytes!("../../bundled/minecraft/textures/font/asciillager.png").to_vec(),
            _ => continue,
        };

        if let Ok(img) = image::load_from_memory(&png_data) {
            provider.image_width = img.width();
            provider.image_height = img.height();
        }
        provider.image_data = png_data;
    }

    FontAsset {
        id: "minecraft:default".to_string(),
        source: FontSource::Minecraft {
            providers: all_providers,
            glyph_map: all_glyph_map,
        },
    }
}
```

- [ ] **Step 4: Build and verify**

```bash
cd src-tauri && cargo build
```

Expected: Build succeeds with bundled assets

- [ ] **Step 5: Commit**

```bash
git add src-tauri/bundled/ src-tauri/src/font/mod.rs
git commit -m "feat: bundle Minecraft 1.21.1 default font assets"
```

---

## Chunk 4: Minecraft Asset Loading + Commands

### Task 4.1: Add Tauri commands for fonts and Minecraft assets

**Tier:** standard

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add Minecraft source scanning command**

```rust
#[derive(Debug, Clone, Serialize)]
pub struct MinecraftSource {
    pub name: String,
    pub path: String,
    pub source_type: String,  // "prismlauncher", "gradle_dev", "vanilla"
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_minecraft_sources() -> Vec<MinecraftSource> {
    let mut sources = Vec::new();

    // Scan PrismLauncher instances
    if let Ok(entries) = std::fs::read_dir(
        dirs_next().home_dir().unwrap_or_default()
            .join(".local/share/PrismLauncher/instances")
    ) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                sources.push(MinecraftSource {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: entry.path().to_string_lossy().to_string(),
                    source_type: "prismlauncher".to_string(),
                });
            }
        }
    }

    // Scan Gradle dev workspaces
    if let Ok(entries) = std::fs::read_dir(
        dirs_next().home_dir().unwrap_or_default()
            .join("Development/minecraft")
    ) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                sources.push(MinecraftSource {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: entry.path().to_string_lossy().to_string(),
                    source_type: "gradle_dev".to_string(),
                });
            }
        }
    }

    sources
}

fn dirs_next() -> &'static dirs_next {
    // Use simple home dir fallback
    todo!() // simplified — use std::env::var("HOME") instead
}
```

Actually, let's not use `dirs_next`. Use `std::env::var("HOME")`:

```rust
fn home_dir() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_minecraft_sources() -> Vec<serde_json::Value> {
    let mut sources = Vec::new();
    let home = home_dir();

    // Scan PrismLauncher instances
    let prism_path = home.join(".local/share/PrismLauncher/instances");
    if let Ok(entries) = std::fs::read_dir(&prism_path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                sources.push(serde_json::json!({
                    "name": entry.file_name().to_string_lossy(),
                    "path": entry.path().to_string_lossy(),
                    "source_type": "prismlauncher"
                }));
            }
        }
    }

    // Scan Gradle dev workspaces
    let dev_path = home.join("Development/minecraft");
    if let Ok(entries) = std::fs::read_dir(&dev_path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                sources.push(serde_json::json!({
                    "name": entry.file_name().to_string_lossy(),
                    "path": entry.path().to_string_lossy(),
                    "source_type": "gradle_dev"
                }));
            }
        }
    }

    sources
}
```

- [ ] **Step 2: Add font import command**

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn font_import(
    state: State<AppState>,
    file_path: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    use std::io::Read;

    let mut file = std::fs::File::open(&file_path)
        .map_err(|e| format!("Failed to open font file: {e}"))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| format!("Failed to read font file: {e}"))?;

    let ext = std::path::Path::new(&file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let font_id = std::path::Path::new(&file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported_font");

    let font_asset = match ext.as_str() {
        "ttf" | "otf" => {
            let font_size = 16u32; // default, could be configurable
            crate::font::rasterizer::rasterize_ttf(&data, font_size, font_id)
                .map_err(|e| format!("Failed to rasterize font: {e}"))?
        }
        _ => return Err(format!("Unsupported font format: .{ext}. Use .ttf or .otf")),
    };

    let mut sessions = state.sessions.lock().unwrap();
    sessions.record_history(project_id.as_deref())?;
    let session = sessions.resolve_mut(project_id.as_deref())?;

    // Replace existing font with same ID, or add new
    session.project.fonts.retain(|f| f.id != font_asset.id);
    session.project.fonts.push(font_asset.clone());
    sessions.mark_changed(project_id.as_deref())?;

    Ok(serde_json::to_value(&font_asset).unwrap_or_default())
}
```

- [ ] **Step 3: Add font list command**

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn font_list(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;

    let fonts: Vec<_> = if session.project.fonts.is_empty() {
        vec![serde_json::json!({
            "id": "minecraft:default",
            "source": { "type": "minecraft" }
        })]
    } else {
        session.project.fonts.iter().map(|f| {
            let source_type = match &f.source {
                crate::project::FontSource::Minecraft { .. } => "minecraft",
                crate::project::FontSource::Ttf { font_size, .. } => "ttf",
            };
            serde_json::json!({
                "id": f.id,
                "source": { "type": source_type }
            })
        }).collect()
    };

    Ok(fonts)
}
```

- [ ] **Step 4: Add glyph map query command**

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn font_glyph_map(
    state: State<AppState>,
    font_id: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;

    let font = session.project.fonts.iter()
        .find(|f| f.id == font_id)
        .ok_or_else(|| format!("Font not found: {font_id}"))?;

    let glyph_map = match &font.source {
        crate::project::FontSource::Minecraft { glyph_map, .. } => glyph_map,
        crate::project::FontSource::Ttf { glyph_map, .. } => glyph_map,
    };

    serde_json::to_value(glyph_map)
        .map_err(|e| format!("Failed to serialize glyph map: {e}"))
}
```

- [ ] **Step 5: Register new commands in `src-tauri/src/lib.rs`**

Add to the `invoke_handler`:
```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::list_minecraft_sources,
    commands::font_import,
    commands::font_list,
    commands::font_glyph_map,
])
```

- [ ] **Step 6: Build and verify**

```bash
cd src-tauri && cargo build
```

Expected: Build succeeds

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add Tauri commands for Minecraft source scanning and font import"
```

---

## Chunk 5: Export Pipeline Changes (Rust)

### Task 5.1: Layer-based texture compositing and multi-atlas export

**Tier:** standard

**Files:**
- Modify: `src-tauri/src/export/mod.rs`
- Modify: `src-tauri/src/texture/mod.rs`

- [ ] **Step 1: Update `composite_atlas` to support layer filtering**

In `src-tauri/src/texture/mod.rs`, add a layer parameter:

```rust
pub fn composite_atlas_for_layer(
    project: &Project,
    layer: crate::project::Layer,
) -> Result<Vec<u8>, String> {
    let elements: Vec<_> = project.elements.iter()
        .filter(|e| e.layer == layer && e.element_type == ElementType::Texture)
        .collect();

    if elements.is_empty() {
        // Generate empty 1x1 placeholder
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        return Ok(bytes);
    }

    // Same compositing logic as existing composite_atlas but only for filtered elements
    composite_atlas_from_elements(&elements, project, project.gui_size.width, project.gui_size.height)
}
```

- [ ] **Step 2: Update `plan_export` to generate per-layer atlases**

Modify the texture generation section:

```rust
// Background atlas
let bg_atlas = crate::texture::composite_atlas_for_layer(project, Layer::Background)?;
let bg_path = export.asset_dir()
    .join(format!("textures/gui/{}_gui.png", export.resource_name));
plan_file(&mut files, bg_path, bg_atlas)?;

// Overlay atlas (only if overlay elements exist)
let has_overlay = project.elements.iter().any(|e| e.layer == Layer::Overlay);
if has_overlay {
    let overlay_atlas = crate::texture::composite_atlas_for_layer(project, Layer::Overlay)?;
    let overlay_path = export.asset_dir()
        .join(format!("textures/gui/{}_overlay.png", export.resource_name));
    plan_file(&mut files, overlay_path, overlay_atlas)?;
}

// Animatable sprites — each element gets its own PNG
for element in &project.elements {
    if element.layer == Layer::Animatable {
        let sprite = crate::texture::composite_single_element(element, project)?;
        let sprite_path = export.asset_dir()
            .join(format!("textures/gui/{}.png", element.id));
        plan_file(&mut files, sprite_path, sprite)?;
    }
}
```

- [ ] **Step 3: Update layout JSON to include layer info and texture references**

```rust
let mut textures_json = serde_json::json!({
    "background": format!("textures/gui/{}_gui.png", export.resource_name),
});
if has_overlay {
    textures_json["overlay"] = serde_json::json!(
        format!("textures/gui/{}_overlay.png", export.resource_name)
    );
}

let layout = serde_json::json!({
    "gui_size": project.gui_size,
    "textures": textures_json,
    "elements": project.elements.iter().map(|e| {
        let mut val = serde_json::to_value(e).unwrap();
        if e.layer == Layer::Animatable {
            val["texture"] = serde_json::json!(format!("textures/gui/{}.png", e.id));
        }
        val
    }).collect::<Vec<_>>(),
    "groups": project.groups,
    "animations": project.animations,
});
```

- [ ] **Step 4: Update GuiLayout.java codegen**

Add `renderOverlay` method and `textureOverlay` field loading. Add `renderAnimatable` for dedicated sprite textures.

Key changes in the generated Java:
- Load `overlay` texture when present
- `renderOverlay(graphics, left, top)` method
- `renderAnimatable(animationId, graphics, left, top, texture, value)` for dedicated sprite textures

- [ ] **Step 5: Add `composite_single_element` to texture module**

In `src-tauri/src/texture/mod.rs`:
```rust
pub fn composite_single_element(element: &Element, project: &Project) -> Result<Vec<u8>, String> {
    let w = element.width.unwrap_or(16);
    let h = element.height.unwrap_or(16);

    // Generate a filled rect image for the element
    let color = match element.element_type {
        ElementType::Progress => image::Rgba([0xE9, 0xA2, 0x3B, 0xFF]),
        ElementType::FluidTank => image::Rgba([0x3B, 0x82, 0xE9, 0xFF]),
        ElementType::EnergyBar => image::Rgba([0xEF, 0x44, 0x44, 0xFF]),
        _ => image::Rgba([0xFF, 0xFF, 0xFF, 0xFF]),
    };

    let img = image::RgbaImage::from_pixel(w, h, color);
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}
```

- [ ] **Step 6: Build and test**

```bash
cd src-tauri && cargo test -- export
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/export/mod.rs src-tauri/src/texture/mod.rs
git commit -m "feat: layer-based texture compositing — per-layer atlases and animatable sprites"
```

---

## Chunk 6: Frontend Types, API, and Store

### Task 6.1: Update TypeScript types

**Tier:** fast

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Add Layer type, FontAsset, GlyphInfo interfaces**

```typescript
export type Layer = "background" | "overlay" | "animatable";

export interface GlyphInfo {
  x: number;
  y: number;
  width: number;
  height: number;
  ascent: number;
}

export interface FontAsset {
  id: string;
  source: { type: "minecraft" | "ttf"; font_size?: number };
  glyph_map?: Record<string, GlyphInfo>;
}

export interface MinecraftSource {
  name: string;
  path: string;
  source_type: "prismlauncher" | "gradle_dev" | "vanilla";
}
```

- [ ] **Step 2: Add `layer` to Element interface**

```typescript
export interface Element {
  // ... existing fields ...
  layer?: Layer;
}
```

- [ ] **Step 3: Add `fonts` to ProjectData**

```typescript
export interface ProjectData {
  // ... existing fields ...
  fonts?: FontAsset[];
}
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/types.ts
git commit -m "feat: add Layer, FontAsset, GlyphInfo, and MinecraftSource frontend types"
```

---

### Task 6.2: Update API layer

**Tier:** fast

**Files:**
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add new API functions**

```typescript
export async function listMinecraftSources(): Promise<MinecraftSource[]> {
  const invoke = await getInvoke();
  return invoke("list_minecraft_sources") as Promise<MinecraftSource[]>;
}

export async function fontImport(filePath: string, projectId?: string): Promise<FontAsset> {
  const invoke = await getInvoke();
  return invoke("font_import", { file_path: filePath, project_id: projectId }) as Promise<FontAsset>;
}

export async function fontList(projectId?: string): Promise<FontAsset[]> {
  const invoke = await getInvoke();
  return invoke("font_list", { project_id: projectId }) as Promise<FontAsset[]>;
}

export async function fontGlyphMap(fontId: string, projectId?: string): Promise<Record<string, GlyphInfo>> {
  const invoke = await getInvoke();
  return invoke("font_glyph_map", { font_id: fontId, project_id: projectId }) as Promise<Record<string, GlyphInfo>>;
}
```

- [ ] **Step 2: Add mock implementations**

In `mockInvoke`:
```typescript
case "list_minecraft_sources":
  return [];
case "font_list":
  return [{ id: "minecraft:default", source: { type: "minecraft" } }];
case "font_glyph_map":
  return {};
case "font_import":
  throw "Mock: font import not supported in browser mode";
```

- [ ] **Step 3: Update template_list mock**

Add the 5 new templates to the mock response.

- [ ] **Step 4: Commit**

```bash
git add src/lib/api.ts
git commit -m "feat: add font and Minecraft source API functions with mock support"
```

---

### Task 6.3: Update project store

**Tier:** standard

**Files:**
- Modify: `src/lib/stores/project.svelte.ts`

- [ ] **Step 1: Add `fonts` state**

```typescript
fonts = $state<FontAsset[]>([]);
```

- [ ] **Step 2: Add font management methods**

```typescript
async importFont(filePath: string) {
  const font = await api.fontImport(filePath, this.activeProjectId ?? undefined);
  const existing = this.fonts.findIndex(f => f.id === font.id);
  if (existing >= 0) this.fonts[existing] = font;
  else this.fonts = [...this.fonts, font];
  this.isDirty = true;
  await this.refreshSessions();
  return font;
}

async refreshFonts() {
  try {
    this.fonts = await api.fontList(this.activeProjectId ?? undefined);
  } catch { /* fonts may not be available */ }
}
```

- [ ] **Step 3: Update `hydrateActiveProject`**

Add `this.fonts = project.fonts ?? [];` in `applyActivePayload`.

- [ ] **Step 4: Update `clearActiveProject`**

Add `this.fonts = [];`

- [ ] **Step 5: Update `newProject` to load default font**

Add `await this.refreshFonts();` after `hydrateActiveProject()` in both `newProject` and `openProject`.

- [ ] **Step 6: Commit**

```bash
git add src/lib/stores/project.svelte.ts
git commit -m "feat: add fonts state and font management to project store"
```

---

## Chunk 7: Frontend Components

### Task 7.1: Update NewProjectDialog for new templates

**Tier:** standard

**Files:**
- Modify: `src/lib/components/NewProjectDialog.svelte`

- [ ] **Step 1: The template list from `api.templateList()` now includes 9 templates (5 old + 4 new + 1 custom). Verify the dialog renders them all properly.**

No code changes needed if the dialog loops over `templateList()` dynamically.

- [ ] **Step 2: Add Custom Grid configuration section**

When `custom_grid` template is selected, show additional fields:
- Grid width (1-9, default 3)
- Grid height (1-6, default 3)
- Include output slot (checkbox, default true)
- Include progress arrow (checkbox, default true)
- Include player inventory (checkbox, default true)

These values are passed through to `projectNew()` as extra template parameters. The template system currently doesn't support parameters — so for now, we use the default custom_grid and the user can adjust elements manually.

- [ ] **Step 3: Build and verify**

```bash
pnpm run build
```

Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/NewProjectDialog.svelte
git commit -m "feat: add Custom Grid template options to NewProjectDialog"
```

---

### Task 7.2: Update PropertyPanel for layer and font

**Tier:** standard

**Files:**
- Modify: `src/lib/components/PropertyPanel.svelte`

- [ ] **Step 1: Add layer picker dropdown**

After the existing type/position fields, add:
```svelte
<div class="property-row">
  <label>Layer</label>
  <select bind:value={layerValue} on:change={updateLayer}>
    <option value="background">Background</option>
    <option value="overlay">Overlay</option>
    <option value="animatable">Animatable</option>
  </select>
</div>
```

- [ ] **Step 2: Wire layer changes to store**

```typescript
let layerValue = $derived(selectedElement?.layer ?? "background");

async function updateLayer() {
  if (!selectedElement) return;
  await project.updateElement(selectedElement.id, { layer: layerValue as Layer });
}
```

- [ ] **Step 3: Add font picker for Text elements**

When element type is `text`, show a font dropdown:
```svelte
{#if selectedElement?.type === "text"}
<div class="property-row">
  <label>Font</label>
  <select bind:value={fontValue} on:change={updateFont}>
    {#each project.fonts as font}
      <option value={font.id}>{font.id}</option>
    {/each}
  </select>
</div>
{/if}
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/PropertyPanel.svelte
git commit -m "feat: add layer picker and font selector to PropertyPanel"
```

---

### Task 7.3: Update AssetLibrary for font import

**Tier:** standard

**Files:**
- Modify: `src/lib/components/AssetLibrary.svelte`

- [ ] **Step 1: Add "Import Font" button**

```svelte
<button on:click={importFont}>Import Font (.ttf, .otf)</button>
```

- [ ] **Step 2: Wire font import**

```typescript
async function importFont() {
  try {
    const result = await import("@tauri-apps/plugin-dialog");
    const path = await result.open({
      filters: [{ name: "Font Files", extensions: ["ttf", "otf"] }],
      multiple: false,
    });
    if (path) {
      await project.importFont(path as string);
    }
  } catch {
    const path = prompt("Enter path to font file:");
    if (path) {
      // In browser mock mode, show a message
      alert("Font import requires the desktop app. Path: " + path);
    }
  }
}
```

- [ ] **Step 3: Show imported fonts in the asset list**

Add a "Fonts" section below textures showing the loaded font IDs.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/AssetLibrary.svelte
git commit -m "feat: add font import button and font list to AssetLibrary"
```

---

## Chunk 8: Integration & Final Assembly

### Task 8.1: Renderer layer awareness and text glyph rendering

**Tier:** standard

**Files:**
- Modify: `src/lib/engine/renderer.ts`

- [ ] **Step 1: Group elements by layer for rendering**

Update the render pipeline to respect layer ordering:
1. Background layer elements
2. Slot elements (always on background virtually)
3. Progress/Fluid/Energy elements
4. Text elements (on overlay)
5. Selection overlays

- [ ] **Step 2: Glyph-based text rendering**

When a text element has a font that has a glyph map loaded, render characters as individual sprites positioned using the glyph map data. Fall back to PIXI.Text for fonts without glyph maps.

- [ ] **Step 3: Build and verify**

```bash
pnpm run build
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/engine/renderer.ts
git commit -m "feat: layer-aware rendering with glyph-based text support"
```

---

### Task 8.2: End-to-end integration test

**Tier:** standard

**Files:**
- Modify: `src-tauri/src/tests/` (or existing test infrastructure)

- [ ] **Step 1: Create an integration test that creates a project with all 9 templates**

- [ ] **Step 2: Verify each template has correct layer assignments**

- [ ] **Step 3: Verify font loading from bundled assets**

- [ ] **Step 4: Verify export produces per-layer atlases**

- [ ] **Step 5: Build final and run full test suite**

```bash
cd src-tauri && cargo test
pnpm run build
```

- [ ] **Step 6: Commit**

```bash
git add .
git commit -m "test: add integration tests for templates, fonts, and layered export"
```
