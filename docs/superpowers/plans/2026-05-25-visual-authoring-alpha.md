# Visual Authoring Alpha Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add nine-slice texture authoring, shared nine-slice rendering, atlas-backed UV/icon selection, and MCP support for the same metadata.

**Architecture:** Extend the existing `Element` texture model and project asset model instead of adding a separate background element type. Use one Rust nine-slice compositor for export, MCP `project_render`, and backend validation; use the existing Pixi/Svelte property panel and `UvEditorDialog.svelte` as the frontend authoring surface. Keep binary image payloads opt-in where possible so the Gemini asset sync concern does not get worse as asset metadata grows.

**Tech Stack:** Rust/Tauri 2 backend, Serde `.mcgui` format, `image` crate compositing, Svelte 5 runes, PixiJS 8 editor rendering, JSON-RPC MCP.

---

## Current Context

- `src-tauri/src/project/mod.rs` already has `UvRect`, `Element.asset`, `Element.uv`, `Element.icon`, and `Element.icon_uv`, but no asset metadata, `render_mode`, or `nine_slice` fields.
- `src/lib/types.ts` mirrors those element fields and needs the same model additions.
- `src-tauri/src/texture/mod.rs` currently renders texture assets by nearest-neighbor resizing a cropped source rectangle. Nine-slice rendering belongs beside `overlay_texture_data()` so export and `project_render` share it.
- `src/lib/components/UvEditorDialog.svelte` already provides a pixel-friendly UV picker. Extend it into a reusable visual region/guide editor instead of creating a separate modal from scratch.
- `GEMINI_CODE_REVIEW.md` flags `asset_list` returning full `data_url` payloads and the renderer `textTextureCache` growing indefinitely. This plan addresses the asset payload issue directly and includes a bounded cleanup task for renderer caches.

## File Structure

- Modify `src-tauri/src/project/mod.rs`: add `AssetMetadata`, `NineSlice`, `NineSliceMode`, `TextureRenderMode`, element fields, project metadata map, defaults, and serialization tests.
- Modify `src/lib/types.ts`: add matching TypeScript types and project-facing asset metadata contracts.
- Modify `src-tauri/src/commands.rs`: return compact asset metadata from `asset_list`, add `asset_metadata_update`, and keep `asset_get_data_url` as the image payload path.
- Modify `src/lib/api.ts`: update asset result types, mock metadata storage, and add `assetMetadataUpdate()`.
- Modify `src/lib/stores/project.svelte.ts`: track `assetMetadata`, hydrate data URLs only when needed, and expose metadata update helpers.
- Modify `src-tauri/src/texture/mod.rs`: add guide resolution and nine-slice tiling/stretch compositing.
- Modify `src-tauri/src/export/mod.rs`: validate nine-slice and UV/icon bounds in preview/export and include metadata in layout output.
- Modify `src-tauri/src/mcp/mod.rs`: expose schema discovery fields and an `asset_metadata_update` MCP tool; ensure `element_update_many` accepts visual fields.
- Modify `src/lib/engine/renderer.ts`: render nine-slice texture elements in Pixi and bound text/glyph cache growth.
- Modify `src/lib/components/UvEditorDialog.svelte`: support `mode: "uv" | "nine_slice"` with draggable guides and target-size preview.
- Modify `src/lib/components/PropertyPanel.svelte`: add render mode, element-level guide override, and atlas icon controls.
- Modify `src/lib/components/AssetLibrary.svelte`: add asset-level guide editing entry point.
- Modify `docs/mcp.md`, `.agents/skills/mc-gui-crafter/SKILL.md`, and `docs/roadmap.md`: document the alpha workflow.

## Task 1: Project Model And Asset Metadata

**Files:**
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src/lib/types.ts`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores/project.svelte.ts`

- [ ] **Step 1: Add failing Rust serialization tests**

Add tests in `src-tauri/src/project/mod.rs`:

```rust
#[test]
fn asset_metadata_round_trips_nine_slice_defaults() {
    let json = serde_json::json!({
        "name": "Meta",
        "gui_size": { "width": 176, "height": 166 },
        "mod_target": "forge",
        "elements": [],
        "groups": [],
        "animations": [],
        "assets": ["textures/gui/panel_atlas.png"],
        "asset_metadata": {
            "textures/gui/panel_atlas.png": {
                "width": 64,
                "height": 64,
                "nine_slice": {
                    "left": 4,
                    "right": 4,
                    "top": 4,
                    "bottom": 4,
                    "edge_mode": "tile",
                    "center_mode": "tile"
                }
            }
        }
    });

    let project: Project = serde_json::from_value(json).unwrap();
    let metadata = project.asset_metadata.get("textures/gui/panel_atlas.png").unwrap();
    assert_eq!(metadata.width, Some(64));
    assert_eq!(metadata.height, Some(64));
    assert_eq!(metadata.nine_slice.as_ref().unwrap().left, 4);
    assert_eq!(metadata.nine_slice.as_ref().unwrap().edge_mode, NineSliceMode::Tile);
}

#[test]
fn texture_element_round_trips_nine_slice_render_mode() {
    let json = serde_json::json!({
        "id": "background",
        "type": "texture",
        "x": 0,
        "y": 0,
        "width": 176,
        "height": 166,
        "asset": "textures/gui/panel_atlas.png",
        "render_mode": "nine_slice",
        "nine_slice": {
            "left": 4,
            "right": 4,
            "top": 4,
            "bottom": 4,
            "edge_mode": "tile",
            "center_mode": "tile"
        }
    });

    let element: Element = serde_json::from_value(json).unwrap();
    assert_eq!(element.render_mode, TextureRenderMode::NineSlice);
    assert_eq!(element.nine_slice.as_ref().unwrap().center_mode, NineSliceMode::Tile);
    assert_eq!(
        serde_json::to_value(&element).unwrap()["render_mode"],
        serde_json::json!("nine_slice")
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml asset_metadata_round_trips_nine_slice_defaults texture_element_round_trips_nine_slice_render_mode
```

