# MCP UI Polish v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add MCP screenshot previews, first-class button/toggle authoring with icons and tooltips, and polish export/MCP iteration behavior reported by the v2 LLM review.

**Architecture:** Keep the `.mcgui` schema backward-compatible by adding optional element fields. Reuse existing texture compositing/export paths for screenshots and icon baking. Keep MCP default responses compact and add large payloads only behind explicit request flags.

**Tech Stack:** Rust/Tauri 2 backend, Serde project format, PNG/image compositing, Svelte 5 runes frontend, PixiJS v8 renderer, JSON-RPC MCP server, Markdown docs/skills.

---

## File Structure

- Modify `src-tauri/src/project/mod.rs`: optional `Element.icon`, `Element.icon_uv`, and `Element.tooltip` fields plus round-trip tests.
- Modify `src/lib/types.ts`: frontend `Element` type parity for `icon`, `icon_uv`, and `tooltip`.
- Modify `src/lib/stores/editor.svelte.ts`: add `button` and `toggle_button` editor tools.
- Modify `src/lib/stores/project.svelte.ts`: default button/toggle sizing, asset, layer, and selection/render updates.
- Modify `src/lib/components/ElementPalette.svelte`: add Button and Toggle tools and keyboard shortcuts.
- Modify `src/lib/components/PropertyPanel.svelte`: button/toggle label, tooltip, icon asset, icon UV, dimensions, and binding controls.
- Modify `src/lib/components/Canvas.svelte`: make reactivity observe `icon`, `icon_uv`, `tooltip`, and `binding`.
- Modify `src/lib/engine/renderer.ts`: render button/toggle icons from standalone PNG or atlas UV, falling back to label text.
- Modify `src-tauri/src/texture/mod.rs`: composite button icons into exported/screenshot PNGs and expose a full-project screenshot helper.
- Modify `src-tauri/src/export/mod.rs`: layout JSON fields, `overwrite` export config, progress/control warning refinements, Java whitespace cleanup, tests.
- Modify `src-tauri/src/mcp/mod.rs`: `project_screenshot`, `overwrite` schema parsing, effective `element_add_many` response, tests.
- Modify `src/lib/api.ts`: frontend/mock export request parity only if `overwrite` affects frontend commands.
- Modify `docs/mcp.md`, `docs/roadmap.md`, `.agents/skills/mc-gui-crafter/SKILL.md`, `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`: document screenshot, button icons, overwrite, and warnings.

## Task 1: Element Icon And Tooltip Data Model

**Files:**
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Add failing Rust round-trip test**

Add this test inside `#[cfg(test)] mod tests` in `src-tauri/src/project/mod.rs`:

```rust
#[test]
fn element_button_icon_tooltip_fields_round_trip() {
    let json = serde_json::json!({
        "id": "settings_button",
        "type": "button",
        "x": 12,
        "y": 18,
        "width": 20,
        "height": 20,
        "asset": "textures/generated/button.png",
        "icon": "textures/gui/widgets.png",
        "icon_uv": { "x": 16, "y": 0, "width": 16, "height": 16 },
        "tooltip": "Open settings",
        "content": "Settings"
    });

    let element: Element = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(element.icon.as_deref(), Some("textures/gui/widgets.png"));
    assert_eq!(element.icon_uv.as_ref().unwrap().x, 16);
    assert_eq!(element.tooltip.as_deref(), Some("Open settings"));

    let serialized = serde_json::to_value(element).unwrap();
    assert_eq!(serialized["icon"], "textures/gui/widgets.png");
    assert_eq!(serialized["icon_uv"]["width"], 16);
    assert_eq!(serialized["tooltip"], "Open settings");
}
```

- [ ] **Step 2: Run the failing model test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml element_button_icon_tooltip_fields_round_trip --locked
```

Expected: FAIL because `Element` does not yet have `icon`, `icon_uv`, or `tooltip`.

- [ ] **Step 3: Add optional Rust fields**

In `src-tauri/src/project/mod.rs`, extend `Element` after `asset` or near the text fields:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub icon: Option<String>,
#[serde(skip_serializing_if = "Option::is_none")]
pub icon_uv: Option<UvRect>,
#[serde(skip_serializing_if = "Option::is_none")]
pub tooltip: Option<String>,
```

Update every local `Element { ... }` literal in Rust tests/templates to include:

```rust
icon: None,
icon_uv: None,
tooltip: None,
```

Use compiler errors to find every literal. Do not add defaults that hide missing construction sites.

- [ ] **Step 4: Add frontend type fields**

In `src/lib/types.ts`, extend `Element`:

```ts
  icon?: string;
  icon_uv?: UvRect | null;
  tooltip?: string;
```

- [ ] **Step 5: Verify model tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml element_button_icon_tooltip_fields_round_trip --locked
cargo test --manifest-path src-tauri/Cargo.toml project::tests --locked
pnpm check
```

Expected: PASS.

- [ ] **Step 6: Commit data model**

```bash
git add src-tauri/src/project/mod.rs src/lib/types.ts
git commit -m "feat: add button icon tooltip fields"
```

## Task 2: Button And Toggle Authoring In The UI

**Files:**
- Modify: `src/lib/stores/editor.svelte.ts`
- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src/lib/components/ElementPalette.svelte`
- Modify: `src/lib/components/PropertyPanel.svelte`
- Modify: `src/lib/components/Canvas.svelte`
- Modify: `src/lib/engine/renderer.ts`

