# Phase 6.x Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the Phase 6.x / Phase 7 candidate review gaps so layered export, font import/rendering, template UX, and verification are implemented end to end.

**Architecture:** Keep Rust authoritative for export planning, Java runtime generation, font parsing/rasterization, and project persistence. Use focused frontend state for loaded font render data and PixiJS rendering, with graceful fallback to native `PIXI.Text` when render data is unavailable. Add regression tests before fixes for each reviewed defect.

**Tech Stack:** Rust 2021, Tauri 2 commands, Svelte 5 runes, TypeScript, PixiJS 8, `image`, `ab_glyph`, `serde`, `cargo test`, `cargo fmt`, `pnpm verify`.

---

## File Structure

- `src-tauri/src/export/mod.rs`: fix generated Forge/NeoForge/Fabric runtime code for overlay textures and animatable sprite textures; add regression tests that inspect generated Java and planned files.
- `src-tauri/src/font/mod.rs`: expose bundled default font data through reusable helpers.
- `src-tauri/src/font/rasterizer.rs`: produce a real PNG atlas for imported TTF/OTF fonts, not only a glyph map.
- `src-tauri/src/project/mod.rs`: extend font model with persistable/renderable atlas data for imported fonts.
- `src-tauri/src/commands.rs`: fix `font_list`, `font_glyph_map`, and add `font_render_data` so the frontend can render glyphs.
- `src/lib/types.ts`: add frontend render-data types for bitmap providers and TTF atlases.
- `src/lib/api.ts`: add `fontRenderData` wrapper and browser mock.
- `src/lib/stores/project.svelte.ts`: load font list and render data after project creation/open/import.
- `src/lib/engine/renderer.ts`: render text with glyph sprite data when available; keep `PIXI.Text` fallback.
- `src/lib/components/NewProjectDialog.svelte`: add visible Custom Grid options while making the current backend limitation explicit in the UI state.
- `src/lib/components/PropertyPanel.svelte`: keep font selector in sync with refreshed font list.
- `src-tauri/src/font/glyph_map.rs`, `src-tauri/src/format/mod.rs`, `src-tauri/src/texture/mod.rs`, `src-tauri/src/templates/mod.rs`: cleanup formatting and unused warnings touched by the Phase 6.x changes.
- `docs/roadmap.md`, `README.md`: adjust claims so docs do not overstate font/glyph or custom-grid parameter support.

---

## Task 1: Export Runtime Regression Tests

**Files:**
- Modify: `src-tauri/src/export/mod.rs`
- Test: `src-tauri/src/export/mod.rs`

- [ ] **Step 1: Add a layered sample project helper**

Inside `#[cfg(test)] mod tests`, add this helper after `sample_project`:

```rust
fn layered_project(target: ModTarget) -> Project {
    let mut project = sample_project(target);
    project.elements.push(Element {
        id: "title".to_string(),
        element_type: ElementType::Text,
        x: 8,
        y: 6,
        width: None,
        height: None,
        size: None,
        asset: None,
        direction: None,
        content: Some("Layered".to_string()),
        font: Some("minecraft:default".to_string()),
        color: Some(0x404040),
        shadow: Some(true),
        animation: None,
        visible: true,
        uv: None,
        layer: Layer::Overlay,
    });
    if let Some(progress) = project.elements.iter_mut().find(|e| e.id == "progress_arrow") {
        progress.layer = Layer::Animatable;
    }
    project
}
```

- [ ] **Step 2: Add failing tests for Fabric overlay and animatable sprite support**

Add these tests:

```rust
#[test]
fn fabric_layered_export_defines_overlay_method_and_loads_overlay_texture() {
    let output_dir = temp_export_dir("fabric-layered");
    let config = ExportConfig {
        mod_id: "testmod".to_string(),
        package: "com.example".to_string(),
        class_name: "LayeredGui".to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
    };

    let files = export_project(&layered_project(ModTarget::Fabric), &config, "fabric").unwrap();
    let layout_path = output_dir.join("src/main/java/com/example/GuiLayout.java");
    let screen_path = output_dir.join("src/main/java/com/example/LayeredGuiScreen.java");
    let layout = read(&layout_path);
    let screen = read(&screen_path);

    assert!(files.iter().any(|path| path.ends_with("layered_gui_overlay.png")));
    assert!(layout.contains("private final Identifier overlay;"));
    assert!(layout.contains("public void renderOverlay(DrawContext context, int left, int top)"));
    assert!(layout.contains("data.textures.overlay"));
    assert!(screen.contains("layout.renderOverlay(context, x, y);"));

    let _ = fs::remove_dir_all(output_dir);
}

#[test]
fn animatable_layer_export_uses_generated_sprite_textures_in_runtime() {
    let output_dir = temp_export_dir("animatable-runtime");
    let config = ExportConfig {
        mod_id: "testmod".to_string(),
        package: "com.example".to_string(),
        class_name: "LayeredGui".to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
    };

    let preview = preview_export(&layered_project(ModTarget::Forge), &config, "forge").unwrap();
    assert!(preview.files.iter().any(|path| path.ends_with("textures/gui/progress_arrow.png")));

    export_project(&layered_project(ModTarget::Forge), &config, "forge").unwrap();
    let layout = read(&output_dir.join("src/main/java/com/example/GuiLayout.java"));

    assert!(layout.contains("ResourceLocation spriteTexture = resource(namespace, element.texture);"));
    assert!(layout.contains("graphics.blit(spriteTexture"));
    assert!(!layout.contains("graphics.fill(x, y, x + Math.round(width * ratio), y + height, 0xFFE9A23B);"));

    let _ = fs::remove_dir_all(output_dir);
}
```

- [ ] **Step 3: Run tests and confirm they fail**

Run:

```bash
cd src-tauri
cargo test export::tests::fabric_layered_export_defines_overlay_method_and_loads_overlay_texture
cargo test export::tests::animatable_layer_export_uses_generated_sprite_textures_in_runtime
```

Expected: both tests fail. The Fabric test should fail because `renderOverlay` is missing in generated Fabric `GuiLayout`. The animatable test should fail because the generated runtime still contains fill-based progress rendering.

- [ ] **Step 4: Commit failing tests**

```bash
git add src-tauri/src/export/mod.rs
git commit -m "test: cover layered export runtime gaps"
```

---

## Task 2: Fix Layered Export Runtime Generation

**Files:**
- Modify: `src-tauri/src/export/mod.rs`
- Test: `src-tauri/src/export/mod.rs`

- [ ] **Step 1: Use the computed `progress_body` in Forge/NeoForge generation**

In `generate_forge_like_layout_java`, replace the hardcoded body inside `public void renderProgress(...)` with `{progress_body}`:

```rust
    public void renderProgress(String animationId, GuiGraphics graphics, int left, int top, float value) {{
        Animation animation = findAnimation(animationId);
        if (animation == null) {{
            return;
        }}
{progress_body}
    }}
```

Keep the existing `progress_body` variable, and make sure the animatable branch includes this exact null guard:

```java
        Element element = findElementByAnimation(animationId);
        if (element == null || element.texture == null) {
            return;
        }
```

- [ ] **Step 2: Add Fabric overlay fields and constructor parameters**

In `generate_fabric_layout_java`, mirror the Forge-like overlay generation using `Identifier`:

```rust
let has_overlay = project.elements.iter().any(|e| e.layer == Layer::Overlay);
let has_animatable = project.elements.iter().any(|e| e.layer == Layer::Animatable);

let overlay_field = if has_overlay {
    "private final Identifier overlay;\n    "
} else {
    ""
};
let overlay_ctor = if has_overlay { ", Identifier overlay" } else { "" };
let overlay_assign = if has_overlay {
    "this.overlay = overlay;\n        "
} else {
    ""
};
```

Use these placeholders in the generated class:

```java
    private final Identifier texture;
    {overlay_field}

    private GuiLayout(List<Element> elements, List<Animation> animations, Identifier texture{overlay_ctor}) {
        this.elements = elements == null ? List.of() : elements;
        this.animations = animations == null ? List.of() : animations;
        this.texture = texture;
        {overlay_assign}
    }
```