Expected: FAIL with missing `asset_metadata`, `NineSliceMode`, `TextureRenderMode`, or `nine_slice` fields.

- [ ] **Step 3: Add Rust model types**

In `src-tauri/src/project/mod.rs`, add after `UvRect`:

```rust
iterable_enum! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum NineSliceMode {
        Tile,
        Stretch,
    }
}

fn default_nine_slice_mode() -> NineSliceMode {
    NineSliceMode::Tile
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NineSlice {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
    #[serde(default = "default_nine_slice_mode")]
    pub edge_mode: NineSliceMode,
    #[serde(default = "default_nine_slice_mode")]
    pub center_mode: NineSliceMode,
}

iterable_enum! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
    #[serde(rename_all = "snake_case")]
    pub enum TextureRenderMode {
        #[default]
        Plain,
        NineSlice,
    }
}

fn is_plain_render_mode(mode: &TextureRenderMode) -> bool {
    *mode == TextureRenderMode::Plain
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nine_slice: Option<NineSlice>,
}
```

Extend `Element`:

```rust
    #[serde(default, skip_serializing_if = "is_plain_render_mode")]
    pub render_mode: TextureRenderMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nine_slice: Option<NineSlice>,
```

Extend `Project`:

```rust
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub asset_metadata: HashMap<String, AssetMetadata>,
```

Update every `Element` and `Project` literal in tests/templates with:

```rust
render_mode: TextureRenderMode::Plain,
nine_slice: None,
```

and:

```rust
asset_metadata: HashMap::new(),
```

- [ ] **Step 4: Add TypeScript model types**

In `src/lib/types.ts`, add:

```ts
export type NineSliceMode = "tile" | "stretch";
export type TextureRenderMode = "plain" | "nine_slice";

export interface NineSlice {
  left: number;
  right: number;
  top: number;
  bottom: number;
  edge_mode: NineSliceMode;
  center_mode: NineSliceMode;
}

export interface AssetMetadata {
  width?: number | null;
  height?: number | null;
  nine_slice?: NineSlice | null;
}
```

Extend `Element`:

```ts
render_mode?: TextureRenderMode;
nine_slice?: NineSlice | null;
```

Extend active project payload/project types where present:

```ts
asset_metadata?: Record<string, AssetMetadata>;
```

- [ ] **Step 5: Make asset lists compact and add metadata update API**

In `src-tauri/src/commands.rs`, change `asset_list` to return objects with `name`, `width`, `height`, `bytes`, `sha256`, and `nine_slice`, without `data_url`. Keep `asset_get_data_url` unchanged.