- [ ] **Step 1: Extend editor tool type**

In `src/lib/stores/editor.svelte.ts`, change:

```ts
export type EditorTool = "select" | "pan" | "slot" | "texture" | "text";
```

to:

```ts
export type EditorTool = "select" | "pan" | "slot" | "texture" | "text" | "button" | "toggle_button";
```

- [ ] **Step 2: Add default element construction**

In `ProjectStore.addElement()` in `src/lib/stores/project.svelte.ts`, after the text defaults, add:

```ts
    if (type === "button") {
      element.width ??= 52;
      element.height ??= 20;
      element.asset ??= "textures/generated/button.png";
      element.layer ??= "background";
      element.content ??= "Button";
    }

    if (type === "toggle_button") {
      element.width ??= 20;
      element.height ??= 20;
      element.asset ??= "textures/generated/button.png";
      element.layer ??= "background";
      element.content ??= "Toggle";
    }
```

- [ ] **Step 3: Add palette tools and shortcuts**

In `src/lib/components/ElementPalette.svelte`, extend `tools`:

```ts
    { id: "button", label: "Button", shortcut: "B" },
    { id: "toggle_button", label: "Toggle", shortcut: "G" },
```

Extend keyboard handling:

```ts
      case "b": editor.tool = "button"; break;
      case "g": editor.tool = "toggle_button"; break;
```

Extend icon rendering:

```svelte
          {:else if tool.id === "button"}▭
          {:else if tool.id === "toggle_button"}◉
```

Extend hints:

```svelte
    {:else if activeTool === "button"}
      Click canvas to place button.
    {:else if activeTool === "toggle_button"}
      Click canvas to place toggle.
```

- [ ] **Step 4: Ensure canvas click placement accepts new tools**

Inspect `src/lib/engine/renderer.ts` click placement logic for `editor.tool`. Add:

```ts
      case "button":
        void project.addElement("button", x, y);
        editor.tool = "select";
        break;
      case "toggle_button":
        void project.addElement("toggle_button", x, y);
        editor.tool = "select";
        break;
```

Use the existing local variable names for GUI coordinates. Do not duplicate coordinate conversion logic.

- [ ] **Step 5: Expose button/toggle dimensions**

In `PropertyPanel.svelte`, change the size branch:

```svelte
      {:else if selectedEl.type === "texture" || selectedEl.type === "progress" || selectedEl.type === "fluid_tank" || selectedEl.type === "energy_bar"}
```

to include:

```svelte
        || selectedEl.type === "button"
        || selectedEl.type === "toggle_button"
```

Keep the same width/height inputs.

- [ ] **Step 6: Share label controls with button/toggle**

Replace:

```svelte
      {#if selectedEl.type === "text"}
```

with:

```svelte
      {#if selectedEl.type === "text" || selectedEl.type === "button" || selectedEl.type === "toggle_button"}
```

Keep `Content`, `Font`, `Color`, and `Shadow` controls for text and button labels. If visual density is poor, keep the same controls; do not introduce a second text-control component in this task.

- [ ] **Step 7: Add button metadata controls**

After the shared text controls, add:

```svelte
      {#if selectedEl.type === "button" || selectedEl.type === "toggle_button"}
        <div class="prop-section">
          <div class="section-title">Button</div>
          <div class="prop-row">
            <label for="prop-tooltip">Tooltip</label>
            <input
              id="prop-tooltip"
              type="text"
              value={selectedEl.tooltip ?? ""}
              oninput={(e) => updateSelectedElement({ tooltip: optionalText(e.currentTarget.value) })}
            />
          </div>
          <div class="prop-row">
            <label for="prop-binding">Binding</label>
            <input
              id="prop-binding"
              type="text"
              value={selectedEl.binding ?? ""}
              oninput={(e) => updateSelectedElement({ binding: optionalText(e.currentTarget.value) ?? undefined })}
            />
          </div>
          <div class="prop-row">
            <label for="prop-icon">Icon</label>
            <select
              id="prop-icon"
              value={selectedEl.icon ?? ""}
              onchange={(e) => updateSelectedElement({ icon: e.currentTarget.value || undefined, icon_uv: e.currentTarget.value ? selectedEl.icon_uv : null })}
            >
              <option value="">(none)</option>
              {#each project.assets as a (a)}
                <option value={a}>{a.replace("textures/", "").replace(".png", "")}</option>
              {/each}
            </select>
          </div>
          <div class="uv-grid">
            <label for="prop-icon-uv-x">Icon X</label>
            <input id="prop-icon-uv-x" type="number" min="0" value={selectedEl.icon_uv?.x ?? 0} oninput={(e) => updateIconUv("x", e.currentTarget.value)} />
            <label for="prop-icon-uv-y">Icon Y</label>
            <input id="prop-icon-uv-y" type="number" min="0" value={selectedEl.icon_uv?.y ?? 0} oninput={(e) => updateIconUv("y", e.currentTarget.value)} />
            <label for="prop-icon-uv-width">Icon W</label>
            <input id="prop-icon-uv-width" type="number" min="1" value={selectedEl.icon_uv?.width ?? 16} oninput={(e) => updateIconUv("width", e.currentTarget.value)} />
            <label for="prop-icon-uv-height">Icon H</label>
            <input id="prop-icon-uv-height" type="number" min="1" value={selectedEl.icon_uv?.height ?? 16} oninput={(e) => updateIconUv("height", e.currentTarget.value)} />
          </div>
          <button class="secondary-btn" onclick={() => updateSelectedElement({ icon_uv: null })}>
            Clear Icon UV
          </button>
        </div>
      {/if}
```