- [ ] **Step 3: Load Fabric overlay from layout JSON**

Replace Fabric `load(...)` construction with:

```java
            Identifier bgId = resource(namespace, data.textures.background);
            Identifier overlayId = data.textures.overlay != null ? resource(namespace, data.textures.overlay) : null;
            GuiLayout layout = new GuiLayout(data.elements, data.animations, bgId, overlayId);
            layout.namespace = namespace;
            return layout;
```

Add `private String namespace;` to the Fabric generated class so animatable sprite lookup can use the same namespace as Forge-like output.

- [ ] **Step 4: Define Fabric `renderOverlay`**

Add this generated method when an overlay exists:

```java
    public void renderOverlay(DrawContext context, int left, int top) {
        if (overlay != null) {
            context.drawTexture(overlay, left, top, 0, 0, WIDTH, HEIGHT, WIDTH, HEIGHT);
        }
    }
```

Add this generated method when there is no overlay:

```java
    public void renderOverlay(DrawContext context, int left, int top) {
    }
```

- [ ] **Step 5: Use animatable sprite textures in Fabric `renderProgress`**

Create a Fabric `progress_body` equivalent:

```rust
let progress_body = if has_animatable {
    r#"        Element element = findElementByAnimation(animationId);
        if (element == null || element.texture == null) {{
            return;
        }}
        Identifier spriteTexture = resource(namespace, element.texture);
        int x = left + element.x;
        int y = top + element.y;
        int width = element.widthOrDefault(22);
        int height = element.heightOrDefault(15);
        float ratio = animation.normalize(value);
        switch (animation.directionOrDefault()) {{
            case "right_to_left" -> context.drawTexture(spriteTexture, x + width - Math.round(width * ratio), y, 0, 0, Math.round(width * ratio), height, width, height);
            case "bottom_to_top" -> context.drawTexture(spriteTexture, x, y + height - Math.round(height * ratio), 0, height - Math.round(height * ratio), width, Math.round(height * ratio), width, height);
            case "top_to_bottom" -> context.drawTexture(spriteTexture, x, y, 0, 0, width, Math.round(height * ratio), width, height);
            default -> context.drawTexture(spriteTexture, x, y, 0, 0, Math.round(width * ratio), height, width, height);
        }}"#
} else {
    r#"        for (Element element : elements) {{
            if (!element.isVisible() || !animationId.equals(element.animation)) {{
                continue;
            }}
            int x = left + element.x;
            int y = top + element.y;
            int width = element.widthOrDefault(22);
            int height = element.heightOrDefault(15);
            float ratio = animation.normalize(value);
            switch (animation.directionOrDefault()) {{
                case "right_to_left" -> context.fill(x + width - Math.round(width * ratio), y, x + width, y + height, 0xFFE9A23B);
                case "bottom_to_top" -> context.fill(x, y + height - Math.round(height * ratio), x + width, y + height, 0xFF3B82E9);
                case "top_to_bottom" -> context.fill(x, y, x + width, y + Math.round(height * ratio), 0xFF3B82E9);
                default -> context.fill(x, y, x + Math.round(width * ratio), y + height, 0xFFE9A23B);
            }}
        }}"#
};
```

Use `{progress_body}` inside Fabric `renderProgress(...)`.

- [ ] **Step 6: Add Fabric `findElementByAnimation` when needed**

When `has_animatable` is true, add:

```java
    private Element findElementByAnimation(String animationId) {
        for (Element element : elements) {
            if (animationId.equals(element.animation)) {
                return element;
            }
        }
        return null;
    }
```

- [ ] **Step 7: Run export tests**

Run:

```bash
cd src-tauri
cargo test export::tests
```

Expected: all export tests pass, including the two added in Task 1.

- [ ] **Step 8: Commit export runtime fix**

```bash
git add src-tauri/src/export/mod.rs
git commit -m "fix: generate layered Fabric and sprite animation runtime"
```

---

## Task 3: Backend Font Render Data

