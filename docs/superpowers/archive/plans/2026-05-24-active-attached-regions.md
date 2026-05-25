# Active Attached Regions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add first-class static attached regions so Minecraft GUIs can have visible and interactive elements outside the main centered `gui_size` bounds while keeping vanilla container positioning stable.

**Architecture:** Keep `gui_size` as the main logical bounds and add a project-level `attached_regions` array plus optional `Element.attached_region`. Compute `visual_bounds` from main bounds, visible/exported elements, and attached regions, then use that computed rectangle for editor framing and background/overlay atlas export. Treat `toggleable` regions as preserved metadata only; full open/closed runtime behavior is explicitly deferred to a future roadmap item.

**Tech Stack:** Rust/Tauri 2 backend, Serde project models, Rust image compositing/export pipeline, MCP JSON-RPC tools, Svelte 5 runes, PixiJS editor renderer.

---

## File Structure

- Modify `src-tauri/src/project/mod.rs`: add `AttachedRegion`, `AttachedRegionAnchor`, `AttachedRegionState`, `VisualBounds`, `Element.attached_region`, project defaults, lookup helpers, and visual-bounds computation.
- Modify `src-tauri/src/texture/mod.rs`: composite background/overlay/preview against computed visual bounds and offset baked element coordinates into expanded atlases.
- Modify `src-tauri/src/export/mod.rs`: include `visual_bounds`, texture offsets, attached regions in layout JSON, export expanded atlases, and generate runtime background/overlay rendering with visual offsets while preserving `imageWidth`/`imageHeight`.
- Modify `src-tauri/src/commands.rs`: add Tauri commands for attached-region CRUD and move-with-elements.
- Modify `src-tauri/src/lib.rs`: register attached-region commands.
- Modify `src-tauri/src/mcp/mod.rs`: expose MCP attached-region tools, schemas, mutation handlers, and element `attached_region` support.
- Modify `src/lib/types.ts`: add attached-region and visual-bounds types; add `attached_region` to `Element` and `attached_regions` to `ProjectData`.
- Modify `src/lib/api.ts`: add frontend wrappers and mock handlers for attached-region commands.
- Modify `src/lib/stores/project.svelte.ts`: hydrate/persist regions, compute frontend visual bounds, add region CRUD/move helpers, and include region membership in movement ids.
- Modify `src/lib/engine/renderer.ts`: draw main bounds and visual bounds separately; render/hit-test outside coordinates; allow region selection/drag.
- Modify `src/lib/components/LayerPanel.svelte`: show attached regions as collapsible rows.
- Modify `src/lib/components/PropertyPanel.svelte`: edit selected element region membership and selected region properties.
- Modify `src/lib/stores/editor.svelte.ts`: add selected attached-region id and region selection revision.
- Modify `docs/mcp.md`: document attached-region workflow for agents.
- Modify `.agents/skills/mc-gui-crafter/SKILL.md` and `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`: teach LLMs when and how to use attached regions.
- Modify `docs/roadmap.md`: mark static attached regions planned/completed according to final implementation and add full toggleable runtime as future work.

## Task 1: Project Model And Visual Bounds

**Files:**
- Modify: `src-tauri/src/project/mod.rs`

- [ ] **Step 1: Add failing project model tests**

Add these tests to `#[cfg(test)] mod tests` in `src-tauri/src/project/mod.rs`:

```rust
#[test]
fn project_defaults_attached_regions_to_empty() {
    let json = r#"{
        "name": "Legacy",
        "gui_size": { "width": 176, "height": 166 },
        "mod_target": "forge",
        "elements": [],
        "groups": [],
        "animations": [],
        "assets": []
    }"#;

    let project: Project = serde_json::from_str(json).unwrap();

    assert!(project.attached_regions.is_empty());
}

#[test]
fn attached_region_and_element_membership_round_trip() {
    let mut project = Project::new("Attached", 100, 200, ModTarget::Forge);
    project.attached_regions.push(AttachedRegion {
        id: "returns_pocket".into(),
        anchor: AttachedRegionAnchor::Right,
        x: 100,
        y: 18,
        width: 54,
        height: 72,
        state: AttachedRegionState::Static,
        kind: Some("returns_pocket".into()),
        semantic_group: Some("food_returns".into()),
        visible: true,
    });
    let mut element = base_element_for_test("returns_0", ElementType::Slot, 108, 26);
    element.attached_region = Some("returns_pocket".into());
    project.elements.push(element);

    let json = serde_json::to_string(&project).unwrap();
    let loaded: Project = serde_json::from_str(&json).unwrap();

    assert_eq!(loaded.attached_regions.len(), 1);
    assert_eq!(loaded.attached_regions[0].anchor, AttachedRegionAnchor::Right);
    assert_eq!(loaded.attached_regions[0].state, AttachedRegionState::Static);
    assert_eq!(
        loaded.elements[0].attached_region.as_deref(),
        Some("returns_pocket")
    );
}

#[test]
fn visual_bounds_include_main_negative_elements_and_regions() {
    let mut project = Project::new("Visual", 100, 200, ModTarget::Forge);
    let mut flair = base_element_for_test("flair", ElementType::Texture, 84, -16);
    flair.width = Some(32);
    flair.height = Some(32);
    project.elements.push(flair);
    project.attached_regions.push(AttachedRegion {
        id: "side".into(),
        anchor: AttachedRegionAnchor::Right,
        x: 100,
        y: 20,
        width: 44,
        height: 80,
        state: AttachedRegionState::Static,
        kind: Some("side_controls".into()),
        semantic_group: None,
        visible: true,
    });

    let bounds = project.visual_bounds();

    assert_eq!(bounds.x, 0);
    assert_eq!(bounds.y, -16);
    assert_eq!(bounds.width, 144);
    assert_eq!(bounds.height, 216);
}

#[test]
fn visual_bounds_ignore_hidden_elements_and_regions() {
    let mut project = Project::new("Hidden Visual", 100, 200, ModTarget::Forge);
    let mut hidden = base_element_for_test("hidden", ElementType::Texture, -40, -40);
    hidden.width = Some(20);
    hidden.height = Some(20);
    hidden.visible = false;
    project.elements.push(hidden);
    project.attached_regions.push(AttachedRegion {
        id: "hidden_region".into(),
        anchor: AttachedRegionAnchor::Left,
        x: -60,
        y: 0,
        width: 20,
        height: 20,
        state: AttachedRegionState::Static,
        kind: None,
        semantic_group: None,
        visible: false,
    });

    let bounds = project.visual_bounds();

    assert_eq!(bounds, VisualBounds { x: 0, y: 0, width: 100, height: 200 });
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project::tests::project_defaults_attached_regions_to_empty --locked
cargo test --manifest-path src-tauri/Cargo.toml project::tests::attached_region_and_element_membership_round_trip --locked
cargo test --manifest-path src-tauri/Cargo.toml project::tests::visual_bounds_include_main_negative_elements_and_regions --locked
```

Expected: FAIL because attached-region types, fields, and `visual_bounds()` do not exist yet.

- [ ] **Step 3: Add attached-region model types**