Add the helper in the `<script>` block:

```ts
  function updateIconUv(key: "x" | "y" | "width" | "height", value: string) {
    if (!selectedEl) return;
    const next = {
      x: selectedEl.icon_uv?.x ?? 0,
      y: selectedEl.icon_uv?.y ?? 0,
      width: selectedEl.icon_uv?.width ?? 16,
      height: selectedEl.icon_uv?.height ?? 16,
      [key]: Math.max(key === "width" || key === "height" ? 1 : 0, numberValue(value)),
    };
    updateSelectedElement({ icon_uv: next });
  }
```

- [ ] **Step 8: Observe new fields for canvas rerendering**

In `Canvas.svelte`, inside the element observation loop, add:

```ts
      void element.icon;
      void element.icon_uv?.x;
      void element.icon_uv?.y;
      void element.icon_uv?.width;
      void element.icon_uv?.height;
      void element.tooltip;
      void element.binding;
```

- [ ] **Step 9: Verify frontend checks**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS. The existing Vite chunk-size warning is acceptable.

- [ ] **Step 10: Commit UI authoring**

```bash
git add src/lib/stores/editor.svelte.ts src/lib/stores/project.svelte.ts src/lib/components/ElementPalette.svelte src/lib/components/PropertyPanel.svelte src/lib/components/Canvas.svelte src/lib/engine/renderer.ts
git commit -m "feat: add button authoring controls"
```

## Task 3: Button Icon Rendering And Export Baking

**Files:**
- Modify: `src/lib/engine/renderer.ts`
- Modify: `src-tauri/src/texture/mod.rs`
- Modify: `src-tauri/src/export/mod.rs`

- [ ] **Step 1: Add failing export tests for icon baking**

In `src-tauri/src/texture/mod.rs`, add tests using tiny PNG fixtures created in-memory:

```rust
fn test_png(width: u32, height: u32, color: Rgba<u8>) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, color);
    let mut bytes = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

#[test]
fn background_export_bakes_button_standalone_icon_pixels() {
    let mut project = Project::new("Icon Button", 64, 32, crate::project::ModTarget::Forge);
    project.texture_data.insert(
        "textures/gui/icons/settings.png".into(),
        test_png(8, 8, Rgba([0x11, 0x22, 0x33, 0xff])),
    );
    project.assets.push("textures/gui/icons/settings.png".into());
    let mut button = crate::templates::base_element_for_test("button", ElementType::Button, 8, 6);
    button.width = Some(20);
    button.height = Some(20);
    button.icon = Some("textures/gui/icons/settings.png".into());
    project.elements.push(button);

    let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
    let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

    assert_eq!(image.get_pixel(14, 12), &Rgba([0x11, 0x22, 0x33, 0xff]));
}

#[test]
fn background_export_bakes_button_icon_uv_pixels() {
    let mut project = Project::new("Icon UV Button", 64, 32, crate::project::ModTarget::Forge);
    let mut atlas = RgbaImage::from_pixel(16, 8, Rgba([0x00, 0x00, 0x00, 0xff]));
    for x in 8..16 {
        for y in 0..8 {
            atlas.put_pixel(x, y, Rgba([0xaa, 0x44, 0x11, 0xff]));
        }
    }
    let mut bytes = Vec::new();
    atlas
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    project.texture_data.insert("textures/gui/widgets.png".into(), bytes);
    project.assets.push("textures/gui/widgets.png".into());
    let mut button = crate::templates::base_element_for_test("button", ElementType::Button, 8, 6);
    button.width = Some(20);
    button.height = Some(20);
    button.icon = Some("textures/gui/widgets.png".into());
    button.icon_uv = Some(crate::project::UvRect { x: 8, y: 0, width: 8, height: 8 });
    project.elements.push(button);

    let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
    let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

    assert_eq!(image.get_pixel(14, 12), &Rgba([0xaa, 0x44, 0x11, 0xff]));
}
```

- [ ] **Step 2: Add failing layout JSON test**

In `src-tauri/src/export/mod.rs`, add:

```rust
#[test]
fn layout_json_preserves_button_icon_and_tooltip_metadata() {
    let mut project = Project::new("Button Metadata", 176, 166, crate::project::ModTarget::Forge);
    let mut button = crate::templates::base_element_for_test("settings", ElementType::Button, 8, 8);
    button.icon = Some("textures/gui/widgets.png".into());
    button.icon_uv = Some(crate::project::UvRect { x: 16, y: 0, width: 16, height: 16 });
    button.tooltip = Some("Open settings".into());
    button.content = Some("Settings".into());
    project.elements.push(button);

    let layout = layout_json_value(
        &project,
        serde_json::json!({ "background": "textures/gui/button_metadata_gui.png" }),
    );
    let element = &layout["elements"][0];

    assert_eq!(element["icon"], "textures/gui/widgets.png");
    assert_eq!(element["icon_uv"]["x"], 16);
    assert_eq!(element["tooltip"], "Open settings");
}
```