**Files:**
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/font/mod.rs`
- Modify: `src-tauri/src/font/rasterizer.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/project/mod.rs`, `src-tauri/src/font/rasterizer.rs`, `src-tauri/src/commands.rs`

- [ ] **Step 1: Extend `FontSource::Ttf` with persistable atlas bytes**

In `src-tauri/src/project/mod.rs`, change `FontSource::Ttf` to:

```rust
Ttf {
    #[serde(default)]
    atlas_png: Vec<u8>,
    font_size: u32,
    glyph_map: GlyphMap,
},
```

Remove the skipped `font_data` field from the persisted model. The rasterizer consumes source font bytes but the project stores only the rendered atlas and glyph map.

- [ ] **Step 2: Update font serialization test**

Change the TTF test fixture to:

```rust
let font = FontAsset {
    id: "minecraft:default".into(),
    source: FontSource::Ttf {
        atlas_png: vec![1, 2, 3],
        font_size: 16,
        glyph_map: glyph_map.clone(),
    },
};
```

Add:

```rust
assert!(value["source"]["atlas_png"].as_array().is_some());
```

- [ ] **Step 3: Make `rasterize_ttf` produce atlas PNG bytes**

In `src-tauri/src/font/rasterizer.rs`, create an atlas image and draw each outlined glyph into it:

```rust
let atlas_width = 256u32;
let atlas_height = 256u32;
let mut atlas = image::RgbaImage::from_pixel(atlas_width, atlas_height, image::Rgba([0, 0, 0, 0]));
```

Inside the existing `if let Some(outlined) = outlined` block, after inserting `GlyphInfo`, draw coverage:

```rust
let draw_x = x_offset;
let draw_y = y_offset;
outlined.draw(|x, y, coverage| {
    let px = draw_x + x;
    let py = draw_y + y;
    if px < atlas_width && py < atlas_height {
        let alpha = (coverage * 255.0).round() as u8;
        atlas.put_pixel(px, py, image::Rgba([255, 255, 255, alpha]));
    }
});
```

After the loop, encode:

```rust
let mut atlas_png = Vec::new();
atlas
    .write_to(&mut std::io::Cursor::new(&mut atlas_png), image::ImageFormat::Png)
    .map_err(|e| format!("Failed to encode font atlas: {e}"))?;
```

Return:

```rust
source: FontSource::Ttf {
    atlas_png,
    font_size,
    glyph_map,
},
```

- [ ] **Step 4: Add rasterizer test for invalid data and atlas presence**

Keep the invalid-data test. Add a positive test only if a stable test font fixture already exists in the repo. If no fixture exists, do not add a brittle binary font to the repo in this task; backend command tests in Step 7 will cover render-data shape with bundled default font.

- [ ] **Step 5: Define a render-data command payload**

In `src-tauri/src/commands.rs`, add helpers near the font commands:

```rust
fn data_url_png(data: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    )
}

fn font_render_data_json(font: &crate::project::FontAsset) -> serde_json::Value {
    match &font.source {
        crate::project::FontSource::Minecraft { providers, glyph_map } => {
            let providers_json: Vec<_> = providers
                .iter()
                .map(|provider| {
                    serde_json::json!({
                        "file": provider.file,
                        "ascent": provider.ascent,
                        "chars": provider.chars,
                        "image_width": provider.image_width,
                        "image_height": provider.image_height,
                        "image_data_url": data_url_png(&provider.image_data),
                    })
                })
                .collect();
            serde_json::json!({
                "id": font.id,
                "source_type": "minecraft",
                "providers": providers_json,
                "glyph_map": glyph_map,
            })
        }
        crate::project::FontSource::Ttf { atlas_png, font_size, glyph_map } => {
            serde_json::json!({
                "id": font.id,
                "source_type": "ttf",
                "font_size": font_size,
                "atlas_data_url": data_url_png(atlas_png),
                "glyph_map": glyph_map,
            })
        }
    }
}
```

- [ ] **Step 6: Fix default font list and glyph map behavior**

Change `font_list` so it always includes `minecraft:default`, then appends project fonts that do not use that ID:

```rust
let mut fonts = vec![serde_json::json!({
    "id": "minecraft:default",
    "source": { "type": "minecraft" }
})];
for f in &session.project.fonts {
    if f.id == "minecraft:default" {
        continue;
    }
    let source_type = match &f.source {
        crate::project::FontSource::Minecraft { .. } => "minecraft",
        crate::project::FontSource::Ttf { .. } => "ttf",
    };
    fonts.push(serde_json::json!({
        "id": f.id,
        "source": { "type": source_type }
    }));
}
```

Change `font_glyph_map` to return `crate::font::load_bundled_font()` when `font_id == "minecraft:default"`.

- [ ] **Step 7: Add `font_render_data` command**

Add:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn font_render_data(
    state: State<AppState>,
    font_id: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    if font_id == "minecraft:default" {
        let font = crate::font::load_bundled_font();
        return Ok(font_render_data_json(&font));
    }

    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;
    let font = session
        .project
        .fonts
        .iter()
        .find(|f| f.id == font_id)
        .ok_or_else(|| format!("Font not found: {font_id}"))?;

    Ok(font_render_data_json(font))
}
```