Add:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn asset_metadata_update(
    state: State<'_, AppState>,
    name: String,
    metadata: AssetMetadata,
    project_id: Option<String>,
) -> Result<AssetMetadata, String> {
    let mut sessions = state.sessions.lock().unwrap();
    let project_id = project_id.as_deref();
    if !sessions.resolve(project_id)?.project.assets.iter().any(|asset| asset == &name) {
        return Err(format!("Asset not found: {name}"));
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.asset_metadata.insert(name, metadata.clone());
    sessions.mark_changed(project_id)?;
    Ok(metadata)
}
```

Register the command in `src-tauri/src/lib.rs`.

In `src/lib/api.ts`, add:

```ts
export interface AssetImportResult {
  name: string;
  width: number;
  height: number;
  bytes: number;
  sha256: string;
  data_url?: string;
  nine_slice?: NineSlice | null;
}

export async function assetMetadataUpdate(name: string, metadata: AssetMetadata, projectId?: string): Promise<AssetMetadata> {
  return invoke("asset_metadata_update", { name, metadata, project_id: projectId }) as Promise<AssetMetadata>;
}
```

Update mock asset storage with a `Map<string, AssetMetadata>` per session, and make mock `asset_get_data_url` the only mock command that returns full data URLs.

- [ ] **Step 6: Hydrate asset previews on demand**

In `src/lib/stores/project.svelte.ts`, add:

```ts
assetMetadata = $state<Record<string, AssetMetadata>>({});

async ensureAssetDataUrl(name: string): Promise<string | undefined> {
  const cached = assetDataUrls.get(name);
  if (cached) return cached;
  if (!this.activeProjectId) return undefined;
  const dataUrl = await api.assetGetDataUrl(name, this.activeProjectId);
  assetDataUrls.set(name, dataUrl);
  this.bumpRenderVersion();
  return dataUrl;
}

async updateAssetMetadata(name: string, metadata: AssetMetadata): Promise<AssetMetadata> {
  const updated = await api.assetMetadataUpdate(name, metadata, this.activeProjectId ?? undefined);
  this.assetMetadata = { ...this.assetMetadata, [name]: updated };
  this.isDirty = true;
  await this.refreshSessions();
  this.bumpRenderVersion();
  return updated;
}
```

Update project hydration so `assetMetadata = payload.asset_metadata ?? {}` and asset list results merge `nine_slice`, `width`, and `height` into that map.

- [ ] **Step 7: Run model/API checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml asset_metadata texture_element_round_trips_nine_slice_render_mode asset_list
pnpm check
```

Expected: Rust tests pass and `svelte-check` reports 0 errors.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/project/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src/lib/types.ts src/lib/api.ts src/lib/stores/project.svelte.ts
git commit -m "feat: add visual asset metadata model"
```

## Task 2: Shared Nine-Slice Compositor

**Files:**
- Modify: `src-tauri/src/texture/mod.rs`
- Test: `src-tauri/src/texture/mod.rs`

- [ ] **Step 1: Add failing compositor tests**

Add tests in `src-tauri/src/texture/mod.rs`:

```rust
#[test]
fn nine_slice_uses_element_override_before_asset_metadata() {
    let mut project = Project::new("Nine Slice", 32, 32, ModTarget::Forge);
    let asset = "textures/gui/panel_atlas.png";
    project.assets.push(asset.into());
    project.texture_data.insert(asset.into(), fixture_panel_atlas());
    project.asset_metadata.insert(asset.into(), AssetMetadata {
        width: Some(8),
        height: Some(8),
        nine_slice: Some(NineSlice { left: 1, right: 1, top: 1, bottom: 1, edge_mode: NineSliceMode::Tile, center_mode: NineSliceMode::Tile }),
    });
    project.elements.push(Element {
        id: "panel".into(),
        element_type: ElementType::Texture,
        x: 0,
        y: 0,
        width: Some(16),
        height: Some(16),
        asset: Some(asset.into()),
        render_mode: TextureRenderMode::NineSlice,
        nine_slice: Some(NineSlice { left: 2, right: 2, top: 2, bottom: 2, edge_mode: NineSliceMode::Tile, center_mode: NineSliceMode::Tile }),
        ..test_element_defaults()
    });

    let png = composite_project_preview(&project).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(2, 2).0, [0, 255, 0, 255]);
}