- [ ] **Step 3: Run failing export/texture tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml background_export_bakes_button_standalone_icon_pixels background_export_bakes_button_icon_uv_pixels layout_json_preserves_button_icon_and_tooltip_metadata --locked
```

Expected: Cargo may reject multiple filters. If so, run each test name separately. The tests should fail until icon fields and baking are implemented.

- [ ] **Step 4: Implement icon texture compositing helper**

In `src-tauri/src/texture/mod.rs`, add a helper after `overlay_button`:

```rust
fn overlay_button_icon(img: &mut RgbaImage, project: &Project, element: &Element) -> Result<(), String> {
    let Some(icon_name) = element.icon.as_deref() else {
        return Ok(());
    };
    let Some(data) = project.texture_data.get(icon_name) else {
        return Ok(());
    };
    let texture = image::load_from_memory(data)
        .map_err(|error| format!("Failed to load button icon '{}': {error}", icon_name))?
        .to_rgba8();
    let source = cropped_source(&texture, element.icon_uv.as_ref());
    if source.width() == 0 || source.height() == 0 {
        return Ok(());
    }
    let max_w = element.width.or(element.size).unwrap_or(20).saturating_sub(4).max(1);
    let max_h = element.height.or(element.size).unwrap_or(20).saturating_sub(4).max(1);
    let target_w = source.width().min(max_w);
    let target_h = source.height().min(max_h);
    let resized = image::imageops::resize(&source, target_w, target_h, image::imageops::FilterType::Nearest);
    let x = element.x + ((element.width.or(element.size).unwrap_or(20) - target_w) / 2) as i32;
    let y = element.y + ((element.height.or(element.size).unwrap_or(20) - target_h) / 2) as i32;
    image::imageops::overlay(img, &resized, x as i64, y as i64);
    Ok(())
}
```

Extract the existing UV crop logic into:

```rust
fn cropped_source(tex: &RgbaImage, uv: Option<&crate::project::UvRect>) -> RgbaImage {
    if let Some(uv) = uv {
        let x = uv.x.min(tex.width());
        let y = uv.y.min(tex.height());
        let width = uv.width.min(tex.width().saturating_sub(x));
        let height = uv.height.min(tex.height().saturating_sub(y));
        if width == 0 || height == 0 {
            return RgbaImage::new(0, 0);
        }
        tex.view(x, y, width, height).to_image()
    } else {
        tex.clone()
    }
}
```

Then update `overlay_texture_data` and `composite_single_element` to use `cropped_source(...)` for normal `uv`.

- [ ] **Step 5: Composite icon after button chrome**

Change `overlay_button`:

```rust
    if let Some(asset_name) = element.asset.as_deref().or_else(|| {
        project
            .texture_data
            .contains_key("textures/generated/button.png")
            .then_some("textures/generated/button.png")
    }) {
        overlay_asset(img, project, element, asset_name)?;
    } else {
        let data = generated_button()?;
        overlay_texture_data(img, element, &data, "generated button")?;
    }

    overlay_button_icon(img, project, element)
```

- [ ] **Step 6: Render icons in Pixi**

In `src/lib/engine/renderer.ts`, add after `drawButtonBackground`:

```ts
  private drawButtonIcon(el: Element): Container | null {
    if (!el.icon) return null;
    const dataUrl = assetDataUrls.get(el.icon);
    if (!dataUrl) return null;
    const source = Texture.from(dataUrl);
    const texture = this.textureWithIconUv(source, el);
    if (!texture) return null;
    const sprite = new Sprite(texture);
    const w = el.width ?? el.size ?? 40;
    const h = el.height ?? el.size ?? 20;
    const maxW = Math.max(1, w - 4);
    const maxH = Math.max(1, h - 4);
    const scale = Math.min(1, maxW / texture.width, maxH / texture.height);
    sprite.width = Math.max(1, Math.floor(texture.width * scale));
    sprite.height = Math.max(1, Math.floor(texture.height * scale));
    sprite.x = Math.floor(el.x + (w - sprite.width) / 2);
    sprite.y = Math.floor(el.y + (h - sprite.height) / 2);
    const container = new Container();
    container.addChild(sprite);
    return container;
  }

  private textureWithIconUv(baseTexture: Texture, el: Element): Texture | null {
    const uv = el.icon_uv;
    if (!uv || uv.width <= 0 || uv.height <= 0) return baseTexture;
    const sourceWidth = baseTexture.source.width;
    const sourceHeight = baseTexture.source.height;
    const x = Math.max(0, Math.min(uv.x, sourceWidth));
    const y = Math.max(0, Math.min(uv.y, sourceHeight));
    const width = Math.min(uv.width, sourceWidth - x);
    const height = Math.min(uv.height, sourceHeight - y);
    if (width <= 0 || height <= 0) return null;
    return new Texture({ source: baseTexture.source, frame: new Rectangle(x, y, width, height) });
  }