In `src-tauri/src/project/mod.rs`, near `Layer` and `UvRect`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AttachedRegionAnchor {
    #[serde(alias = "Left")]
    Left,
    #[serde(alias = "Right")]
    Right,
    #[serde(alias = "Top")]
    Top,
    #[serde(alias = "Bottom")]
    Bottom,
    #[serde(alias = "Free")]
    Free,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AttachedRegionState {
    #[serde(alias = "Static")]
    Static,
    #[serde(alias = "Toggleable")]
    Toggleable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttachedRegion {
    pub id: String,
    pub anchor: AttachedRegionAnchor,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub state: AttachedRegionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_group: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
```

- [ ] **Step 4: Add project and element fields**

In `Element`, after `open_height`, add:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_region: Option<String>,
```

In `Project`, after `semantic_groups`, add:

```rust
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attached_regions: Vec<AttachedRegion>,
```

In `Project::new`, initialize:

```rust
            attached_regions: Vec::new(),
```

Update every manual `Element { ... }` construction in tests and templates by adding:

```rust
            attached_region: None,
```

Use the compiler to find all missing fields.

- [ ] **Step 5: Add visual bounds helpers**

In `impl Project`, add:

```rust
    pub fn find_attached_region(&self, id: &str) -> Option<&AttachedRegion> {
        self.attached_regions.iter().find(|region| region.id == id)
    }

    pub fn find_attached_region_mut(&mut self, id: &str) -> Option<&mut AttachedRegion> {
        self.attached_regions.iter_mut().find(|region| region.id == id)
    }

    pub fn visual_bounds(&self) -> VisualBounds {
        let mut min_x = 0_i32;
        let mut min_y = 0_i32;
        let mut max_x = self.gui_size.width as i32;
        let mut max_y = self.gui_size.height as i32;

        for element in self.elements.iter().filter(|element| element.visible) {
            let width = element.width.or(element.size).unwrap_or(16) as i32;
            let height = element.height.or(element.size).unwrap_or(16) as i32;
            min_x = min_x.min(element.x);
            min_y = min_y.min(element.y);
            max_x = max_x.max(element.x.saturating_add(width));
            max_y = max_y.max(element.y.saturating_add(height));
        }

        for region in self.attached_regions.iter().filter(|region| region.visible) {
            min_x = min_x.min(region.x);
            min_y = min_y.min(region.y);
            max_x = max_x.max(region.x.saturating_add(region.width as i32));
            max_y = max_y.max(region.y.saturating_add(region.height as i32));
        }

        VisualBounds {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).max(1) as u32,
            height: (max_y - min_y).max(1) as u32,
        }
    }
```

- [ ] **Step 6: Verify model tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project::tests::project_defaults_attached_regions_to_empty --locked
cargo test --manifest-path src-tauri/Cargo.toml project::tests::attached_region_and_element_membership_round_trip --locked
cargo test --manifest-path src-tauri/Cargo.toml project::tests::visual_bounds_include_main_negative_elements_and_regions --locked
cargo test --manifest-path src-tauri/Cargo.toml project::tests::visual_bounds_ignore_hidden_elements_and_regions --locked
```

Expected: PASS.

- [ ] **Step 7: Commit project model**

```bash
git add src-tauri/src/project/mod.rs src-tauri/src/templates/mod.rs src-tauri/src/export/mod.rs src-tauri/src/texture/mod.rs src-tauri/src/mcp/mod.rs
git commit -m "feat: model attached regions"
```

## Task 2: Texture Compositing With Visual Bounds

**Files:**
- Modify: `src-tauri/src/texture/mod.rs`

- [ ] **Step 1: Add failing texture tests**

Add tests to `#[cfg(test)] mod tests` in `src-tauri/src/texture/mod.rs`:

```rust
#[test]
fn background_export_expands_to_visual_bounds_for_outside_elements() {
    let mut project = Project::new("Outside", 100, 80, ModTarget::Forge);
    project.texture_data.insert(
        "textures/flair.png".into(),
        test_png(32, 32, Rgba([0xd7, 0xa3, 0x39, 0xff])),
    );
    let mut flair = button_element("flair", 84, -16);
    flair.element_type = ElementType::Texture;
    flair.width = Some(32);
    flair.height = Some(32);
    flair.asset = Some("textures/flair.png".into());
    project.elements.push(flair);

    let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
    let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

    assert_eq!(image.dimensions(), (116, 96));
    assert_eq!(image.get_pixel(84, 0).0, [0xd7, 0xa3, 0x39, 0xff]);
}

#[test]
fn background_export_remains_main_size_when_elements_stay_inside() {
    let mut project = Project::new("Inside", 100, 80, ModTarget::Forge);
    project.texture_data.insert(
        "textures/panel.png".into(),
        test_png(10, 10, Rgba([0x11, 0x22, 0x33, 0xff])),
    );
    let mut panel = button_element("panel", 10, 10);
    panel.element_type = ElementType::Texture;
    panel.width = Some(10);
    panel.height = Some(10);
    panel.asset = Some("textures/panel.png".into());
    project.elements.push(panel);

    let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
    let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

    assert_eq!(image.dimensions(), (100, 80));
}
```

- [ ] **Step 2: Run failing texture tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml background_export_expands_to_visual_bounds_for_outside_elements --locked
```

Expected: FAIL because `composite_atlas_for_layer` still creates an image exactly `gui_size`.

- [ ] **Step 3: Offset atlas compositing by visual bounds**

Change `composite_atlas_for_layer`:

```rust
pub fn composite_atlas_for_layer(project: &Project, layer: Layer) -> Result<Vec<u8>, String> {
    let bounds = project.visual_bounds();
    let mut img = RgbaImage::new(bounds.width, bounds.height);
    let has_elements = project
        .elements
        .iter()
        .any(|el| el.visible && el.layer == layer && is_baked_atlas_element(el));

    if !has_elements {
        return encode_png(img);
    }

    for el in &project.elements {
        if !el.visible || el.layer != layer || !is_baked_atlas_element(el) {
            continue;
        }
        overlay_baked_element(&mut img, project, el, bounds.x, bounds.y)?;
    }

    encode_png(img)
}
```

Update `overlay_baked_element`, `overlay_slot`, `overlay_button`, `overlay_button_icon`, `overlay_asset`, and `overlay_texture_data` to accept `offset_x: i32, offset_y: i32`. Apply the offset at the final overlay point:

```rust
let target_x = element.x - offset_x;
let target_y = element.y - offset_y;
image::imageops::overlay(img, &resized, target_x as i64, target_y as i64);
```

For centered button icons, compute:

```rust
let x = element.x - offset_x + (element_w.saturating_sub(target_w) / 2) as i32;
let y = element.y - offset_y + (element_h.saturating_sub(target_h) / 2) as i32;
```

- [ ] **Step 4: Update project preview to visual bounds**

Change `composite_project_preview` to use the same bounds:

```rust
pub fn composite_project_preview(project: &Project) -> Result<Vec<u8>, String> {
    let bounds = project.visual_bounds();
    let mut preview = RgbaImage::new(bounds.width, bounds.height);

    for layer in [Layer::Background, Layer::Overlay, Layer::Animatable] {
        for element in project
            .elements
            .iter()
            .filter(|element| element.visible && element.layer == layer)
        {
            if is_baked_atlas_element(element) {
                overlay_baked_element(&mut preview, project, element, bounds.x, bounds.y)?;
            } else {
                let element_png = composite_single_element(element, project)?;
                overlay_png(
                    &mut preview,
                    &element_png,
                    (element.x - bounds.x) as i64,
                    (element.y - bounds.y) as i64,
                    &element.id,
                )?;
            }
        }
    }

    encode_png(preview)
}
```

- [ ] **Step 5: Verify texture tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml background_export_expands_to_visual_bounds_for_outside_elements --locked
cargo test --manifest-path src-tauri/Cargo.toml background_export_remains_main_size_when_elements_stay_inside --locked
cargo test --manifest-path src-tauri/Cargo.toml texture::tests --locked
```

Expected: PASS.

- [ ] **Step 6: Commit texture visual bounds**

```bash
git add src-tauri/src/texture/mod.rs
git commit -m "feat: composite atlases with visual bounds"
```

## Task 3: Export Layout Offsets And Runtime Rendering

**Files:**
- Modify: `src-tauri/src/export/mod.rs`

- [ ] **Step 1: Add failing export tests**

Add tests to `#[cfg(test)] mod tests` in `src-tauri/src/export/mod.rs`:

```rust
#[test]
fn layout_json_includes_visual_bounds_offsets_and_attached_regions() {
    let output_dir = TempExportDir::new("attached-layout-json");
    let mut project = sample_project(ModTarget::Forge);
    project.gui_size = Size { width: 100, height: 80 };
    project.attached_regions.push(AttachedRegion {
        id: "returns_pocket".into(),
        anchor: AttachedRegionAnchor::Right,
        x: 100,
        y: 18,
        width: 54,
        height: 72,
        state: AttachedRegionState::Static,
        kind: Some("returns_pocket".into()),
        semantic_group: Some("food_returns".into()),
        visible: true,
    });
    let config = ExportConfig {
        mod_id: "testmod".into(),
        package: "com.example".into(),
        class_name: "AttachedGui".into(),
        output_dir: output_dir.path().to_string_lossy().into_owned(),
        settings_override: None,
        overwrite: false,
    };

    export_project(&project, &config, "forge").unwrap();
    let layout_json = read(
        &output_dir
            .path()
            .join("src/main/resources/assets/testmod/gui/attached_gui_layout.json"),
    );

    assert!(layout_json.contains(r#""visual_bounds""#));
    assert!(layout_json.contains(r#""visual_offset_x""#));
    assert!(layout_json.contains(r#""attached_regions""#));
    assert!(layout_json.contains(r#""returns_pocket""#));
}

#[test]
fn generated_runtime_draws_background_at_visual_offset_but_keeps_main_size() {
    let output_dir = TempExportDir::new("attached-runtime-offset");
    let mut project = sample_project(ModTarget::Forge);
    project.gui_size = Size { width: 100, height: 80 };
    let mut flair = button_element("flair", 84, -16, None);
    flair.element_type = ElementType::Texture;
    flair.width = Some(32);
    flair.height = Some(32);
    flair.asset = Some("textures/widgets/panel.png".into());
    project.elements.push(flair);
    let config = ExportConfig {
        mod_id: "testmod".into(),
        package: "com.example".into(),
        class_name: "AttachedGui".into(),
        output_dir: output_dir.path().to_string_lossy().into_owned(),
        settings_override: None,
        overwrite: false,
    };

    export_project(&project, &config, "forge").unwrap();
    let screen = read(
        &output_dir
            .path()
            .join("src/main/java/com/example/AttachedGuiScreen.java"),
    );
    let layout = read(&output_dir.path().join("src/main/java/com/example/GuiLayout.java"));

    assert!(screen.contains("this.imageWidth = 100;"));
    assert!(screen.contains("this.imageHeight = 80;"));
    assert!(layout.contains("left + data.textures.visualOffsetX"));
    assert!(layout.contains("top + data.textures.visualOffsetY"));
}
```

Also add imports in the test module:

```rust
use crate::project::{AttachedRegion, AttachedRegionAnchor, AttachedRegionState};
```

- [ ] **Step 2: Run failing export tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml layout_json_includes_visual_bounds_offsets_and_attached_regions --locked
cargo test --manifest-path src-tauri/Cargo.toml generated_runtime_draws_background_at_visual_offset_but_keeps_main_size --locked
```

Expected: FAIL because layout JSON and generated runtime do not include visual offsets yet.

- [ ] **Step 3: Add visual bounds to layout JSON**

In `layout_json_value`, compute bounds:

```rust
let visual_bounds = project.visual_bounds();
let mut textures_json = textures_json;
textures_json["visual_offset_x"] = serde_json::json!(visual_bounds.x);
textures_json["visual_offset_y"] = serde_json::json!(visual_bounds.y);
```

Then include:

```rust
"visual_bounds": visual_bounds,
"attached_regions": project.attached_regions,
```

Keep existing `gui_size`, `elements`, `groups`, `semantic_groups`, `animations`, and `export_settings` fields unchanged.

- [ ] **Step 4: Use visible overlay detection**

In `plan_export`, replace:

```rust
let has_overlay = project.elements.iter().any(|e| e.layer == Layer::Overlay);
```

with:

```rust
let has_overlay = project
    .elements
    .iter()
    .any(|element| element.visible && element.layer == Layer::Overlay);
```

- [ ] **Step 5: Skip hidden animatable sprite export**

In `plan_export`, change the animatable sprite loop condition:

```rust
if element.visible && element.layer == Layer::Animatable {
```

- [ ] **Step 6: Generate runtime texture offset fields**

In both Forge-like and Fabric `TexturesData` generated classes, add:

```java
@SerializedName("visual_offset_x")
Integer visualOffsetX;
@SerializedName("visual_offset_y")
Integer visualOffsetY;
int visualOffsetXOrDefault() { return visualOffsetX == null ? 0 : visualOffsetX; }
int visualOffsetYOrDefault() { return visualOffsetY == null ? 0 : visualOffsetY; }
```

Update generated background rendering:

Forge-like:

```java
graphics.blit(texture, left + visualOffsetX, top + visualOffsetY, 0, 0, BACKGROUND_WIDTH, BACKGROUND_HEIGHT, BACKGROUND_WIDTH, BACKGROUND_HEIGHT);
```

Fabric:

```java
context.drawTexture(texture, left + visualOffsetX, top + visualOffsetY, 0, 0, BACKGROUND_WIDTH, BACKGROUND_HEIGHT, BACKGROUND_WIDTH, BACKGROUND_HEIGHT);
```

Use names that match the generated class structure. If the current generated `GuiLayout` stores only `ResourceLocation texture`, add `private final int visualOffsetX; private final int visualOffsetY; private final int backgroundWidth; private final int backgroundHeight;` so rendering can use atlas dimensions and offsets.

- [ ] **Step 7: Pass atlas dimensions to generated runtime**

In generated `load`, after parsing `LayoutData data`, compute:

```java
int visualOffsetX = data.textures.visualOffsetXOrDefault();
int visualOffsetY = data.textures.visualOffsetYOrDefault();
int backgroundWidth = data.visualBounds != null ? data.visualBounds.width : WIDTH;
int backgroundHeight = data.visualBounds != null ? data.visualBounds.height : HEIGHT;
```

Add a generated `VisualBounds` class:

```java
private static final class VisualBounds {
    int x;
    int y;
    int width;
    int height;
}
```

Add `VisualBounds visualBounds;` to `LayoutData`.

- [ ] **Step 8: Verify export tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml layout_json_includes_visual_bounds_offsets_and_attached_regions --locked
cargo test --manifest-path src-tauri/Cargo.toml generated_runtime_draws_background_at_visual_offset_but_keeps_main_size --locked
cargo test --manifest-path src-tauri/Cargo.toml export::tests --locked
```

Expected: PASS.

- [ ] **Step 9: Commit export offsets**

```bash
git add src-tauri/src/export/mod.rs
git commit -m "feat: export visual bounds for attached regions"
```

## Task 4: Backend Commands For Attached Regions

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add command tests**

Add tests to `#[cfg(test)] mod tests` in `src-tauri/src/commands.rs`:

```rust
#[test]
fn attached_region_create_update_remove_record_history() {
    let mut sessions = ProjectSessionManager::default();
    let project_id =
        sessions.create_session(Project::new("Attached", 176, 166, ModTarget::Forge));
    let region = crate::project::AttachedRegion {
        id: "returns_pocket".into(),
        anchor: crate::project::AttachedRegionAnchor::Right,
        x: 100,
        y: 18,
        width: 54,
        height: 72,
        state: crate::project::AttachedRegionState::Static,
        kind: Some("returns_pocket".into()),
        semantic_group: Some("food_returns".into()),
        visible: true,
    };

    let created =
        create_attached_region_in_session(&mut sessions, Some(&project_id), region).unwrap();
    assert_eq!(created.id, "returns_pocket");

    let changed = update_attached_region_in_session(
        &mut sessions,
        Some(&project_id),
        "returns_pocket".into(),
        serde_json::json!({ "x": 112, "state": "toggleable" }),
    )
    .unwrap();
    assert_eq!(changed.x, 112);
    assert_eq!(changed.state, crate::project::AttachedRegionState::Toggleable);

    let removed =
        remove_attached_region_in_session(&mut sessions, Some(&project_id), "returns_pocket").unwrap();
    assert!(removed);

    let session = sessions.resolve(Some(&project_id)).unwrap();
    assert!(session.project.attached_regions.is_empty());
    assert_eq!(session.revision, 3);
}

#[test]
fn attached_region_move_with_elements_updates_absolute_child_coordinates() {
    let mut sessions = ProjectSessionManager::default();
    let project_id =
        sessions.create_session(Project::new("Move Region", 176, 166, ModTarget::Forge));
    {
        let session = sessions.resolve_mut(Some(&project_id)).unwrap();
        session.project.attached_regions.push(crate::project::AttachedRegion {
            id: "returns_pocket".into(),
            anchor: crate::project::AttachedRegionAnchor::Right,
            x: 100,
            y: 18,
            width: 54,
            height: 72,
            state: crate::project::AttachedRegionState::Static,
            kind: None,
            semantic_group: None,
            visible: true,
        });
        let mut slot = crate::templates::base_element_for_test("returns_0", crate::project::ElementType::Slot, 108, 26);
        slot.attached_region = Some("returns_pocket".into());
        session.project.elements.push(slot);
    }

    let moved = move_attached_region_with_elements_in_session(
        &mut sessions,
        Some(&project_id),
        "returns_pocket".into(),
        110,
        28,
    )
    .unwrap();

    assert_eq!(moved.x, 110);
    let session = sessions.resolve(Some(&project_id)).unwrap();
    let child = session.project.find_element("returns_0").unwrap();
    assert_eq!((child.x, child.y), (118, 36));
}
```

- [ ] **Step 2: Run failing command tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml attached_region_create_update_remove_record_history --locked
cargo test --manifest-path src-tauri/Cargo.toml attached_region_move_with_elements_updates_absolute_child_coordinates --locked
```

Expected: FAIL because commands do not exist.

- [ ] **Step 3: Add command functions**

In `src-tauri/src/commands.rs`, add internal session helpers first:

```rust
fn create_attached_region_in_session(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    region: crate::project::AttachedRegion,
) -> Result<crate::project::AttachedRegion, String> {
    if sessions
        .resolve(project_id)?
        .project
        .find_attached_region(&region.id)
        .is_some()
    {
        return Err(format!("Attached region already exists: {}", region.id));
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.attached_regions.push(region.clone());
    sessions.mark_changed(project_id)?;
    Ok(region)
}
```

Add matching helpers:

- `update_attached_region_in_session(sessions, project_id, id, changes) -> Result<AttachedRegion, String>`;
- `remove_attached_region_in_session(sessions, project_id, id) -> Result<bool, String>`;
- `move_attached_region_with_elements_in_session(sessions, project_id, id, x, y) -> Result<AttachedRegion, String>`;

The update helper merges `changes` into the current region via `serde_json::to_value`, ignores `id`, deserializes `AttachedRegion`, and replaces only if changed.

The remove helper removes the region, clears matching `element.attached_region`, records history only when the region existed, and returns `Ok(false)` when it did not exist.

The move helper computes `dx = new_x - old_x`, `dy = new_y - old_y`, updates the region `x/y`, and adds `dx/dy` to every element whose `attached_region` matches the region id.

Then add public Tauri commands:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn attached_region_create(
    state: State<AppState>,
    project_id: Option<String>,
    region: crate::project::AttachedRegion,
) -> Result<crate::project::AttachedRegion, String> {
    let mut sessions = state.sessions.lock().unwrap();
    create_attached_region_in_session(&mut sessions, project_id.as_deref(), region)
}
```

The public wrappers delegate to the helpers and adapt return payloads:

- `attached_region_update` returns `AttachedRegion`;
- `attached_region_remove` returns `serde_json::json!({ "removed": removed })`;
- `attached_region_list` returns `Vec<AttachedRegion>`;
- `attached_region_move_with_elements` returns `AttachedRegion`.

- [ ] **Step 4: Register commands**

In `src-tauri/src/lib.rs`, add to `tauri::generate_handler!`:

```rust
commands::attached_region_create,
commands::attached_region_update,
commands::attached_region_remove,
commands::attached_region_list,
commands::attached_region_move_with_elements,
```

- [ ] **Step 5: Verify command tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml attached_region_create_update_remove_record_history --locked
cargo test --manifest-path src-tauri/Cargo.toml attached_region_move_with_elements_updates_absolute_child_coordinates --locked
cargo check --manifest-path src-tauri/Cargo.toml --locked
```

Expected: PASS.

- [ ] **Step 6: Commit backend commands**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add attached region commands"
```

## Task 5: MCP Attached Region Tools

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`

- [ ] **Step 1: Add MCP tool tests**

Add tests to `#[cfg(test)] mod tests` in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn tools_list_exposes_attached_region_tools() {
    let state = test_state();
    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "tools",
            "method": "tools/list"
        }),
        &state,
    );

    let names = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"attached_region_add"));
    assert!(names.contains(&"attached_region_update"));
    assert!(names.contains(&"attached_region_remove"));
    assert!(names.contains(&"attached_region_list"));
    assert!(names.contains(&"attached_region_move_with_elements"));
}