#[test]
fn nine_slice_rejects_overlapping_guides() {
    let guides = NineSlice { left: 5, right: 5, top: 1, bottom: 1, edge_mode: NineSliceMode::Tile, center_mode: NineSliceMode::Tile };
    let err = validate_nine_slice_guides(&guides, 8, 8).unwrap_err();
    assert!(err.contains("leave no center region"));
}
```

Use local test helpers:

```rust
fn test_element_defaults() -> Element {
    Element {
        id: String::new(),
        element_type: ElementType::Texture,
        x: 0,
        y: 0,
        width: None,
        height: None,
        size: None,
        asset: None,
        icon: None,
        icon_uv: None,
        tooltip: None,
        direction: None,
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: None,
        visible: true,
        uv: None,
        layer: Layer::Background,
        slot_role: None,
        slot_index: None,
        inventory_group: None,
        scroll_binding: None,
        scroll_min: None,
        scroll_max: None,
        visible_rows: None,
        total_rows: None,
        columns: None,
        target_group: None,
        binding: None,
        dock: None,
        open_width: None,
        open_height: None,
        attached_region: None,
        render_mode: TextureRenderMode::Plain,
        nine_slice: None,
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml nine_slice
```

Expected: FAIL because nine-slice rendering helpers do not exist.

- [ ] **Step 3: Implement guide resolution**

In `src-tauri/src/texture/mod.rs`, import the new types and add:

```rust
fn resolved_nine_slice<'a>(project: &'a Project, element: &'a Element) -> Option<&'a NineSlice> {
    element.nine_slice.as_ref().or_else(|| {
        let asset = element.asset.as_ref()?;
        project.asset_metadata.get(asset)?.nine_slice.as_ref()
    })
}

fn validate_nine_slice_guides(guides: &NineSlice, source_width: u32, source_height: u32) -> Result<(), String> {
    if guides.left + guides.right >= source_width || guides.top + guides.bottom >= source_height {
        return Err("Nine-slice guides leave no center region".to_string());
    }
    Ok(())
}
```

- [ ] **Step 4: Implement tiled and stretched patch rendering**

Add helpers:

```rust
fn copy_region_tiled(target: &mut RgbaImage, source: &RgbaImage, source_rect: image::math::Rect, target_rect: image::math::Rect) {
    if source_rect.width == 0 || source_rect.height == 0 || target_rect.width == 0 || target_rect.height == 0 {
        return;
    }
    let tile = source.view(source_rect.x, source_rect.y, source_rect.width, source_rect.height).to_image();
    let end_x = target_rect.x + target_rect.width;
    let end_y = target_rect.y + target_rect.height;
    let mut y = target_rect.y;
    while y < end_y {
        let mut x = target_rect.x;
        while x < end_x {
            let width = tile.width().min(end_x - x);
            let height = tile.height().min(end_y - y);
            let cropped = tile.view(0, 0, width, height).to_image();
            image::imageops::overlay(target, &cropped, i64::from(x), i64::from(y));
            x += width;
        }
        y += tile.height().min(end_y - y).max(1);
    }
}

fn copy_region_scaled(target: &mut RgbaImage, source: &RgbaImage, source_rect: image::math::Rect, target_rect: image::math::Rect, mode: NineSliceMode) {
    if mode == NineSliceMode::Tile {
        copy_region_tiled(target, source, source_rect, target_rect);
        return;
    }
    if source_rect.width == 0 || source_rect.height == 0 || target_rect.width == 0 || target_rect.height == 0 {
        return;
    }
    let patch = source.view(source_rect.x, source_rect.y, source_rect.width, source_rect.height).to_image();
    let resized = image::imageops::resize(&patch, target_rect.width, target_rect.height, image::imageops::FilterType::Nearest);
    image::imageops::overlay(target, &resized, i64::from(target_rect.x), i64::from(target_rect.y));
}
```

Then add:

```rust
fn render_nine_slice(source: &RgbaImage, guides: &NineSlice, width: u32, height: u32) -> Result<RgbaImage, String> {
    validate_nine_slice_guides(guides, source.width(), source.height())?;
    if width <= guides.left + guides.right || height <= guides.top + guides.bottom {
        return Err("Nine-slice target is smaller than fixed corner sizes".to_string());
    }
    let mut output = RgbaImage::new(width, height);
    let source_mid_w = source.width() - guides.left - guides.right;
    let source_mid_h = source.height() - guides.top - guides.bottom;
    let target_mid_w = width - guides.left - guides.right;
    let target_mid_h = height - guides.top - guides.bottom;

    let source_rects = [
        image::math::Rect { x: 0, y: 0, width: guides.left, height: guides.top },
        image::math::Rect { x: guides.left, y: 0, width: source_mid_w, height: guides.top },
        image::math::Rect { x: source.width() - guides.right, y: 0, width: guides.right, height: guides.top },
        image::math::Rect { x: 0, y: guides.top, width: guides.left, height: source_mid_h },
        image::math::Rect { x: guides.left, y: guides.top, width: source_mid_w, height: source_mid_h },
        image::math::Rect { x: source.width() - guides.right, y: guides.top, width: guides.right, height: source_mid_h },
        image::math::Rect { x: 0, y: source.height() - guides.bottom, width: guides.left, height: guides.bottom },
        image::math::Rect { x: guides.left, y: source.height() - guides.bottom, width: source_mid_w, height: guides.bottom },
        image::math::Rect { x: source.width() - guides.right, y: source.height() - guides.bottom, width: guides.right, height: guides.bottom },
    ];
    let target_rects = [
        image::math::Rect { x: 0, y: 0, width: guides.left, height: guides.top },
        image::math::Rect { x: guides.left, y: 0, width: target_mid_w, height: guides.top },
        image::math::Rect { x: width - guides.right, y: 0, width: guides.right, height: guides.top },
        image::math::Rect { x: 0, y: guides.top, width: guides.left, height: target_mid_h },
        image::math::Rect { x: guides.left, y: guides.top, width: target_mid_w, height: target_mid_h },
        image::math::Rect { x: width - guides.right, y: guides.top, width: guides.right, height: target_mid_h },
        image::math::Rect { x: 0, y: height - guides.bottom, width: guides.left, height: guides.bottom },
        image::math::Rect { x: guides.left, y: height - guides.bottom, width: target_mid_w, height: guides.bottom },
        image::math::Rect { x: width - guides.right, y: height - guides.bottom, width: guides.right, height: guides.bottom },
    ];
    for index in [0, 2, 6, 8] {
        copy_region_scaled(&mut output, source, source_rects[index], target_rects[index], NineSliceMode::Stretch);
    }
    for index in [1, 3, 5, 7] {
        copy_region_scaled(&mut output, source, source_rects[index], target_rects[index], guides.edge_mode);
    }
    copy_region_scaled(&mut output, source, source_rects[4], target_rects[4], guides.center_mode);
    Ok(output)
}
```

- [ ] **Step 5: Use nine-slice in texture compositing**

In `overlay_texture_data()`, after loading and cropping the source:

```rust
if element.element_type == ElementType::Texture && element.render_mode == TextureRenderMode::NineSlice {
    let guides = resolved_nine_slice(project, element)
        .ok_or_else(|| format!("Texture element '{}' uses nine_slice without guides", element.id))?;
    let rendered = render_nine_slice(&source, guides, tw, th)?;
    image::imageops::overlay(img, &rendered, i64::from(element.x) - i64::from(offset_x), i64::from(element.y) - i64::from(offset_y));
    return Ok(());
}
```

Pass `project` into `overlay_texture_data()` callers that need guide resolution.

- [ ] **Step 6: Run compositor tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml nine_slice
cargo test --manifest-path src-tauri/Cargo.toml texture
```

Expected: all texture and nine-slice tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/texture/mod.rs
git commit -m "feat: render nine-slice textures"
```

## Task 3: Validation, Export, And MCP Support

**Files:**
- Modify: `src-tauri/src/export/mod.rs`
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `docs/mcp.md`

- [ ] **Step 1: Add failing validation and MCP tests**

Add export tests for:

```rust
#[test]
fn preview_warns_for_nine_slice_texture_without_guides() {
    let output_dir = TempExportDir::new("missing-nine-slice-guides");
    let mut project = Project::new("Missing Guides", 32, 32, ModTarget::Forge);
    project.assets.push("textures/gui/panel.png".into());
    project.texture_data.insert("textures/gui/panel.png".into(), png_bytes([120, 120, 120, 255]));
    project.elements.push(Element {
        id: "background".into(),
        element_type: ElementType::Texture,
        x: 0,
        y: 0,
        width: Some(32),
        height: Some(32),
        asset: Some("textures/gui/panel.png".into()),
        render_mode: TextureRenderMode::NineSlice,
        ..button_element("defaults", ElementType::Texture, 0, 0, None)
    });

    let preview = preview_export(&project, export_request(output_dir.path())).unwrap();
    assert!(preview.warnings.iter().any(|warning| {
        warning.contains("background") && warning.contains("nine_slice")
    }), "missing guide warning should name the element: {:?}", preview.warnings);
}

#[test]
fn preview_warns_for_icon_uv_outside_asset_bounds() {
    let output_dir = TempExportDir::new("icon-uv-bounds");
    let mut project = Project::new("Icon UV", 32, 32, ModTarget::Forge);
    project.assets.push("textures/icons/buttons.png".into());
    project.texture_data.insert("textures/icons/buttons.png".into(), png_bytes([10, 200, 10, 255]));
    project.elements.push(Element {
        icon: Some("textures/icons/buttons.png".into()),
        icon_uv: Some(UvRect { x: 16, y: 16, width: 16, height: 16 }),
        ..button_element("settings_button", ElementType::Button, 4, 4, Some("Settings"))
    });

    let preview = preview_export(&project, export_request(output_dir.path())).unwrap();
    assert!(preview.warnings.iter().any(|warning| {
        warning.contains("settings_button") && warning.contains("icon_uv")
    }), "invalid icon_uv warning should name the button: {:?}", preview.warnings);
}

#[test]
fn export_composes_nine_slice_background_into_gui_texture() {
    let output_dir = TempExportDir::new("nine-slice-export");
    let mut project = Project::new("Nine Slice Export", 24, 24, ModTarget::Forge);
    let asset = "textures/gui/panel_atlas.png";
    project.assets.push(asset.into());
    project.texture_data.insert(asset.into(), fixture_panel_atlas());
    project.asset_metadata.insert(asset.into(), AssetMetadata {
        width: Some(8),
        height: Some(8),
        nine_slice: Some(NineSlice { left: 2, right: 2, top: 2, bottom: 2, edge_mode: NineSliceMode::Tile, center_mode: NineSliceMode::Tile }),
    });
    project.elements.push(Element {
        id: "background".into(),
        element_type: ElementType::Texture,
        x: 0,
        y: 0,
        width: Some(24),
        height: Some(24),
        asset: Some(asset.into()),
        render_mode: TextureRenderMode::NineSlice,
        ..button_element("defaults", ElementType::Texture, 0, 0, None)
    });

    export_project(&project, export_request(output_dir.path())).unwrap();
    let gui_png = output_dir.path().join("src/main/resources/assets/testmod/textures/gui/ninesliceexport_gui.png");
    let image = image::open(gui_png).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (24, 24));
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(12, 12).0, [0, 255, 0, 255]);
}
```

Add MCP tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn schema_discover_lists_visual_authoring_fields() {
    let value = tool_text_value(&response_for(schema_discover_call(), &test_state()));
    assert!(value["texture_render_modes"].as_array().unwrap().contains(&serde_json::json!("nine_slice")));
    assert!(value["nine_slice_modes"].as_array().unwrap().contains(&serde_json::json!("tile")));
    assert!(value["editable_element_fields"].as_array().unwrap().contains(&serde_json::json!("render_mode")));
    assert!(value["editable_element_fields"].as_array().unwrap().contains(&serde_json::json!("nine_slice")));
}

#[test]
fn asset_metadata_update_sets_nine_slice_metadata() {
    let state = test_state();
    let project_id = project_with_asset(&state, "textures/gui/panel_atlas.png");
    let response = response_for(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "asset-metadata",
        "method": "tools/call",
        "params": {
            "name": "asset_metadata_update",
            "arguments": {
                "project_id": project_id,
                "name": "textures/gui/panel_atlas.png",
                "metadata": {
                    "nine_slice": { "left": 4, "right": 4, "top": 4, "bottom": 4, "edge_mode": "tile", "center_mode": "tile" }
                }
            }
        }
    }), &state);
    assert!(response["error"].is_null(), "{response:#}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml nine_slice icon_uv schema_discover_lists_visual_authoring_fields asset_metadata_update_sets_nine_slice_metadata
```