```

Change `drawButton` so the icon has priority:

```ts
    const icon = this.drawButtonIcon(el);
    if (icon) {
      container.addChild(icon);
    } else {
      const label = this.drawButtonLabel(el);
      if (label) container.addChild(label);
    }
```

- [ ] **Step 7: Verify rendering/export checks**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml texture --locked
cargo test --manifest-path src-tauri/Cargo.toml export --locked
pnpm check
pnpm build
```

Expected: PASS.

- [ ] **Step 8: Commit icon rendering/export**

```bash
git add src/lib/engine/renderer.ts src-tauri/src/texture/mod.rs src-tauri/src/export/mod.rs
git commit -m "feat: render button icons"
```

## Task 4: MCP Screenshot And Response Ergonomics

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `src-tauri/src/texture/mod.rs`

- [ ] **Step 1: Add screenshot PNG helper**

In `src-tauri/src/texture/mod.rs`, add:

```rust
pub fn composite_project_preview(project: &Project) -> Result<Vec<u8>, String> {
    composite_atlas_for_layer(project, Layer::Background)
}
```

This intentionally starts with background-layer visual parity with exported GUI texture. Overlay/animatable screenshots can be added later without changing the MCP tool contract.

- [ ] **Step 2: Add failing MCP screenshot tests**

In `src-tauri/src/mcp/mod.rs`, add:

```rust
#[test]
fn project_screenshot_writes_compact_png_metadata() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Screenshot", 64, 32, ModTarget::Forge);
        project.elements.push(base_slot_element("slot_a".into(), 8, 8, 18));
        sessions.create_session(project)
    };
    let temp = tempfile::tempdir().unwrap();
    let output_path = temp.path().join("preview.png");

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "screenshot",
            "method": "tools/call",
            "params": {
                "name": "project_screenshot",
                "arguments": {
                    "project_id": project_id,
                    "output_path": output_path,
                }
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(value["width"], 64);
    assert_eq!(value["height"], 32);
    assert!(value["bytes"].as_u64().unwrap() > 0);
    assert_eq!(value["sha256"].as_str().unwrap().len(), 64);
    assert!(value.get("data_url").is_none());
    assert!(std::path::Path::new(value["path"].as_str().unwrap()).exists());
}

#[test]
fn project_screenshot_includes_data_url_only_when_requested() {
    let state = test_state();
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.create_session(Project::new("Screenshot Data URL", 32, 24, ModTarget::Forge));
    }

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "screenshot",
            "method": "tools/call",
            "params": {
                "name": "project_screenshot",
                "arguments": { "include_data_url": true }
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(value["data_url"].as_str().unwrap().starts_with("data:image/png;base64,"));
}
```

- [ ] **Step 3: Add tool definition**

Add to `get_tool_definitions()`:

```rust
td(
    "project_screenshot",
    "Render the current project to a PNG screenshot and return compact metadata",
    project_props(&[
        ("output_path", "string", "Optional PNG path to write; temp file is used when omitted", false),
        ("include_data_url", "boolean", "Include data:image/png;base64 payload; defaults to false", false),
    ]),
),
```

Add `"project_screenshot"` to `is_mutating_tool` as read-only:

```rust
            | "project_screenshot"
```

- [ ] **Step 4: Implement `project_screenshot` route**

Add to `execute_tool`:

```rust
"project_screenshot" => project_screenshot(&sessions, project_id, args),
```

Implement:

```rust
fn project_screenshot(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let project = &sessions.resolve(project_id)?.project;
    let png = crate::texture::composite_project_preview(project)?;
    let path = optional_string(args, "output_path")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "mc-gui-crafter-screenshot-{}.png",
                uuid::Uuid::new_v4()
            ))
        });
    if path.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err("output_path must end with .png".to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create screenshot directory: {error}"))?;
    }
    std::fs::write(&path, &png)
        .map_err(|error| format!("Failed to write screenshot PNG: {error}"))?;

    let image = image::load_from_memory(&png)
        .map_err(|error| format!("Failed to inspect screenshot PNG: {error}"))?;
    let mut metadata = compact_asset_metadata_with_dimensions(
        path.to_string_lossy().as_ref(),
        &png,
        image.width(),
        image.height(),
    );
    metadata["path"] = serde_json::json!(path.to_string_lossy().to_string());
    if optional_bool(args, "include_data_url")?.unwrap_or(false) {
        metadata["data_url"] = serde_json::json!(data_url_for_png(&png));
    }
    Ok(metadata)
}
```

If `optional_bool` does not exist, add:

```rust
fn optional_bool(value: &serde_json::Value, key: &str) -> Result<Option<bool>, String> {
    value
        .get(key)
        .map(|value| value.as_bool().ok_or(format!("{key} must be boolean")))
        .transpose()
}
```

Add this helper near `decode_png_data_url`:

```rust
fn data_url_for_png(data: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    )
}
```

Then update `asset_get_data_url` to call `data_url_for_png(data)` so screenshot and asset responses share the same encoder.

- [ ] **Step 5: Make `element_add_many` return MCP presentation elements**

Change its response:

```rust
let returned_elements = elements
    .iter()
    .map(element_for_mcp)
    .collect::<Vec<_>>();
Ok(serde_json::json!({
    "created_count": elements.len(),
    "elements": returned_elements,
}))
```