#[test]
fn attached_region_add_and_move_with_elements_mutate_live_session() {
    let state = test_state();
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.create_session(Project::new("Attached MCP", 100, 80, ModTarget::Forge));
    }

    let add = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "add-region",
            "method": "tools/call",
            "params": {
                "name": "attached_region_add",
                "arguments": {
                    "id": "returns_pocket",
                    "anchor": "right",
                    "x": 100,
                    "y": 18,
                    "width": 54,
                    "height": 72,
                    "state": "static",
                    "kind": "returns_pocket",
                    "semantic_group": "food_returns"
                }
            }
        }),
        &state,
    );
    assert!(add.get("error").is_none());

    let element = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "add-element",
            "method": "tools/call",
            "params": {
                "name": "element_add",
                "arguments": {
                    "id": "returns_0",
                    "type": "slot",
                    "x": 108,
                    "y": 26,
                    "size": 18,
                    "attached_region": "returns_pocket"
                }
            }
        }),
        &state,
    );
    assert!(element.get("error").is_none());

    let moved = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "move-region",
            "method": "tools/call",
            "params": {
                "name": "attached_region_move_with_elements",
                "arguments": {
                    "id": "returns_pocket",
                    "x": 110,
                    "y": 28
                }
            }
        }),
        &state,
    );
    assert!(moved.get("error").is_none());

    let sessions = state.sessions.lock().unwrap();
    let active = sessions.active_session().unwrap();
    assert_eq!(active.project.attached_regions[0].x, 110);
    assert_eq!(active.project.find_element("returns_0").unwrap().x, 118);
}
```

- [ ] **Step 2: Run failing MCP tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_attached_region_tools --locked
cargo test --manifest-path src-tauri/Cargo.toml attached_region_add_and_move_with_elements_mutate_live_session --locked
```