Expected: FAIL for missing warnings or MCP tool fields.

- [ ] **Step 3: Add export preview validation**

In `src-tauri/src/export/mod.rs`, add validation that:

- `render_mode: "nine_slice"` requires element or asset guides.
- guides must leave positive source center width and height.
- target width and height must exceed fixed corner widths and heights.
- `uv` and `icon_uv` rectangles must fit inside the resolved asset dimensions.
- missing/invalid data produces warnings unless the element cannot be rendered, in which case export returns an error from the compositor.

Use warning strings with element ids:

```rust
warnings.push(format!(
    "Texture element '{}' uses nine_slice but no element or asset nine_slice guides are defined",
    element.id
));
```

- [ ] **Step 4: Add MCP tool schema and routing**

In `get_tool_definitions()` add `asset_metadata_update` with required `name` and `metadata` fields.

In `execute_tool()` route:

```rust
"asset_metadata_update" => asset_metadata_update(&mut sessions, project_id, args),
```

Implement:

```rust
fn asset_metadata_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = required_str(args, "name")?;
    let metadata: AssetMetadata = serde_json::from_value(required_value(args, "metadata")?.clone())
        .map_err(|error| format!("Invalid asset metadata: {error}"))?;
    if !sessions.resolve(project_id)?.project.assets.iter().any(|asset| asset == name) {
        return Err(format!("Asset not found: {name}"));
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.asset_metadata.insert(name.to_string(), metadata.clone());
    let session_id = session.id.clone();
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({
        "project_id": session_id,
        "name": name,
        "metadata": metadata
    }))
}
```