Add test:

```rust
#[test]
fn element_add_many_response_includes_effective_layer() {
    let state = test_state();
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.create_session(Project::new("Bulk Effective Layer", 176, 166, ModTarget::Forge));
    }

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "bulk",
            "method": "tools/call",
            "params": {
                "name": "element_add_many",
                "arguments": {
                    "elements": [
                        { "id": "slot_a", "type": "slot", "x": 8, "y": 8, "size": 18 }
                    ]
                }
            }
        }),
        &state,
    );

    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(value["elements"][0]["layer"], "background");
}
```

- [ ] **Step 6: Verify MCP tests**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml mcp::tests --locked
```

Expected: PASS.

- [ ] **Step 7: Commit MCP screenshot**

```bash
git add src-tauri/src/mcp/mod.rs src-tauri/src/texture/mod.rs
git commit -m "feat: add mcp project screenshots"
```

## Task 5: Export Overwrite, Java Whitespace, And Preview Warnings

**Files:**
- Modify: `src-tauri/src/export/mod.rs`
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `src/lib/api.ts` only if frontend export command types require `overwrite`

- [ ] **Step 1: Add failing overwrite test**

In `src-tauri/src/export/mod.rs`, next to `preview_reports_existing_target_files_as_warnings_without_overwriting`, add:

```rust
#[test]
fn preview_overwrite_suppresses_existing_target_warnings() {
    let temp = tempfile::tempdir().unwrap();
    let project = Project::new("Overwrite", 176, 166, crate::project::ModTarget::Forge);
    let config = ExportConfig {
        mod_id: "overwrite_test".into(),
        package: "net.inkyquill.overwrite".into(),
        class_name: "OverwriteScreen".into(),
        output_dir: temp.path().to_string_lossy().into_owned(),
        settings_override: None,
        overwrite: false,
    };
    let first = preview_export(&project, &config, "forge").unwrap();
    std::fs::create_dir_all(std::path::Path::new(&first.files[0]).parent().unwrap()).unwrap();
    std::fs::write(&first.files[0], "existing").unwrap();

    let warning_preview = preview_export(&project, &config, "forge").unwrap();
    assert!(warning_preview.warnings.iter().any(|warning| warning.contains("already exists")));

    let overwrite_preview = preview_export(&project, &ExportConfig { overwrite: true, ..config }, "forge").unwrap();
    assert!(!overwrite_preview.warnings.iter().any(|warning| warning.contains("already exists")));
}
```

- [ ] **Step 2: Add failing whitespace test**

In `src-tauri/src/export/mod.rs`, add:

```rust
#[test]
fn generated_java_files_have_no_trailing_whitespace() {
    let project = Project::new("Whitespace", 176, 166, crate::project::ModTarget::Forge);
    let temp = tempfile::tempdir().unwrap();
    let config = ExportConfig {
        mod_id: "whitespace_test".into(),
        package: "net.inkyquill.whitespace".into(),
        class_name: "WhitespaceScreen".into(),
        output_dir: temp.path().to_string_lossy().into_owned(),
        settings_override: None,
        overwrite: false,
    };
    let plan = plan_export(&project, &config, "forge").unwrap();

    for file in plan.files {
        if file.path.extension().and_then(|extension| extension.to_str()) != Some("java") {
            continue;
        }
        let text = String::from_utf8(file.data).unwrap();
        for (index, line) in text.lines().enumerate() {
            assert_eq!(line.trim_end(), line, "{}:{} has trailing whitespace", file.path.display(), index + 1);
        }
    }
}
```

- [ ] **Step 3: Add failing warning tests**

In `src-tauri/src/export/mod.rs`, add:

```rust
#[test]
fn preview_warns_when_progress_element_stretches_referenced_texture() {
    let mut project = Project::new("Progress Stretch", 176, 166, crate::project::ModTarget::Forge);
    project.texture_data.insert(
        "textures/generated/progress_arrow.png".into(),
        crate::texture::generated_progress_arrow().unwrap(),
    );
    project.assets.push("textures/generated/progress_arrow.png".into());
    let mut progress = crate::templates::base_element_for_test("progress", ElementType::Progress, 8, 8);
    progress.asset = Some("textures/generated/progress_arrow.png".into());
    progress.width = Some(40);
    progress.height = Some(20);
    project.elements.push(progress);

    let temp = tempfile::tempdir().unwrap();
    let preview = preview_export(&project, &ExportConfig {
        mod_id: "progress_stretch".into(),
        package: "net.inkyquill.progress".into(),
        class_name: "ProgressStretchScreen".into(),
        output_dir: temp.path().to_string_lossy().into_owned(),
        settings_override: None,
        overwrite: false,
    }, "forge").unwrap();

    assert!(preview.warnings.iter().any(|warning| warning.contains("progress") && warning.contains("stretched")));
}