Register `commands::font_render_data` in `src-tauri/src/lib.rs`.

- [ ] **Step 8: Add backend tests**

Add command-level tests that verify:

```rust
#[test]
fn font_list_always_includes_minecraft_default() {
    let state = test_state_with_project(Project::new("Fonts", 176, 166, ModTarget::Forge));
    let fonts = font_list(tauri::State::from(&state), None).unwrap();
    assert!(fonts.iter().any(|font| font["id"] == "minecraft:default"));
}
```

If constructing `tauri::State` is not available in unit tests, extract the list logic into a pure helper:

```rust
fn font_list_json(project: &Project) -> Vec<serde_json::Value>
```

Then test the helper directly.

Also add:

```rust
#[test]
fn bundled_default_font_render_data_contains_glyphs_and_provider_images() {
    let font = crate::font::load_bundled_font();
    let value = font_render_data_json(&font);
    assert_eq!(value["id"], "minecraft:default");
    assert!(value["glyph_map"].get("A").is_some());
    assert!(value["providers"].as_array().unwrap().iter().any(|p| {
        p["image_data_url"].as_str().unwrap_or("").starts_with("data:image/png;base64,")
    }));
}
```

- [ ] **Step 9: Run backend font tests**

Run:

```bash
cd src-tauri
cargo test font project::tests::font_asset_serialization commands::tests::bundled_default_font_render_data_contains_glyphs_and_provider_images
```

Expected: all selected tests pass.

- [ ] **Step 10: Commit backend font data fix**

```bash
git add src-tauri/src/project/mod.rs src-tauri/src/font/mod.rs src-tauri/src/font/rasterizer.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "fix: expose renderable font data to the editor"
```

---

## Task 4: Frontend Font State And Glyph Rendering

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src/lib/engine/renderer.ts`
- Modify: `src/lib/components/PropertyPanel.svelte`
- Test: `pnpm verify`

- [ ] **Step 1: Add frontend font render data types**

In `src/lib/types.ts`, add:

```ts
export interface FontBitmapProviderRenderData {
  file: string;
  ascent: number;
  chars: string[];
  image_width: number;
  image_height: number;
  image_data_url: string;
}

export interface MinecraftFontRenderData {
  id: string;
  source_type: "minecraft";
  providers: FontBitmapProviderRenderData[];
  glyph_map: Record<string, GlyphInfo>;
}

export interface TtfFontRenderData {
  id: string;
  source_type: "ttf";
  font_size: number;
  atlas_data_url: string;
  glyph_map: Record<string, GlyphInfo>;
}

export type FontRenderData = MinecraftFontRenderData | TtfFontRenderData;
```

- [ ] **Step 2: Add API wrapper and mock**

In `src/lib/api.ts`, import `FontRenderData` and add:

```ts
export async function fontRenderData(fontId: string, projectId?: string): Promise<FontRenderData> {
  const invoke = await getInvoke();
  return invoke("font_render_data", { font_id: fontId, project_id: projectId }) as Promise<FontRenderData>;
}
```

In `mockInvoke`, add:

```ts
case "font_render_data":
  return {
    id: "minecraft:default",
    source_type: "minecraft",
    providers: [],
    glyph_map: {},
  };