Expected: FAIL because tools do not exist.

- [ ] **Step 3: Add schema helpers**

In `src-tauri/src/mcp/mod.rs`, add constants:

```rust
const ATTACHED_REGION_ANCHOR_DESCRIPTION: &str = "Attached region anchor. Accepted values: left, right, top, bottom, free.";
const ATTACHED_REGION_STATE_DESCRIPTION: &str = "Attached region state. Accepted values: static, toggleable. Toggleable is metadata only in this release.";
```

Add:

```rust
fn attached_region_props(require_region: bool) -> serde_json::Value {
    project_schema(vec![
        ("id", serde_json::json!({ "type": "string", "description": "Attached region ID" }), require_region),
        ("anchor", serde_json::json!({ "type": "string", "description": ATTACHED_REGION_ANCHOR_DESCRIPTION }), true),
        ("x", serde_json::json!({ "type": "integer", "description": "Absolute X relative to main GUI origin" }), true),
        ("y", serde_json::json!({ "type": "integer", "description": "Absolute Y relative to main GUI origin" }), true),
        ("width", serde_json::json!({ "type": "integer", "description": "Region width" }), true),
        ("height", serde_json::json!({ "type": "integer", "description": "Region height" }), true),
        ("state", serde_json::json!({ "type": "string", "description": ATTACHED_REGION_STATE_DESCRIPTION }), false),
        ("kind", serde_json::json!({ "type": "string", "description": "Optional descriptive kind, e.g. flair, upgrade_panel, returns_pocket, side_controls" }), false),
        ("semantic_group", serde_json::json!({ "type": "string", "description": "Optional associated semantic group id" }), false),
        ("visible", serde_json::json!({ "type": "boolean", "description": "Whether region contributes to visual bounds" }), false),
    ])
}
```