Update `schema_discover()`:

```rust
"texture_render_modes": ["plain", "nine_slice"],
"nine_slice_modes": ["tile", "stretch"],
"editable_element_fields": [
    "x",
    "y",
    "width",
    "height",
    "size",
    "asset",
    "uv",
    "icon",
    "icon_uv",
    "tooltip",
    "render_mode",
    "nine_slice"
],
"asset_metadata_fields": ["width", "height", "nine_slice"]
```

- [ ] **Step 5: Document MCP visual authoring**

In `docs/mcp.md`, add a `Visual authoring alpha` section with examples for:

- `asset_metadata_update` setting asset-level guides.
- `element_update_many` setting `render_mode: "nine_slice"`.
- `project_render` verification after guide changes.
- compact response rule: binary image payloads remain opt-in.

- [ ] **Step 6: Run backend checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml nine_slice icon_uv asset_metadata_update schema_discover
cargo test --manifest-path src-tauri/Cargo.toml export
cargo test --manifest-path src-tauri/Cargo.toml mcp
```

Expected: all listed suites pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/export/mod.rs src-tauri/src/mcp/mod.rs docs/mcp.md
git commit -m "feat: expose visual authoring metadata through mcp"
```

## Task 4: Frontend Guide Editor And Property Controls

**Files:**
- Modify: `src/lib/components/UvEditorDialog.svelte`
- Modify: `src/lib/components/PropertyPanel.svelte`
- Modify: `src/lib/components/AssetLibrary.svelte`
- Modify: `src/lib/stores/project.svelte.ts`

- [ ] **Step 1: Add Svelte-facing type and helper checks**

Run the Svelte docs/autofixer tools before editing:

```bash
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/UvEditorDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/PropertyPanel.svelte --svelte-version 5
```

Expected: current files either pass or report issues to preserve while editing.

- [ ] **Step 2: Extend `UvEditorDialog` props**

Change props to:

```ts
type EditorMode = "uv" | "nine_slice";

let {
  title,
  mode = "uv",
  assets,
  asset,
  uv = null,
  nineSlice = null,
  targetSize = null,
  onapply,
  onclear,
  onclose,
}: {
  title: string;
  mode?: EditorMode;
  assets: string[];
  asset: string | null;
  uv?: UvRect | null;
  nineSlice?: NineSlice | null;
  targetSize?: Size | null;
  onapply: (asset: string, value: UvRect | NineSlice | null) => void;
  onclear: () => void;
  onclose: () => void;
} = $props();
```

Add guide state:

```ts
let guides = $state<NineSlice>({
  left: nineSlice?.left ?? 4,
  right: nineSlice?.right ?? 4,
  top: nineSlice?.top ?? 4,
  bottom: nineSlice?.bottom ?? 4,
  edge_mode: nineSlice?.edge_mode ?? "tile",
  center_mode: nineSlice?.center_mode ?? "tile",
});
```

- [ ] **Step 3: Add draggable guide behavior**