#[test]
fn preview_warns_for_specific_control_buttons_group_without_buttons() {
    let mut project = Project::new("Control Buttons", 176, 166, crate::project::ModTarget::Forge);
    project.semantic_groups.push(SemanticGroup {
        id: "settings".into(),
        kind: SemanticGroupKind::ControlButtons,
        columns: None,
        visible_rows: None,
        total_rows: None,
        slot_count: Some(1),
        data_source: Some("settings".into()),
        scroll_binding: None,
        dynamic_height: false,
    });

    let warnings = semantic_warnings(&project, &ProjectExportSettings {
        codegen_mode: CodegenMode::Modular,
        generate_runtime_helpers: true,
        generate_semantic_registry: true,
    });

    assert!(warnings.iter().any(|warning| warning.contains("settings") && warning.contains("button")));
}
```

- [ ] **Step 4: Implement `overwrite` in export config**

Add field:

```rust
pub overwrite: bool,
```

to `ExportConfig`. Update all Rust tests and constructors with `overwrite: false` unless the test explicitly uses true.

Change `preview_export`:

```rust
let mut warnings = if config.overwrite {
    Vec::new()
} else {
    existing_file_warnings(&plan.files)
};
```

- [ ] **Step 5: Parse `overwrite` in MCP export request**

In `export_props()`, add:

```rust
("overwrite", "boolean", "Allow overwriting planned generated files without existing-file warnings", false),
```

In `export_request`, set:

```rust
overwrite: optional_bool(args, "overwrite")?.unwrap_or(false),
```

Add MCP tests for wrong-typed overwrite:

```rust
#[test]
fn export_request_rejects_wrong_typed_overwrite() {
    let state = test_state();
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.create_session(Project::new("Overwrite Type", 176, 166, ModTarget::Forge));
    }
    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "preview",
            "method": "tools/call",
            "params": {
                "name": "project_export_preview",
                "arguments": {
                    "target": "forge",
                    "mod_id": "overwrite_type",
                    "package": "net.inkyquill.overwrite",
                    "class_name": "OverwriteType",
                    "output_dir": "/tmp/mcgui-overwrite-type",
                    "overwrite": "true"
                }
            }
        }),
        &state,
    );
    assert_eq!(response["error"]["message"], "overwrite must be boolean");
}
```

- [ ] **Step 6: Trim generated text files**

In `src-tauri/src/export/mod.rs`, add:

```rust
fn generated_text(text: String) -> Vec<u8> {
    let mut output = String::new();
    for line in text.lines() {
        output.push_str(line.trim_end());
        output.push('\n');
    }
    output.into_bytes()
}
```

Change every generated Java/Gradle/TOML/README string passed to `plan_file` from:

```rust
generate_gui_layout_java(&export, target, project).into_bytes()
```

to:

```rust
generated_text(generate_gui_layout_java(&export, target, project))
```

Do this for all generated text files in `plan_export`. Do not apply it to PNG data or serde JSON bytes.

- [ ] **Step 7: Implement warnings**

Add warning helper:

```rust
fn progress_texture_warnings(project: &Project) -> Vec<String> {
    project
        .elements
        .iter()
        .filter(|element| element.element_type == ElementType::Progress)
        .filter_map(|element| {
            let asset = element.asset.as_deref()?;
            let data = project.texture_data.get(asset)?;
            let image = image::load_from_memory(data).ok()?;
            let width = element.width.unwrap_or(image.width());
            let height = element.height.unwrap_or(image.height());
            ((width, height) != (image.width(), image.height())).then(|| {
                format!(
                    "Progress element '{}' is stretched from texture '{}' ({}x{}) to {}x{}; this is allowed but may be accidental for pixel-art GUI work.",
                    element.id,
                    asset,
                    image.width(),
                    image.height(),
                    width,
                    height
                )
            })
        })
        .collect()
}
```

Add helper:

```rust
fn control_button_warnings(project: &Project, group: &SemanticGroup) -> Vec<String> {
    if group.kind != SemanticGroupKind::ControlButtons {
        return Vec::new();
    }
    if group.slot_count.is_none() && group.data_source.is_none() {
        return Vec::new();
    }
    let expected = group.slot_count.unwrap_or(1) as usize;
    let matching = project
        .elements
        .iter()
        .filter(|element| matches!(element.element_type, ElementType::Button | ElementType::ToggleButton))
        .filter(|element| {
            element.inventory_group.as_deref() == Some(group.id.as_str())
                || element.binding.as_deref() == group.data_source.as_deref()
                || element.target_group.as_deref() == Some(group.id.as_str())
        })
        .count();
    if matching < expected {
        vec![format!(
            "Semantic group '{}' declares control buttons but only {} matching button elements were found.",
            group.id, matching
        )]
    } else {
        Vec::new()
    }
}
```

Extend `semantic_integrity_warnings`:

```rust
warnings.extend(control_button_warnings(project, group));
```

Extend `preview_export` after semantic warnings so progress-size validation runs for both simple and modular codegen:

```rust
warnings.extend(semantic_warnings(project, &settings));
warnings.extend(progress_texture_warnings(project));
```

- [ ] **Step 8: Verify export/MCP tests**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml export --locked
cargo test --manifest-path src-tauri/Cargo.toml mcp::tests --locked
git diff --check
```

Expected: PASS.

- [ ] **Step 9: Commit export polish**

```bash
git add src-tauri/src/export/mod.rs src-tauri/src/mcp/mod.rs src/lib/api.ts
git commit -m "feat: polish export preview warnings"
```