- [ ] **Step 4: Register MCP tools**

Add tool descriptors in `tools/list` construction:

```rust
td("attached_region_add", "Create an attached region with absolute coordinates", attached_region_props(true)),
td("attached_region_update", "Update attached region fields", project_props(&[
    ("id", "string", "Attached region ID", true),
    ("changes", "object", "Attached region fields to update", true),
])),
td("attached_region_remove", "Remove attached region and clear child memberships", project_props(&[
    ("id", "string", "Attached region ID", true),
])),
td("attached_region_list", "List attached regions", project_props(&[])),
td("attached_region_move_with_elements", "Move attached region and its member elements while keeping absolute coordinates", project_props(&[
    ("id", "string", "Attached region ID", true),
    ("x", "integer", "New region X", true),
    ("y", "integer", "New region Y", true),
])),
```

- [ ] **Step 5: Add MCP handlers**

Add match arms in `execute_tool`:

```rust
"attached_region_add" => attached_region_add(&mut sessions, project_id, args),
"attached_region_update" => attached_region_update(&mut sessions, project_id, args),
"attached_region_remove" => attached_region_remove(&mut sessions, project_id, args),
"attached_region_list" => attached_region_list(&sessions, project_id),
"attached_region_move_with_elements" => attached_region_move_with_elements(&mut sessions, project_id, args),
```

Implement handlers mirroring Tauri command behavior. Parse `AttachedRegion` with `serde_json::from_value`; default `state` to `"static"` and `visible` to true when omitted by inserting them into the object before deserialization.

- [ ] **Step 6: Verify MCP tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_attached_region_tools --locked
cargo test --manifest-path src-tauri/Cargo.toml attached_region_add_and_move_with_elements_mutate_live_session --locked
cargo test --manifest-path src-tauri/Cargo.toml mcp::tests --locked
```

Expected: PASS.

- [ ] **Step 7: Commit MCP tools**

```bash
git add src-tauri/src/mcp/mod.rs
git commit -m "feat: expose attached regions through mcp"
```

## Task 6: Frontend Types, API, And Store

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores/project.svelte.ts`

- [ ] **Step 1: Add TypeScript types**

In `src/lib/types.ts`, add:

```ts
export type AttachedRegionAnchor = "left" | "right" | "top" | "bottom" | "free";
export type AttachedRegionState = "static" | "toggleable";

export interface VisualBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AttachedRegion {
  id: string;
  anchor: AttachedRegionAnchor;
  x: number;
  y: number;
  width: number;
  height: number;
  state: AttachedRegionState;
  kind?: string | null;
  semantic_group?: string | null;
  visible?: boolean;
}
```

Add to `Element`:

```ts
attached_region?: string | null;
```

Add to `ProjectData`:

```ts
attached_regions?: AttachedRegion[];
```

- [ ] **Step 2: Add API wrappers**

In `src/lib/api.ts`, import `AttachedRegion`. Add wrappers:

```ts
export async function attachedRegionCreate(region: AttachedRegion, projectId?: string): Promise<AttachedRegion> {
  const invoke = await getInvoke();
  return invoke("attached_region_create", { region, project_id: projectId }) as Promise<AttachedRegion>;
}

export async function attachedRegionUpdate(id: string, changes: Partial<AttachedRegion>, projectId?: string): Promise<AttachedRegion> {
  const invoke = await getInvoke();
  return invoke("attached_region_update", { id, changes, project_id: projectId }) as Promise<AttachedRegion>;
}

export async function attachedRegionRemove(id: string, projectId?: string): Promise<{ removed: boolean }> {
  const invoke = await getInvoke();
  return invoke("attached_region_remove", { id, project_id: projectId }) as Promise<{ removed: boolean }>;
}

export async function attachedRegionList(projectId?: string): Promise<AttachedRegion[]> {
  const invoke = await getInvoke();
  return invoke("attached_region_list", { project_id: projectId }) as Promise<AttachedRegion[]>;
}

export async function attachedRegionMoveWithElements(id: string, x: number, y: number, projectId?: string): Promise<AttachedRegion> {
  const invoke = await getInvoke();
  return invoke("attached_region_move_with_elements", { id, x, y, project_id: projectId }) as Promise<AttachedRegion>;
}
```

In `mockInvoke`, add these cases:

```ts
case "attached_region_create": {
  const session = mockSession(args?.project_id);
  const region = clone(args?.region as AttachedRegion);
  if (session.project.attached_regions?.some(existing => existing.id === region.id)) {
    throw new Error(`Attached region already exists: ${region.id}`);
  }
  const previous = clone(session.project);
  session.project.attached_regions = [...(session.project.attached_regions ?? []), region];
  markMockChanged(session, previous);
  return clone(region);
}
case "attached_region_update": {
  const session = mockSession(args?.project_id);
  const id = String(args?.id);
  const changes = args?.changes as Partial<AttachedRegion>;
  const previous = clone(session.project);
  let updated: AttachedRegion | null = null;
  session.project.attached_regions = (session.project.attached_regions ?? []).map(region => {
    if (region.id !== id) return region;
    updated = { ...region, ...changes, id };
    return updated;
  });
  if (!updated) throw new Error(`Attached region not found: ${id}`);
  markMockChanged(session, previous);
  return clone(updated);
}
case "attached_region_remove": {
  const session = mockSession(args?.project_id);
  const id = String(args?.id);
  const previous = clone(session.project);
  session.project.attached_regions = (session.project.attached_regions ?? []).filter(region => region.id !== id);
  session.project.elements = session.project.elements.map(element =>
    element.attached_region === id ? { ...element, attached_region: null } : element,
  );
  markMockChanged(session, previous);
  return { removed: true };
}
case "attached_region_list": {
  const session = mockSession(args?.project_id);
  return clone(session.project.attached_regions ?? []);
}
case "attached_region_move_with_elements": {
  const session = mockSession(args?.project_id);
  const id = String(args?.id);
  const x = Number(args?.x);
  const y = Number(args?.y);
  const old = (session.project.attached_regions ?? []).find(region => region.id === id);
  if (!old) throw new Error(`Attached region not found: ${id}`);
  const dx = x - old.x;
  const dy = y - old.y;
  const previous = clone(session.project);
  const updated = { ...old, x, y };
  session.project.attached_regions = (session.project.attached_regions ?? []).map(region =>
    region.id === id ? updated : region,
  );
  session.project.elements = session.project.elements.map(element =>
    element.attached_region === id ? { ...element, x: element.x + dx, y: element.y + dy } : element,
  );
  markMockChanged(session, previous);
  return clone(updated);
}
```

- [ ] **Step 3: Update ProjectStore state**

In `src/lib/stores/project.svelte.ts`, import `AttachedRegion` and `VisualBounds`. Add state:

```ts
attachedRegions = $state<AttachedRegion[]>([]);
```

Add:

```ts
get visualBounds(): VisualBounds {
  let minX = 0;
  let minY = 0;
  let maxX = this.guiSize.width;
  let maxY = this.guiSize.height;
  for (const el of this.elements) {
    if (el.visible === false) continue;
    const w = el.width ?? el.size ?? 16;
    const h = el.height ?? el.size ?? 16;
    minX = Math.min(minX, el.x);
    minY = Math.min(minY, el.y);
    maxX = Math.max(maxX, el.x + w);
    maxY = Math.max(maxY, el.y + h);
  }
  for (const region of this.attachedRegions) {
    if (region.visible === false) continue;
    minX = Math.min(minX, region.x);
    minY = Math.min(minY, region.y);
    maxX = Math.max(maxX, region.x + region.width);
    maxY = Math.max(maxY, region.y + region.height);
  }
  return { x: minX, y: minY, width: Math.max(1, maxX - minX), height: Math.max(1, maxY - minY) };
}
```

Hydrate:

```ts
this.attachedRegions = project.attached_regions ?? [];
```

Clear:

```ts
this.attachedRegions = [];
```

Add helpers:

```ts
attachedRegionById(id: string): AttachedRegion | undefined {
  return this.attachedRegions.find(region => region.id === id);
}

elementsForAttachedRegion(id: string): Element[] {
  return this.elements.filter(element => element.attached_region === id);
}
```

- [ ] **Step 4: Add region mutations to ProjectStore**

Add:

```ts
async createAttachedRegion(region: AttachedRegion): Promise<AttachedRegion> {
  const created = await api.attachedRegionCreate(region, this.activeProjectId ?? undefined);
  this.attachedRegions = [...this.attachedRegions, created];
  this.isDirty = true;
  await this.refreshSessions();
  this.bumpRenderVersion();
  return created;
}

async updateAttachedRegion(id: string, changes: Partial<AttachedRegion>): Promise<AttachedRegion> {
  const updated = await api.attachedRegionUpdate(id, changes, this.activeProjectId ?? undefined);
  this.attachedRegions = this.attachedRegions.map(region => region.id === id ? updated : region);
  this.isDirty = true;
  await this.refreshSessions();
  this.bumpRenderVersion();
  return updated;
}

async removeAttachedRegion(id: string): Promise<void> {
  await api.attachedRegionRemove(id, this.activeProjectId ?? undefined);
  this.attachedRegions = this.attachedRegions.filter(region => region.id !== id);
  this.elements = this.elements.map(element => element.attached_region === id ? { ...element, attached_region: null } : element);
  this.isDirty = true;
  await this.refreshSessions();
  this.bumpRenderVersion();
}

async moveAttachedRegionWithElements(id: string, x: number, y: number): Promise<void> {
  const old = this.attachedRegionById(id);
  if (!old) return;
  const dx = x - old.x;
  const dy = y - old.y;
  const updated = await api.attachedRegionMoveWithElements(id, x, y, this.activeProjectId ?? undefined);
  this.attachedRegions = this.attachedRegions.map(region => region.id === id ? updated : region);
  this.elements = this.elements.map(element => element.attached_region === id ? { ...element, x: element.x + dx, y: element.y + dy } : element);
  this.isDirty = true;
  await this.refreshSessions();
  this.bumpRenderVersion();
}

previewMoveAttachedRegionWithElements(id: string, x: number, y: number): void {
  const old = this.attachedRegionById(id);
  if (!old) return;
  const dx = x - old.x;
  const dy = y - old.y;
  this.attachedRegions = this.attachedRegions.map(region => region.id === id ? { ...region, x, y } : region);
  this.elements = this.elements.map(element => element.attached_region === id ? { ...element, x: element.x + dx, y: element.y + dy } : element);
  this.bumpRenderVersion();
}
```