Add guide dragging functions:

```ts
type GuideHandle = "left" | "right" | "top" | "bottom";
let draggingGuide = $state<GuideHandle | null>(null);

function clampGuides(next: NineSlice): NineSlice {
  const left = Math.max(0, Math.min(imageNaturalWidth - next.right - 1, Math.round(next.left)));
  const right = Math.max(0, Math.min(imageNaturalWidth - left - 1, Math.round(next.right)));
  const top = Math.max(0, Math.min(imageNaturalHeight - next.bottom - 1, Math.round(next.top)));
  const bottom = Math.max(0, Math.min(imageNaturalHeight - top - 1, Math.round(next.bottom)));
  return { ...next, left, right, top, bottom };
}

function startGuideDrag(handle: GuideHandle, event: PointerEvent) {
  event.preventDefault();
  draggingGuide = handle;
  window.addEventListener("pointermove", dragGuide);
  window.addEventListener("pointerup", stopGuideDrag, { once: true });
}

function dragGuide(event: PointerEvent) {
  if (!draggingGuide) return;
  const point = imagePoint(event);
  const next = { ...guides };
  if (draggingGuide === "left") next.left = point.x;
  if (draggingGuide === "right") next.right = imageNaturalWidth - point.x;
  if (draggingGuide === "top") next.top = point.y;
  if (draggingGuide === "bottom") next.bottom = imageNaturalHeight - point.y;
  guides = clampGuides(next);
}

function stopGuideDrag() {
  draggingGuide = null;
  window.removeEventListener("pointermove", dragGuide);
}
```

Render four guide handles over the image when `mode === "nine_slice"`, plus numeric controls and mode selects for `edge_mode` and `center_mode`.

- [ ] **Step 4: Add asset-level guide editing**

In `AssetLibrary.svelte`, change thumbnails so the main image button still opens `PixelEditor`, and add a second button:

```svelte
<button class="asset-action" type="button" onclick={() => editingGuidesAsset = name}>
  Guides
</button>
```

When `editingGuidesAsset` is set, render `UvEditorDialog` with `mode="nine_slice"`:

```svelte
<UvEditorDialog
  title={`Nine-slice: ${displayName(editingGuidesAsset)}`}
  mode="nine_slice"
  assets={project.assets}
  asset={editingGuidesAsset}
  nineSlice={project.assetMetadata[editingGuidesAsset]?.nine_slice ?? null}
  onapply={async (_asset, value) => {
    await project.updateAssetMetadata(editingGuidesAsset!, {
      ...(project.assetMetadata[editingGuidesAsset!] ?? {}),
      nine_slice: value as NineSlice,
    });
    editingGuidesAsset = null;
  }}
  onclear={async () => {
    await project.updateAssetMetadata(editingGuidesAsset!, {
      ...(project.assetMetadata[editingGuidesAsset!] ?? {}),
      nine_slice: null,
    });
    editingGuidesAsset = null;
  }}
  onclose={() => editingGuidesAsset = null}
/>
```

- [ ] **Step 5: Add texture render controls**

In `PropertyPanel.svelte`, for `selectedEl.type === "texture"` add:

```svelte
<label for="prop-render-mode">Render</label>
<select
  id="prop-render-mode"
  value={selectedEl.render_mode ?? "plain"}
  onchange={(event) => updateProp("render_mode", event.currentTarget.value)}
>
  <option value="plain">Plain</option>
  <option value="nine_slice">Nine-slice</option>
</select>

{#if (selectedEl.render_mode ?? "plain") === "nine_slice"}
  <button class="secondary-btn" onclick={() => openNineSliceEditor()} disabled={!selectedEl.asset}>
    Edit Guides
  </button>
  <button class="secondary-btn" onclick={() => updateProp("nine_slice", null)}>
    Use Asset Guides
  </button>
{/if}
```

Implement `openNineSliceEditor()` by reusing `UvEditorDialog` with `mode="nine_slice"` and `targetSize` from the selected element width/height.

- [ ] **Step 6: Run Svelte validation**

Run:

```bash
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/UvEditorDialog.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/PropertyPanel.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/AssetLibrary.svelte --svelte-version 5
pnpm check
```