If `src/lib/api.ts` was not modified, omit it from `git add`.

## Task 6: Documentation And Skill Updates

**Files:**
- Modify: `docs/mcp.md`
- Modify: `docs/roadmap.md`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`
- Modify: `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`

- [ ] **Step 1: Update MCP docs**

In `docs/mcp.md`, add sections describing:

````markdown
### Screenshots

Use `project_screenshot` when an agent needs a visual check. By default it
writes a PNG and returns compact metadata:

```json
{
  "name": "project_screenshot",
  "arguments": {
    "output_path": "/tmp/mcgui-preview.png"
  }
}
```

The response includes `path`, `width`, `height`, `bytes`, and `sha256`.
Set `include_data_url: true` only when the client cannot open local files.
````

Add button icon example:

```json
{
  "id": "settings_button",
  "type": "button",
  "x": 148,
  "y": 18,
  "width": 20,
  "height": 20,
  "content": "Settings",
  "tooltip": "Open settings",
  "icon": "textures/gui/widgets.png",
  "icon_uv": { "x": 16, "y": 0, "width": 16, "height": 16 },
  "asset": "textures/generated/button.png"
}
```

Document `overwrite: true` on `project_export_preview`/`project_export`.

- [ ] **Step 2: Update skill workflow**

In `.agents/skills/mc-gui-crafter/SKILL.md` and `references/mcp-workflows.md`, add:

- use `project_screenshot` after major layout changes;
- use `icon` plus `icon_uv` for atlas-backed button icons;
- keep `content` even for icon buttons as label/accessibility metadata;
- use `overwrite: true` during iteration when re-exporting to the same generated directory;
- call `asset_get_data_url` only for explicit binary inspection.

- [ ] **Step 3: Update roadmap**

In `docs/roadmap.md`, add or update an MCP/UI polish entry:

```markdown
- [x] MCP/UI polish v2: screenshot previews, button/toggle authoring, icon/tooltip metadata, overwrite previews, and validation polish
```

- [ ] **Step 4: Validate docs**

Run:

```bash
python /home/inky/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/mc-gui-crafter
git diff --check
```

Expected: PASS and `Skill is valid!`.

- [ ] **Step 5: Commit docs**

```bash
git add docs/mcp.md docs/roadmap.md .agents/skills/mc-gui-crafter/SKILL.md .agents/skills/mc-gui-crafter/references/mcp-workflows.md
git commit -m "docs: describe screenshots and button icons"
```

## Task 7: End-To-End Verification

**Files:**
- No planned source edits. Commit only if verification finds and fixes a real issue.

- [ ] **Step 1: Run full local verification**

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
- Svelte check has 0 errors and 0 warnings;
- Vite build passes; existing large chunk warning is acceptable;
- no whitespace errors.

- [ ] **Step 2: Start dev app for live MCP**

Run with the documented Wayland workaround:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11 pnpm tauri dev
```

If single-instance redirects to an already running app whose MCP listener is stale, stop only the stale `target/debug/mc-gui-crafter` process and restart the command.

- [ ] **Step 3: Run live MCP workflow**

Use the configured port from `~/.config/mc-gui-crafter/config.json`.

Create a project:

```json
{
  "name": "MCP UI Polish v2 E2E",
  "template": "empty",
  "width": 176,
  "height": 166,
  "mod_target": "forge"
}
```

Add at least:

- one machine slot grid through `slot_grid_add`;
- player inventory and hotbar through `slot_grid_add`;
- one text-only button;
- one icon button using standalone icon PNG;
- one icon button using atlas `icon_uv`;
- one progress element with matching dimensions;
- one progress element with mismatched dimensions to verify the warning.

- [ ] **Step 4: Verify screenshot MCP output**

Call:

```json
{
  "name": "project_screenshot",
  "arguments": {
    "output_path": "/tmp/mcgui-polish-v2-e2e.png"
  }
}
```

Expected:

- response has `path`, `width`, `height`, `bytes`, `sha256`;
- response has no `data_url`;
- `/tmp/mcgui-polish-v2-e2e.png` exists and is a PNG.

- [ ] **Step 5: Verify export behavior**

Call `project_export_preview` and `project_export` with:

```json
{
  "target": "forge",
  "mod_id": "polish_v2_e2e",
  "package": "net.inkyquill.polishv2",
  "class_name": "PolishV2Screen",
  "output_dir": "/tmp/mcgui-polish-v2-export",
  "codegen_mode": "modular",
  "overwrite": true
}
```

Expected:

- no existing-file warnings on repeated preview/export with `overwrite: true`;
- progress stretching warning appears only for the deliberately mismatched progress element;
- exported layout JSON contains `icon`, `icon_uv`, and `tooltip`;
- exported GUI PNG contains button chrome and icon pixels;
- `rg -n "[ \t]+$" /tmp/mcgui-polish-v2-export` returns no matches.

- [ ] **Step 6: Browser smoke**

In the running app:

- create a button from the palette;
- select it;
- edit label, tooltip, icon asset, and icon UV in Properties;
- verify the canvas updates without console errors;
- create a toggle button and verify default size/asset.

- [ ] **Step 7: Final commit check**

Run:

```bash
git status --short
git log --oneline -10
```

Expected: clean working tree after all planned commits.