- [ ] **Step 5: Update movement ids**

In `movementIdsForElement`, if an element belongs to an attached region, include all elements in that same region:

```ts
const regionId = this.elementById(id)?.attached_region;
const regionIds = regionId ? this.elementsForAttachedRegion(regionId).map(element => element.id) : [];
const ids = group ? group.elements : regionIds.length > 0 ? regionIds : [id];
```

- [ ] **Step 6: Verify frontend types**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS.

- [ ] **Step 7: Commit frontend state**

```bash
git add src/lib/types.ts src/lib/api.ts src/lib/stores/project.svelte.ts
git commit -m "feat: add attached region frontend state"
```

## Task 7: Editor Rendering And Region UI

**Files:**
- Modify: `src/lib/stores/editor.svelte.ts`
- Modify: `src/lib/engine/renderer.ts`
- Modify: `src/lib/components/LayerPanel.svelte`
- Modify: `src/lib/components/PropertyPanel.svelte`

- [ ] **Step 1: Add editor region selection state**

In `src/lib/stores/editor.svelte.ts`, add:

```ts
selectedAttachedRegionId = $state<string | null>(null);
regionSelectionRevision = $state(0);

selectAttachedRegion(id: string | null) {
  this.selectedAttachedRegionId = id;
  this.selectedElementId = null;
  this.selectedIds.clear();
  this.regionSelectionRevision += 1;
  this.selectionRevision += 1;
}
```

In existing `selectElement` and `clearSelection`, clear `selectedAttachedRegionId` and increment `regionSelectionRevision` when needed.

- [ ] **Step 2: Draw main and visual bounds**

In `src/lib/engine/renderer.ts`, update grid/background drawing so:

- main bounds remains the existing filled GUI frame at `(0, 0, guiSize.width, guiSize.height)`;
- visual bounds is drawn as a dashed outline using `project.visualBounds`.

Add in `drawGrid()` after main GUI border:

```ts
const visual = project.visualBounds;
if (visual.x !== 0 || visual.y !== 0 || visual.width !== project.guiSize.width || visual.height !== project.guiSize.height) {
  g.rect(visual.x, visual.y, visual.width, visual.height);
  g.stroke({ width: 1, color: 0xd7a339, alpha: 0.8 });
}
```

Use Pixi's available stroke API already used in the file. If dashed outlines are not directly supported, use a solid amber outline for this cycle.

- [ ] **Step 3: Draw attached region outlines and hit-test regions**

Add a `drawAttachedRegions()` helper called before selection handles:

```ts
private drawAttachedRegions() {
  for (const region of project.attachedRegions) {
    if (region.visible === false) continue;
    const g = new Graphics();
    g.rect(region.x, region.y, region.width, region.height);
    g.stroke({
      width: editor.selectedAttachedRegionId === region.id ? 2 : 1,
      color: editor.selectedAttachedRegionId === region.id ? SELECTED_TINT : 0x3f76b5,
      alpha: 0.85,
    });
    this.elementsContainer.addChild(g);
  }
}
```

In pointer down selection, after element hit-test fails and before clearing selection, hit-test visible regions from last to first:

```ts
const clickedRegion = [...project.attachedRegions]
  .reverse()
  .find(region => (region.visible ?? true)
    && gui.x >= region.x
    && gui.y >= region.y
    && gui.x < region.x + region.width
    && gui.y < region.y + region.height);
if (clickedRegion && editor.tool === "select") {
  editor.selectAttachedRegion(clickedRegion.id);
  this.dragStartPositions = new Map(
    project.elementsForAttachedRegion(clickedRegion.id).map(element => [element.id, { x: element.x, y: element.y }]),
  );
  editor.startDragElementAt(clickedRegion.id, pointer.x, pointer.y, clickedRegion.x, clickedRegion.y);
  return;
}
```

Use pointer-up-only backend commits for region dragging. During pointer move, call:

```ts
project.previewMoveAttachedRegionWithElements(selectedRegionId, newDragX, newDragY);
```

On pointer up, call:

```ts
await project.moveAttachedRegionWithElements(selectedRegionId, finalX, finalY);
```

Do not call `moveAttachedRegionWithElements` during pointer move. This keeps drag responsive and records one undo history entry.

- [ ] **Step 4: LayerPanel grouping by attached region**

In `src/lib/components/LayerPanel.svelte`, import `AttachedRegion`. Extend `LayerRow`:

```ts
| { kind: "attached_region"; region: AttachedRegion; meta: string; elements: Element[] }
```

At the start of `groupedRows()`, before semantic groups, add rows for regions with member elements:

```ts
for (const region of project.attachedRegions) {
  const elements = project.elements.filter(element => element.attached_region === region.id);
  if (elements.length > 0) {
    for (const element of elements) consumed.add(element.id);
    rows.push({
      kind: "attached_region",
      region,
      meta: `${region.anchor} · ${region.state} · ${elements.length} elements`,
      elements,
    });
  }
}
```

Render region rows:

```svelte
{:else if row.kind === "attached_region"}
  <div class="group-row">
    <button
      class="group-main"
      class:selected={editor.selectedAttachedRegionId === row.region.id}
      onclick={() => editor.selectAttachedRegion(row.region.id)}
      ondblclick={() => toggleGroup(row.region.id)}
    >
      <span class="disclosure">{collapsedGroups.has(row.region.id) ? "▸" : "▾"}</span>
      <span class="group-text">
        <span class="group-title">{displayId(row.region.id)}</span>
        <span class="group-meta">{row.meta}</span>
      </span>
    </button>
  </div>
  {#if !collapsedGroups.has(row.region.id)}
    {#each row.elements as el (el.id)}
      {@render elementRow(el, true)}
    {/each}
  {/if}
```

- [ ] **Step 5: PropertyPanel region editing**

In `PropertyPanel.svelte`, derive selected region:

```ts
let selectedRegion = $derived.by(() => {
  void editor.regionSelectionRevision;
  return editor.selectedAttachedRegionId ? project.attachedRegionById(editor.selectedAttachedRegionId) : null;
});
```

Add:

```ts
function updateRegion(changes: Partial<AttachedRegion>) {
  if (!selectedRegion) return;
  void project.updateAttachedRegion(selectedRegion.id, changes);
}
```

When `selectedEl`, add a select field:

```svelte
<div class="prop-row">
  <label for="prop-attached-region">Region</label>
  <select
    id="prop-attached-region"
    value={selectedEl.attached_region ?? ""}
    onchange={(event) => updateSelectedElement({ attached_region: event.currentTarget.value || null })}
  >
    <option value="">None</option>
    {#each project.attachedRegions as region (region.id)}
      <option value={region.id}>{region.id}</option>
    {/each}
  </select>
</div>
```

When `selectedRegion`, render region properties instead of element properties:

```svelte
{:else if selectedRegion}
  <div class="props-form">
    <div class="prop-row"><span class="prop-label">Region</span><span class="prop-value mono">{selectedRegion.id}</span></div>
    <div class="prop-row">
      <label for="region-anchor">Anchor</label>
      <select id="region-anchor" value={selectedRegion.anchor} onchange={(event) => updateRegion({ anchor: event.currentTarget.value as AttachedRegionAnchor })}>
        <option value="left">Left</option>
        <option value="right">Right</option>
        <option value="top">Top</option>
        <option value="bottom">Bottom</option>
        <option value="free">Free</option>
      </select>
    </div>
    <div class="prop-row">
      <label for="region-x">X</label>
      <input id="region-x" type="number" value={selectedRegion.x} oninput={(event) => updateRegion({ x: numberValue(event.currentTarget.value, selectedRegion.x) })} />
    </div>
    <div class="prop-row">
      <label for="region-y">Y</label>
      <input id="region-y" type="number" value={selectedRegion.y} oninput={(event) => updateRegion({ y: numberValue(event.currentTarget.value, selectedRegion.y) })} />
    </div>
    <div class="prop-row">
      <label for="region-width">Width</label>
      <input id="region-width" type="number" min="1" value={selectedRegion.width} oninput={(event) => updateRegion({ width: Math.max(1, numberValue(event.currentTarget.value, selectedRegion.width)) })} />
    </div>
    <div class="prop-row">
      <label for="region-height">Height</label>
      <input id="region-height" type="number" min="1" value={selectedRegion.height} oninput={(event) => updateRegion({ height: Math.max(1, numberValue(event.currentTarget.value, selectedRegion.height)) })} />
    </div>
    <div class="prop-row">
      <label for="region-state">State</label>
      <select id="region-state" value={selectedRegion.state} onchange={(event) => updateRegion({ state: event.currentTarget.value as AttachedRegionState })}>
        <option value="static">Static</option>
        <option value="toggleable">Toggleable</option>
      </select>
    </div>
    <div class="prop-row">
      <label for="region-kind">Kind</label>
      <input id="region-kind" value={selectedRegion.kind ?? ""} oninput={(event) => updateRegion({ kind: optionalText(event.currentTarget.value) })} />
    </div>
    <div class="prop-row">
      <label for="region-semantic-group">Semantic</label>
      <input id="region-semantic-group" value={selectedRegion.semantic_group ?? ""} oninput={(event) => updateRegion({ semantic_group: optionalText(event.currentTarget.value) })} />
    </div>
  </div>
```

- [ ] **Step 6: Verify editor checks**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS.

- [ ] **Step 7: Commit editor UI**

```bash
git add src/lib/stores/editor.svelte.ts src/lib/engine/renderer.ts src/lib/components/LayerPanel.svelte src/lib/components/PropertyPanel.svelte
git commit -m "feat: edit attached regions in canvas"
```

## Task 8: Documentation, Skill, Roadmap, And Final Verification

**Files:**
- Modify: `docs/mcp.md`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`
- Modify: `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update MCP docs**

In `docs/mcp.md`, add a section after slot grids:

````markdown
### Attached Regions

Use attached regions when a GUI has visible or interactive elements outside the
main `gui_size` rectangle: side toggles, upgrade panels, return pockets, or
decorative flair. Coordinates remain absolute relative to the main GUI origin.

1. Create the region:

```json
{
  "id": "returns_pocket",
  "anchor": "right",
  "x": 176,
  "y": 18,
  "width": 54,
  "height": 72,
  "state": "static",
  "kind": "returns_pocket",
  "semantic_group": "food_returns"
}
```

2. Add elements using normal absolute coordinates and set `attached_region`:

```json
{
  "id": "returns_0",
  "type": "slot",
  "x": 184,
  "y": 26,
  "size": 18,
  "slot_role": "machine",
  "inventory_group": "food_returns",
  "attached_region": "returns_pocket"
}
```

3. Add semantic groups separately. The region describes geometry; semantic
groups describe runtime meaning.

`state: "toggleable"` is preserved as metadata, but generated runtime open/close
behavior is deferred to the toggleable attached-region roadmap item. Use
`static` for fully supported exports today.
````

- [ ] **Step 2: Update agent skill**

In `.agents/skills/mc-gui-crafter/SKILL.md`, add a concise rule:

```markdown
## Attached Regions

When creating side panels, module pockets, return-slot pockets, or flair outside
the main GUI rectangle, create an attached region first. Keep child element
coordinates absolute relative to the main GUI origin and set each child's
`attached_region`. Use semantic groups to describe slot/button meaning; the
region only describes geometry and anchoring. Prefer `state: "static"` until
toggleable runtime support is implemented.
```

In `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`, add:

````markdown
### Attached Region Workflow

Use attached regions for GUI parts outside the main `gui_size` bounds:
side module panels, return pockets, upgrade panes, floating toggles, and flair.
Coordinates stay absolute relative to the main GUI origin.

1. Call `attached_region_add` with `id`, `anchor`, `x`, `y`, `width`,
   `height`, `state: "static"`, and optional `kind` / `semantic_group`.
2. Add child elements with normal absolute `x` / `y` coordinates and set
   `attached_region` to the region id.
3. Use semantic groups to describe the meaning of slots/buttons inside the
   region. The region itself only describes geometry and anchoring.
4. Prefer `state: "static"` for generated exports. `toggleable` is preserved as
   metadata until runtime open/closed behavior is implemented.
5. Use `attached_region_move_with_elements` when repositioning the region after
   adding children.
````

- [ ] **Step 3: Update roadmap**

In `docs/roadmap.md`, under Phase 6.x / Phase 7 Candidates, add:

```markdown
- [x] Static attached regions: active outside-GUI flair, side panels, return pockets, computed visual bounds, and MCP authoring
- [ ] Toggleable attached-region runtime: open/closed state, click bindings, animation, conditional slot activation, and modular generated helpers
```

- [ ] **Step 4: Commit docs**

```bash
git add docs/mcp.md .agents/skills/mc-gui-crafter/SKILL.md .agents/skills/mc-gui-crafter/references/mcp-workflows.md docs/roadmap.md
git commit -m "docs: describe attached region workflows"
```

- [ ] **Step 5: Run full automated verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm check
pnpm build
git diff --check
git status --short
```

Expected:

- Rust tests pass;
- Svelte check reports 0 errors and 0 warnings;
- Vite build passes; existing large chunk warning is acceptable;
- no whitespace errors;
- working tree is clean except `.superpowers/` if visual companion artifacts remain untracked.

- [ ] **Step 6: Desktop smoke**

Start dev mode:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11 pnpm tauri dev
```

Smoke:

- create/open a GUI;
- create a right attached region;
- add a slot/button with negative or outside-main coordinates;
- confirm the visual bounds frame expands;
- select the outside element and region in Layers;
- edit region properties;
- move the region and confirm children keep absolute coordinates;
- export preview and verify layout JSON contains `visual_bounds`, `visual_offset_x`, `visual_offset_y`, and `attached_regions`.

Expected: no console/runtime errors; outside elements remain visible and selectable.

## Self-Review Notes

- Spec coverage: model, visual bounds, export/runtime contract, MCP, editor behavior, migration, docs, and roadmap are covered by Tasks 1-8.
- Scope: full `toggleable` open/closed runtime is intentionally deferred and documented as a future roadmap item.
- Compatibility: existing projects default to `attached_regions: []`; main-size exports remain unchanged when no outside bounds exist.