Expected: autofixer reports no blocking Svelte issues and `pnpm check` exits 0.

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/UvEditorDialog.svelte src/lib/components/PropertyPanel.svelte src/lib/components/AssetLibrary.svelte src/lib/stores/project.svelte.ts
git commit -m "feat: add visual nine-slice guide editor"
```

## Task 5: Pixi Rendering And Cache Bound

**Files:**
- Modify: `src/lib/engine/renderer.ts`

- [ ] **Step 1: Add renderer cache bound before visual changes**

Address Gemini’s `textTextureCache` concern with a small helper:

```ts
private enforceTextureCacheLimit(cache: Map<string, Texture>, limit = 256) {
  while (cache.size > limit) {
    const first = cache.keys().next().value;
    if (!first) return;
    cache.get(first)?.destroy(true);
    cache.delete(first);
  }
}
```

Call it after inserting into `textTextureCache`, `glyphTextureCache`, and `fontSourceTextureCache`.

- [ ] **Step 2: Render nine-slice textures in Pixi**

Add a `renderNineSliceElement(element: Element, texture: Texture, guides: NineSlice)` path that creates nine sprites in a container. Use the same 3x3 rectangle math as Rust:

```ts
const sourceRects = [
  new Rectangle(0, 0, left, top),
  new Rectangle(left, 0, sourceWidth - left - right, top),
  new Rectangle(sourceWidth - right, 0, right, top),
  new Rectangle(0, top, left, sourceHeight - top - bottom),
  new Rectangle(left, top, sourceWidth - left - right, sourceHeight - top - bottom),
  new Rectangle(sourceWidth - right, top, right, sourceHeight - top - bottom),
  new Rectangle(0, sourceHeight - bottom, left, bottom),
  new Rectangle(left, sourceHeight - bottom, sourceWidth - left - right, bottom),
  new Rectangle(sourceWidth - right, sourceHeight - bottom, right, bottom),
];
```

Use `Texture` frame rectangles for corners and `Texture.WHITE` tiling is not acceptable for alpha; use repeated sprites for `tile` mode and scaled sprites for `stretch` mode. Set `roundPixels = true` on sprites and keep `image-rendering` pixelated.

- [ ] **Step 3: Resolve guides in the renderer**

Add:

```ts
private resolveNineSlice(element: Element): NineSlice | null {
  if (element.nine_slice) return element.nine_slice;
  if (!element.asset) return null;
  return project.assetMetadata[element.asset]?.nine_slice ?? null;
}
```

When drawing a texture element with `render_mode === "nine_slice"`, use `resolveNineSlice()`. If guides are missing, draw the plain texture and a warning outline so the user sees the element instead of a blank canvas.

- [ ] **Step 4: Run frontend checks**

Run:

```bash
pnpm check
pnpm build
```

Expected: checks pass. Vite may still warn about the existing large chunk; do not treat that as a failure unless a new build error appears.

- [ ] **Step 5: Commit**

```bash
git add src/lib/engine/renderer.ts
git commit -m "feat: render nine-slice textures in editor"
```

## Task 6: Documentation, Roadmap, And Final Verification

**Files:**
- Modify: `docs/mcp.md`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`
- Modify: `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update local skill**

In `.agents/skills/mc-gui-crafter/SKILL.md`, add:

```markdown
For visual authoring:

- Use asset-level `nine_slice` metadata for reusable panel atlases.
- Set texture elements to `render_mode: "nine_slice"` only when guides are defined on the element or asset.
- Prefer `project_render` after guide changes and inspect the PNG before export.
- Use `icon` plus `icon_uv` for atlas-backed icon buttons; keep `content` as fallback metadata.
```

- [ ] **Step 2: Add MCP workflow example**

In `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`, add a compact JSON-RPC sequence:

```json
{ "name": "asset_metadata_update", "arguments": { "name": "textures/gui/panel_atlas.png", "metadata": { "nine_slice": { "left": 4, "right": 4, "top": 4, "bottom": 4, "edge_mode": "tile", "center_mode": "tile" } } } }
```

Then show `element_update_many` setting:

```json
{ "id": "background", "changes": { "render_mode": "nine_slice", "width": 176, "height": 166 } }
```

and `project_render` verification.

- [ ] **Step 3: Update roadmap**

In `docs/roadmap.md`, mark:

```markdown
- [x] Visual Authoring Alpha design/spec written.
- [x] Visual Authoring Alpha implementation plan written.
- [ ] Visual Authoring Alpha implementation complete.
```

Keep existing MCP Reliability Alpha checked items unchanged.

- [ ] **Step 4: Run final verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
pnpm check
pnpm build
git diff --check
```

Expected:

- Rust tests pass.
- Svelte/TypeScript check reports 0 errors.
- Vite build exits 0.
- `git diff --check` exits 0.

- [ ] **Step 5: Commit final docs**

```bash
git add docs/mcp.md .agents/skills/mc-gui-crafter/SKILL.md .agents/skills/mc-gui-crafter/references/mcp-workflows.md docs/roadmap.md
git commit -m "docs: document visual authoring alpha"
```

## Self-Review Checklist

- Spec coverage:
  - asset-level nine-slice metadata: Task 1 and Task 4.
  - texture `render_mode` and element overrides: Task 1, Task 2, and Task 4.
  - visual guide editor: Task 4.
  - shared Pixi/MCP/export rendering rules: Task 2, Task 3, and Task 5.
  - button standalone and atlas icons: existing model plus Task 3 validation and Task 4 editor controls.
  - MCP authoring and schema discovery: Task 3 and Task 6.
  - compact binary payloads: Task 1 and Task 3.
- Gemini review coverage:
  - `asset_list` payload issue: Task 1.
  - `textTextureCache` growth: Task 5.
  - command/file modularization and broader frontend tests remain separate follow-up work.
- Scope control:
  - no full pixel editor replacement;
  - no arbitrary procedural effects;
  - no editable state variants;
  - no full generated texture replacement migration.