```

- [ ] **Step 3: Add font render-data cache to project store**

In `src/lib/stores/project.svelte.ts`, add:

```ts
fontRenderData = $state<Record<string, FontRenderData>>({});
```

Update imports to include `FontRenderData`.

- [ ] **Step 4: Load fonts after new/open/import**

After every `hydrateActiveProject()` call in `newProject`, `openProject`, and `switchProject`, add:

```ts
await this.refreshFonts();
```

Change `importFont` to:

```ts
async importFont(filePath: string) {
  const font = await api.fontImport(filePath, this.activeProjectId ?? undefined);
  const existing = this.fonts.findIndex(f => f.id === font.id);
  if (existing >= 0) this.fonts[existing] = font;
  else this.fonts = [...this.fonts, font];
  await this.loadFontRenderData(font.id);
  this.isDirty = true;
  await this.refreshSessions();
  return font;
}
```

- [ ] **Step 5: Implement render-data loading**

Add methods:

```ts
async refreshFonts() {
  try {
    this.fonts = await api.fontList(this.activeProjectId ?? undefined);
    await Promise.all(this.fonts.map(font => this.loadFontRenderData(font.id)));
  } catch {
    this.fonts = [];
    this.fontRenderData = {};
  }
}

async loadFontRenderData(fontId: string) {
  try {
    const data = await api.fontRenderData(fontId, this.activeProjectId ?? undefined);
    this.fontRenderData = { ...this.fontRenderData, [fontId]: data };
    this.bumpRenderVersion();
  } catch {
    const { [fontId]: _removed, ...rest } = this.fontRenderData;
    this.fontRenderData = rest;
  }
}
```

Reset `fontRenderData = {}` in `clearActiveProject`.

- [ ] **Step 6: Render glyph sprites in PixiJS**

In `src/lib/engine/renderer.ts`, update `drawText`:

```ts
private drawText(el: Element): Container {
  const renderData = project.fontRenderData[el.font ?? "minecraft:default"];
  if (renderData && Object.keys(renderData.glyph_map).length > 0) {
    const glyphText = this.drawGlyphText(el, renderData);
    if (glyphText) return glyphText;
  }

  const container = new Container();
  const text = new Text({
    text: el.content ?? "{text}",
    style: new TextStyle({
      fontSize: 8,
      fill: el.color ?? 0x404040,
      fontFamily: "monospace",
      dropShadow: el.shadow ? { alpha: 0.5, blur: 0, distance: 1, color: 0x000000 } : undefined,
    }),
  });
  text.x = el.x;
  text.y = el.y;
  container.addChild(text);
  return container;
}
```

Add:

```ts
private drawGlyphText(el: Element, font: FontRenderData): Container | null {
  const content = el.content ?? "{text}";
  const container = new Container();
  let cursorX = el.x;
  const baseTexture = this.fontBaseTexture(font);
  if (!baseTexture) return null;

  for (const ch of content) {
    const glyph = font.glyph_map[ch];
    if (!glyph) {
      cursorX += 4;
      continue;
    }
    if (glyph.width === 0 || glyph.height === 0) {
      cursorX += Math.max(1, glyph.width || 4);
      continue;
    }
    const texture = new Texture({
      source: baseTexture.source,
      frame: new Rectangle(glyph.x, glyph.y, glyph.width, glyph.height),
    });
    const sprite = new Sprite(texture);
    sprite.tint = el.color ?? 0x404040;
    sprite.x = cursorX;
    sprite.y = el.y + Math.max(0, 8 - glyph.ascent);
    container.addChild(sprite);
    cursorX += glyph.width;
  }

  return container.children.length > 0 ? container : null;
}
```

Add:

```ts
private fontBaseTexture(font: FontRenderData): Texture | null {
  if (font.source_type === "ttf") {
    return Texture.from(font.atlas_data_url);
  }
  const provider = font.providers.find(p => p.image_data_url);
  return provider ? Texture.from(provider.image_data_url) : null;
}
```

Import `FontRenderData` from `../types`.

- [ ] **Step 7: Keep PropertyPanel selector fed by refreshed font list**

In `PropertyPanel.svelte`, keep the fallback default option but ensure it is not duplicated:

```svelte
{#if !project.fonts.some(font => font.id === "minecraft:default")}
  <option value="minecraft:default">minecraft:default</option>
{/if}
{#each project.fonts as font}
  <option value={font.id}>{font.id}</option>
{/each}
```

- [ ] **Step 8: Run frontend verification**

Run:

```bash
pnpm verify
```

Expected: `svelte-check found 0 errors and 0 warnings`, and Vite build passes. If `node_modules` is missing, run `pnpm install` first after user approval if network access is required.

- [ ] **Step 9: Commit frontend font rendering**

```bash
git add src/lib/types.ts src/lib/api.ts src/lib/stores/project.svelte.ts src/lib/engine/renderer.ts src/lib/components/PropertyPanel.svelte
git commit -m "fix: render imported fonts on the canvas"
```

---

## Task 5: Custom Grid UX And Documentation Honesty

**Files:**
- Modify: `src/lib/components/NewProjectDialog.svelte`
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Test: `pnpm verify`

- [ ] **Step 1: Add local Custom Grid state**

In `NewProjectDialog.svelte`, add:

```ts
let customGridWidth = $state(3);
let customGridHeight = $state(3);
let customGridOutput = $state(true);
let customGridProgress = $state(true);
let customGridInventory = $state(true);
```

- [ ] **Step 2: Render Custom Grid options when selected**

Below the template grid, add:

```svelte
{#if selectedTemplate === "custom_grid"}
  <div class="custom-grid-options">
    <div class="form-row">
      <label for="custom-grid-width">Grid width</label>
      <input id="custom-grid-width" type="number" min="1" max="9" bind:value={customGridWidth} />
    </div>
    <div class="form-row">
      <label for="custom-grid-height">Grid height</label>
      <input id="custom-grid-height" type="number" min="1" max="6" bind:value={customGridHeight} />
    </div>
    <label class="check-row">
      <input type="checkbox" bind:checked={customGridOutput} />
      <span>Output slot</span>
    </label>
    <label class="check-row">
      <input type="checkbox" bind:checked={customGridProgress} />
      <span>Progress arrow</span>
    </label>
    <label class="check-row">
      <input type="checkbox" bind:checked={customGridInventory} />
      <span>Player inventory</span>
    </label>
    <p class="inline-note">
      Current backend creates the default 3x3 custom grid; these choices are shown for the next parameterized-template pass.
    </p>
  </div>
{/if}
```

This keeps the UI honest until backend template parameters are implemented.

- [ ] **Step 3: Style the options compactly**

Add CSS:

```css
.custom-grid-options {
  border: 1px solid #202040;
  border-radius: 6px;
  padding: 10px;
  margin: 10px 0;
  background: #0a0a18;
}

.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #a0b0d0;
  font-size: 12px;
  margin: 6px 0;
}

.inline-note {
  color: #808090;
  font-size: 11px;
  line-height: 1.4;
  margin: 8px 0 0;
}
```

- [ ] **Step 4: Update docs to avoid overclaiming**

In `README.md` and `docs/roadmap.md`, use wording equivalent to:

```markdown
- Custom Grid template is available as a default 3x3 starter layout. Parameterized custom grid generation is planned for a later template pass.
- Font import supports project font selection and canvas preview. Exported Minecraft runtime currently uses the platform text renderer unless custom runtime font support is added.
```

Do not claim Bedrock export, marketplace listing, installer builds, or parameterized custom grids are implemented.

- [ ] **Step 5: Run docs scan**

Run:

```bash
rg -n "parameterized|custom grid|font import|glyph|planned|not implemented|Phase 7" README.md docs
```

Expected: any hits are accurate and distinguish implemented behavior from future work.

- [ ] **Step 6: Run frontend verification**

Run:

```bash
pnpm verify
```

Expected: frontend check/build passes.

- [ ] **Step 7: Commit UX/docs fix**

```bash
git add src/lib/components/NewProjectDialog.svelte README.md docs/roadmap.md
git commit -m "docs: clarify custom grid and font support scope"
```

---

## Task 6: Formatting, Warning Cleanup, And Final Verification

**Files:**
- Modify as needed: `src-tauri/src/font/glyph_map.rs`
- Modify as needed: `src-tauri/src/format/mod.rs`
- Modify as needed: `src-tauri/src/texture/mod.rs`
- Modify as needed: `src-tauri/src/templates/mod.rs`
- Modify as needed: any file reported by `cargo fmt --all -- --check`

- [ ] **Step 1: Run Rust formatter**

Run:

```bash
cd src-tauri
cargo fmt --all
```

Expected: Rust files are formatted. Review the diff to ensure it is only formatting.

- [ ] **Step 2: Remove unused Rust imports and dead helpers**

Apply these specific cleanup rules:

- Remove `use std::collections::HashMap;` from `src-tauri/src/font/glyph_map.rs` if still unused.
- Remove `Layer` from `use crate::project::{Layer, Project};` in `src-tauri/src/format/mod.rs` if still unused.
- Keep `load_bundled_font`, parser functions, and glyph helpers if Task 3 made them reachable through commands; otherwise wire them as described in Task 3 rather than deleting them.
- Keep `composite_atlas` only if existing callers still use it; otherwise remove the wrapper and update tests to call `composite_atlas_for_layer`.

- [ ] **Step 3: Run Rust checks**

Run:

```bash
cd src-tauri
cargo fmt --all -- --check
cargo test
cargo build
```

Expected: formatting passes, all tests pass, and build output has no warnings.

- [ ] **Step 4: Run frontend checks**

Run:

```bash
pnpm verify
```

Expected: `pnpm check` and `pnpm build` pass. If dependencies are missing, install them with `pnpm install` after user approval for network access.

- [ ] **Step 5: Manual smoke checklist**

Run the app and verify:

```bash
pnpm run dev
```

Manual checks:

- New project with `advanced_machine`, `fluid_tank`, `brewing_stand`, `anvil`, and `custom_grid` templates.
- Layer picker changes element layer and canvas hit testing still selects top-layer elements first.
- Export preview for Fabric with overlay and animatable elements lists overlay and sprite PNG files.
- Exported Fabric `GuiLayout.java` contains `renderOverlay`.
- Exported Forge/NeoForge `GuiLayout.java` uses `graphics.blit(spriteTexture` for animatable elements.
- Font selector shows `minecraft:default`.
- Imported TTF appears in the font list and text preview changes to glyph-rendered output or falls back without crashing.

- [ ] **Step 6: Commit cleanup**

```bash
git add src-tauri/src src/lib README.md docs/roadmap.md
git commit -m "chore: verify phase6x review fixes"
```

---

## Plan Self-Review

- **Spec coverage:** Export runtime gaps map to Tasks 1-2. Backend default font and render data map to Task 3. Frontend font loading/rendering maps to Task 4. Custom Grid UI and documentation accuracy map to Task 5. Formatting and verification map to Task 6.
- **Review finding coverage:** Fabric `renderOverlay` compile break is covered by Task 1 test and Task 2 implementation. Unused `progress_body` and fill-based animatable rendering are covered by Task 1 test and Task 2 implementation. Dead bundled font path and missing default glyph map are covered by Task 3. Missing frontend glyph use is covered by Task 4. Missing Custom Grid options and overclaiming risk are covered by Task 5. Formatter and warning failures are covered by Task 6.
- **Placeholder scan:** This plan contains no deferred implementation holes. The Custom Grid backend parameterization is explicitly out of scope for this remediation because the existing backend command has no template-parameter contract; the plan requires honest UI/docs instead of silently pretending parameters work.
- **Type consistency:** `FontRenderData`, `MinecraftFontRenderData`, `TtfFontRenderData`, and `fontRenderData` names are introduced before frontend store and renderer tasks use them. Rust command names use snake_case for Tauri and camelCase wrappers in TypeScript.
