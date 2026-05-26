# Editor UX Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish the editor so new GUIs open with a visible generated background, progress/button atlas regions are selected visually, and the right inspector dock supports fast layer selection plus property editing.

**Architecture:** Keep the Rust project format backward-compatible by using existing optional element fields. Store editor layout and window geometry in the existing app config at `~/.config/mc-gui-crafter/config.json`, exposed through small Tauri commands. Build the frontend dock as a focused Svelte shell around the existing Properties, Layers, and Assets components, and reuse one UV Editor dialog for texture/progress/button icon UVs.

**Tech Stack:** Rust/Tauri 2 backend, Serde config/project models, Svelte 5 runes, PixiJS renderer, existing generated texture/export pipeline.

---

## File Structure

- Modify `src-tauri/src/templates/mod.rs`: ensure every applied template, including `empty`, inserts exactly one generated background texture element at z-order bottom.
- Modify `src-tauri/src/commands.rs`: use the template background path from project creation and add app config/layout/window commands.
- Modify `src-tauri/src/mcp/mod.rs`: keep MCP `project_new` behavior consistent with Tauri project creation.
- Modify `src-tauri/src/config.rs`: add persisted editor layout and app window geometry models, clamping, defaults, and reset helpers.
- Modify `src-tauri/src/lib.rs`: restore window geometry on startup, persist geometry on close/move/resize, and register new config commands.
- Modify `src/lib/types.ts`: add frontend app config/layout/window geometry types.
- Modify `src/lib/api.ts`: add config command wrappers and mock behavior.
- Create `src/lib/stores/layout.svelte.ts`: frontend store for inspector widths, active browser tab, and reset/save behavior.
- Create `src/lib/components/InspectorDock.svelte`: right dock with resizable Properties area and tabbed Layers/Assets browser.
- Modify `src/App.svelte`: replace `sidebar-right` with `InspectorDock` and add global reset shortcuts.
- Modify `src/lib/components/LayerPanel.svelte`: hybrid grouped/collapsible Layers UI.
- Create `src/lib/components/UvEditorDialog.svelte`: reusable asset/UV picker modal.
- Modify `src/lib/components/PropertyPanel.svelte`: progress asset/UV controls and UV Editor launch points for texture/progress/button icon regions.
- Modify `src/lib/components/AssetLibrary.svelte`: compact behavior inside the new dock and compatibility with future UV entry points.
- Modify `src/lib/components/Canvas.svelte`: verify progress UV changes trigger repaint.
- Modify `src/lib/stores/editor.svelte.ts`: keep existing `resetView()` as the `Ctrl+R` implementation hook.
- Modify `docs/roadmap.md`: add completed polish-cycle item and future workspace/dock framework candidate.

## Task 1: Generated Background Elements

**Files:**
- Modify: `src-tauri/src/templates/mod.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/mcp/mod.rs`

- [x] **Step 1: Add failing template tests**

Add tests to `#[cfg(test)] mod tests` in `src-tauri/src/templates/mod.rs`:

```rust
#[test]
fn applying_empty_template_adds_generated_background_element() {
    let mut project = Project::new("Empty", 264, 162, crate::project::ModTarget::Forge);

    apply_template(&mut project, "empty").expect("template applies");

    let backgrounds: Vec<_> = project
        .elements
        .iter()
        .filter(|element| {
            element.element_type == ElementType::Texture
                && element.asset.as_deref() == Some(GENERATED_GUI_PANEL)
        })
        .collect();
    assert_eq!(backgrounds.len(), 1);
    let background = backgrounds[0];
    assert_eq!(background.id, "background");
    assert_eq!(background.x, 0);
    assert_eq!(background.y, 0);
    assert_eq!(background.width, Some(264));
    assert_eq!(background.height, Some(162));
    assert_eq!(background.layer, Layer::Background);
    assert_eq!(project.elements.first().map(|element| element.id.as_str()), Some("background"));
}

#[test]
fn applying_templates_keeps_exactly_one_generated_background_element() {
    for info in list_template_info() {
        let mut project = Project::new(
            "Template",
            info.default_width,
            info.default_height,
            crate::project::ModTarget::Forge,
        );

        apply_template(&mut project, &info.name).expect("template applies");

        let backgrounds: Vec<_> = project
            .elements
            .iter()
            .filter(|element| {
                element.element_type == ElementType::Texture
                    && element.asset.as_deref() == Some(GENERATED_GUI_PANEL)
            })
            .collect();
        assert_eq!(backgrounds.len(), 1, "{} background count", info.name);
        assert_eq!(
            project.elements.first().map(|element| element.id.as_str()),
            Some("background"),
            "{} background z-order",
            info.name
        );
    }
}
```

- [x] **Step 2: Run failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml generated_background_element --locked
```

Expected: FAIL because `empty` currently has no background and some templates use `bg` instead of normalized `background`.

- [x] **Step 3: Add generated background helper**

In `src-tauri/src/templates/mod.rs`, add helper functions near `base_element`:

```rust
fn generated_background_element(width: u32, height: u32) -> Element {
    Element {
        width: Some(width),
        height: Some(height),
        asset: Some(GENERATED_GUI_PANEL.into()),
        ..base_element("background", ElementType::Texture, 0, 0)
    }
}

fn ensure_generated_background_element(project: &mut Project) {
    project.elements.retain(|element| {
        !(element.element_type == ElementType::Texture
            && element.asset.as_deref() == Some(GENERATED_GUI_PANEL))
    });
    project.elements.insert(
        0,
        generated_background_element(project.gui_size.width, project.gui_size.height),
    );
}
```

- [x] **Step 4: Apply background after templates**

In `apply_template`, after assigning template elements/groups/semantic groups and before `add_generated_template_assets(project)?`, call:

```rust
ensure_generated_background_element(project);
```

Also ensure `Project::new` paths without an explicit template get the same default. Add a public helper:

```rust
pub fn apply_generated_defaults(project: &mut Project) -> Result<(), String> {
    ensure_generated_background_element(project);
    add_generated_template_assets(project)
}
```

Then update `apply_template` to call `apply_generated_defaults(project)` instead of `add_generated_template_assets(project)?`.

- [x] **Step 5: Use generated defaults in project creation**

In `src-tauri/src/commands.rs`, after optional template application, call defaults when no template was supplied:

```rust
if let Some(tmpl) = template {
    crate::templates::apply_template(&mut project, &tmpl)?;
} else {
    crate::templates::apply_generated_defaults(&mut project)?;
}
```

In `src-tauri/src/mcp/mod.rs`, mirror the same behavior in `project_new`:

```rust
if let Some(template) = args.get("template").and_then(|value| value.as_str()) {
    templates::apply_template(&mut project, template)?;
} else {
    templates::apply_generated_defaults(&mut project)?;
}
```

- [x] **Step 6: Update existing template tests**

Update `applying_empty_template_preserves_requested_canvas_size` so it asserts both size preservation and one generated background:

```rust
#[test]
fn applying_empty_template_preserves_requested_canvas_size_and_adds_background() {
    let mut project = Project::new("Empty", 240, 120, crate::project::ModTarget::Forge);

    apply_template(&mut project, "empty").expect("template applies");

    assert_eq!(project.gui_size.width, 240);
    assert_eq!(project.gui_size.height, 120);
    assert_eq!(project.elements.len(), 1);
    assert_eq!(project.elements[0].asset.as_deref(), Some(GENERATED_GUI_PANEL));
}
```

- [x] **Step 7: Verify background tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml templates::tests --locked
cargo test --manifest-path src-tauri/Cargo.toml mcp::tests::project_new_empty_template_respects_requested_dimensions --locked
```

Expected: PASS.

- [x] **Step 8: Commit generated backgrounds**

```bash
git add src-tauri/src/templates/mod.rs src-tauri/src/commands.rs src-tauri/src/mcp/mod.rs
git commit -m "fix: apply generated gui background by default"
```

## Task 2: Persisted App Layout And Window Geometry

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`

- [x] **Step 1: Add failing config tests**

In `src-tauri/src/config.rs`, extend tests with:

```rust
#[test]
fn layout_config_defaults_and_clamps_invalid_values() {
    let config = AppConfig {
        mcp_port: Some(49_152),
        editor_layout: Some(EditorLayoutConfig {
            version: 1,
            right_dock_width: 9_999,
            properties_width: 10,
            browser_tab: "unknown".into(),
        }),
        window: Some(WindowConfig {
            width: 100,
            height: 100,
            x: Some(-50_000),
            y: Some(50_000),
        }),
    };

    let clamped = config.clamped();

    assert_eq!(clamped.mcp_port, Some(49_152));
    assert_eq!(clamped.editor_layout.as_ref().unwrap().right_dock_width, DEFAULT_RIGHT_DOCK_WIDTH);
    assert_eq!(clamped.editor_layout.as_ref().unwrap().properties_width, DEFAULT_PROPERTIES_WIDTH);
    assert_eq!(clamped.editor_layout.as_ref().unwrap().browser_tab, "layers");
    assert_eq!(clamped.window.as_ref().unwrap().width, DEFAULT_WINDOW_WIDTH);
    assert_eq!(clamped.window.as_ref().unwrap().height, DEFAULT_WINDOW_HEIGHT);
    assert_eq!(clamped.window.as_ref().unwrap().x, None);
    assert_eq!(clamped.window.as_ref().unwrap().y, None);
}

#[test]
fn reset_layout_preserves_mcp_port_and_clears_window_position() {
    let config = AppConfig {
        mcp_port: Some(49_152),
        editor_layout: Some(EditorLayoutConfig {
            version: 1,
            right_dock_width: 600,
            properties_width: 330,
            browser_tab: "assets".into(),
        }),
        window: Some(WindowConfig {
            width: 1440,
            height: 900,
            x: Some(200),
            y: Some(120),
        }),
    };

    let reset = config.with_reset_ui_layout();

    assert_eq!(reset.mcp_port, Some(49_152));
    assert_eq!(reset.editor_layout, Some(EditorLayoutConfig::default()));
    assert_eq!(reset.window, Some(WindowConfig::default()));
}
```

- [x] **Step 2: Run failing config tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml config::tests --locked
```

Expected: FAIL because the new config structs and helpers do not exist.

- [x] **Step 3: Extend config model**

In `src-tauri/src/config.rs`, add constants and structs:

```rust
pub const DEFAULT_RIGHT_DOCK_WIDTH: u32 = 520;
pub const DEFAULT_PROPERTIES_WIDTH: u32 = 300;
pub const MIN_RIGHT_DOCK_WIDTH: u32 = 360;
pub const MAX_RIGHT_DOCK_WIDTH: u32 = 900;
pub const MIN_PROPERTIES_WIDTH: u32 = 240;
pub const DEFAULT_WINDOW_WIDTH: u32 = 1280;
pub const DEFAULT_WINDOW_HEIGHT: u32 = 800;
pub const MIN_WINDOW_WIDTH: u32 = 900;
pub const MIN_WINDOW_HEIGHT: u32 = 600;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EditorLayoutConfig {
    pub version: u32,
    pub right_dock_width: u32,
    pub properties_width: u32,
    pub browser_tab: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
}
```

Add defaults:

```rust
impl Default for EditorLayoutConfig {
    fn default() -> Self {
        Self {
            version: 1,
            right_dock_width: DEFAULT_RIGHT_DOCK_WIDTH,
            properties_width: DEFAULT_PROPERTIES_WIDTH,
            browser_tab: "layers".into(),
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WINDOW_WIDTH,
            height: DEFAULT_WINDOW_HEIGHT,
            x: None,
            y: None,
        }
    }
}
```

Extend `AppConfig`:

```rust
pub struct AppConfig {
    pub mcp_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_layout: Option<EditorLayoutConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowConfig>,
}
```

- [x] **Step 4: Add clamp/reset helpers**

Add helper methods in `src-tauri/src/config.rs`:

```rust
impl EditorLayoutConfig {
    pub fn clamped(self) -> Self {
        let right_dock_width = if (MIN_RIGHT_DOCK_WIDTH..=MAX_RIGHT_DOCK_WIDTH).contains(&self.right_dock_width) {
            self.right_dock_width
        } else {
            DEFAULT_RIGHT_DOCK_WIDTH
        };
        let max_properties = right_dock_width.saturating_sub(160).max(MIN_PROPERTIES_WIDTH);
        let properties_width = if (MIN_PROPERTIES_WIDTH..=max_properties).contains(&self.properties_width) {
            self.properties_width
        } else {
            DEFAULT_PROPERTIES_WIDTH.min(max_properties)
        };
        let browser_tab = match self.browser_tab.as_str() {
            "layers" | "assets" => self.browser_tab,
            _ => "layers".into(),
        };
        Self {
            version: 1,
            right_dock_width,
            properties_width,
            browser_tab,
        }
    }
}

impl WindowConfig {
    pub fn clamped(self) -> Self {
        let valid_size = self.width >= MIN_WINDOW_WIDTH && self.height >= MIN_WINDOW_HEIGHT;
        let valid_position = self
            .x
            .zip(self.y)
            .is_some_and(|(x, y)| x.abs() < 20_000 && y.abs() < 20_000);
        if valid_size {
            Self {
                width: self.width,
                height: self.height,
                x: valid_position.then_some(self.x).flatten(),
                y: valid_position.then_some(self.y).flatten(),
            }
        } else {
            Self::default()
        }
    }
}

impl AppConfig {
    pub fn clamped(self) -> Self {
        Self {
            mcp_port: self.mcp_port,
            editor_layout: Some(self.editor_layout.unwrap_or_default().clamped()),
            window: Some(self.window.unwrap_or_default().clamped()),
        }
    }

    pub fn with_reset_ui_layout(mut self) -> Self {
        self.editor_layout = Some(EditorLayoutConfig::default());
        self.window = Some(WindowConfig::default());
        self
    }
}
```

Update `load_from_dir` to return `config.clamped()`.

- [x] **Step 5: Add Tauri config commands**

In `src-tauri/src/commands.rs`, import config types:

```rust
use crate::config::{AppConfig, EditorLayoutConfig, WindowConfig};
```

Add commands:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn app_config_get() -> Result<AppConfig, String> {
    crate::config::load()
}

#[tauri::command(rename_all = "snake_case")]
pub fn editor_layout_save(layout: EditorLayoutConfig) -> Result<AppConfig, String> {
    let mut config = crate::config::load()?;
    config.editor_layout = Some(layout.clamped());
    crate::config::save(&config)?;
    Ok(config.clamped())
}

#[tauri::command(rename_all = "snake_case")]
pub fn app_window_save(window: WindowConfig) -> Result<AppConfig, String> {
    let mut config = crate::config::load()?;
    config.window = Some(window.clamped());
    crate::config::save(&config)?;
    Ok(config.clamped())
}

#[tauri::command(rename_all = "snake_case")]
pub fn ui_layout_reset() -> Result<AppConfig, String> {
    let config = crate::config::load()?.with_reset_ui_layout();
    crate::config::save(&config)?;
    Ok(config.clamped())
}
```

Register them in `src-tauri/src/lib.rs` `generate_handler!`.

- [x] **Step 6: Restore and persist window geometry**

In `src-tauri/src/lib.rs`, before starting the MCP server in `.setup`, load config and apply window geometry to the main window:

```rust
let mut app_config = config::load().map_err(Box::<dyn std::error::Error>::from)?;
if let Some(window_config) = app_config.window.clone() {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: window_config.width,
            height: window_config.height,
        }));
        if let Some((x, y)) = window_config.x.zip(window_config.y) {
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        }
    }
}
```

After storing `app_handle`, attach move/resize/close persistence with `on_window_event` or the Tauri v2 event API available in this codebase. Persist through a small helper:

```rust
fn save_main_window_geometry(window: &tauri::WebviewWindow) {
    let Ok(size) = window.outer_size() else { return; };
    let position = window.outer_position().ok();
    if let Ok(mut config) = crate::config::load() {
        config.window = Some(crate::config::WindowConfig {
            width: size.width,
            height: size.height,
            x: position.as_ref().map(|position| position.x),
            y: position.as_ref().map(|position| position.y),
        }
        .clamped());
        let _ = crate::config::save(&config);
    }
}
```

Use a debounce if repeated resize events are noisy; at minimum persist on close.

- [x] **Step 7: Add frontend API types and wrappers**

In `src/lib/types.ts`, add:

```ts
export type BrowserTab = "layers" | "assets";

export interface EditorLayoutConfig {
  version: number;
  right_dock_width: number;
  properties_width: number;
  browser_tab: BrowserTab;
}

export interface WindowConfig {
  width: number;
  height: number;
  x?: number | null;
  y?: number | null;
}

export interface AppConfig {
  mcp_port?: number | null;
  editor_layout?: EditorLayoutConfig | null;
  window?: WindowConfig | null;
}
```

In `src/lib/api.ts`, add wrappers:

```ts
export async function appConfigGet(): Promise<AppConfig> {
  const invoke = await getInvoke();
  return invoke("app_config_get") as Promise<AppConfig>;
}

export async function editorLayoutSave(layout: EditorLayoutConfig): Promise<AppConfig> {
  const invoke = await getInvoke();
  return invoke("editor_layout_save", { layout }) as Promise<AppConfig>;
}

export async function uiLayoutReset(): Promise<AppConfig> {
  const invoke = await getInvoke();
  return invoke("ui_layout_reset") as Promise<AppConfig>;
}
```

Add mock cases returning and updating an in-memory mock config.

- [x] **Step 8: Verify config tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml config::tests --locked
cargo check --manifest-path src-tauri/Cargo.toml --locked
pnpm check
```

Expected: PASS.

- [x] **Step 9: Commit config and window layout persistence**

```bash
git add src-tauri/src/config.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src/lib/types.ts src/lib/api.ts
git commit -m "feat: persist editor layout and window geometry"
```

## Task 3: Inspector Dock Store And Shell

**Files:**
- Create: `src/lib/stores/layout.svelte.ts`
- Create: `src/lib/components/InspectorDock.svelte`
- Modify: `src/App.svelte`

- [x] **Step 1: Create layout store**

Create `src/lib/stores/layout.svelte.ts`:

```ts
import * as api from "../api";
import type { BrowserTab, EditorLayoutConfig } from "../types";

export const DEFAULT_EDITOR_LAYOUT: EditorLayoutConfig = {
  version: 1,
  right_dock_width: 520,
  properties_width: 300,
  browser_tab: "layers",
};

function clampLayout(layout: Partial<EditorLayoutConfig> | null | undefined): EditorLayoutConfig {
  const right = Math.min(900, Math.max(360, Math.round(layout?.right_dock_width ?? DEFAULT_EDITOR_LAYOUT.right_dock_width)));
  const maxProperties = Math.max(240, right - 160);
  const properties = Math.min(maxProperties, Math.max(240, Math.round(layout?.properties_width ?? DEFAULT_EDITOR_LAYOUT.properties_width)));
  const tab: BrowserTab = layout?.browser_tab === "assets" ? "assets" : "layers";
  return {
    version: 1,
    right_dock_width: right,
    properties_width: properties,
    browser_tab: tab,
  };
}

class LayoutStore {
  values = $state<EditorLayoutConfig>({ ...DEFAULT_EDITOR_LAYOUT });
  loaded = $state(false);
  private saveTimer: number | null = null;

  async load() {
    const config = await api.appConfigGet();
    this.values = clampLayout(config.editor_layout);
    this.loaded = true;
  }

  update(changes: Partial<EditorLayoutConfig>) {
    this.values = clampLayout({ ...this.values, ...changes });
    this.scheduleSave();
  }

  async reset() {
    const config = await api.uiLayoutReset();
    this.values = clampLayout(config.editor_layout);
  }

  private scheduleSave() {
    if (this.saveTimer !== null) window.clearTimeout(this.saveTimer);
    this.saveTimer = window.setTimeout(() => {
      this.saveTimer = null;
      void api.editorLayoutSave(this.values);
    }, 250);
  }
}

export const layout = new LayoutStore();
```

- [x] **Step 2: Create InspectorDock component**

Create `src/lib/components/InspectorDock.svelte`:

```svelte
<script lang="ts">
  import PropertyPanel from "./PropertyPanel.svelte";
  import LayerPanel from "./LayerPanel.svelte";
  import AssetLibrary from "./AssetLibrary.svelte";
  import { layout } from "../stores/layout.svelte";
  import type { BrowserTab } from "../types";

  let resizing = $state<"dock" | "properties" | null>(null);
  let startX = 0;
  let startRightWidth = 0;
  let startPropertiesWidth = 0;

  function startResize(kind: "dock" | "properties", event: PointerEvent) {
    resizing = kind;
    startX = event.clientX;
    startRightWidth = layout.values.right_dock_width;
    startPropertiesWidth = layout.values.properties_width;
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", stopResize, { once: true });
  }

  function handlePointerMove(event: PointerEvent) {
    if (resizing === "dock") {
      layout.update({ right_dock_width: startRightWidth - (event.clientX - startX) });
    } else if (resizing === "properties") {
      layout.update({ properties_width: startPropertiesWidth + (event.clientX - startX) });
    }
  }

  function stopResize() {
    resizing = null;
    window.removeEventListener("pointermove", handlePointerMove);
  }

  function setTab(tab: BrowserTab) {
    layout.update({ browser_tab: tab });
  }
</script>

<aside
  class="inspector-dock"
  style={`width: ${layout.values.right_dock_width}px; grid-template-columns: ${layout.values.properties_width}px 1fr;`}
>
  <div
    class="dock-resizer dock-resizer-outer"
    role="separator"
    aria-orientation="vertical"
    tabindex="0"
    onpointerdown={(event) => startResize("dock", event)}
  ></div>
  <section class="dock-pane properties-pane">
    <PropertyPanel />
  </section>
  <div
    class="dock-resizer dock-resizer-inner"
    role="separator"
    aria-orientation="vertical"
    tabindex="0"
    onpointerdown={(event) => startResize("properties", event)}
  ></div>
  <section class="dock-pane browser-pane">
    <div class="browser-tabs" role="tablist" aria-label="Editor browsers">
      <button class:active={layout.values.browser_tab === "layers"} onclick={() => setTab("layers")}>Layers</button>
      <button class:active={layout.values.browser_tab === "assets"} onclick={() => setTab("assets")}>Assets</button>
    </div>
    <div class="browser-content">
      {#if layout.values.browser_tab === "layers"}
        <LayerPanel />
      {:else}
        <AssetLibrary />
      {/if}
    </div>
  </section>
</aside>

<style>
  .inspector-dock {
    position: relative;
    display: grid;
    flex-shrink: 0;
    min-width: 360px;
    max-width: 900px;
    height: 100%;
    background: var(--surface);
    border-left: 1px solid var(--border);
    overflow: hidden;
  }
  .dock-pane { min-width: 0; overflow: auto; }
  .properties-pane { border-right: 1px solid var(--border); }
  .browser-pane { display: flex; flex-direction: column; min-height: 0; }
  .browser-tabs { display: flex; border-bottom: 1px solid var(--border); }
  .browser-tabs button {
    flex: 1;
    background: transparent;
    border: 0;
    color: var(--muted-text);
    padding: 8px 10px;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }
  .browser-tabs button.active {
    background: var(--surface-raised);
    color: var(--text);
  }
  .browser-content { flex: 1; min-height: 0; overflow: auto; }
  .dock-resizer {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    z-index: 5;
  }
  .dock-resizer-outer { left: -3px; }
  .dock-resizer-inner { left: calc(var(--properties-width, 300px) - 3px); }
</style>
```

After writing, replace the inner resizer positioning with a CSS variable in the root style:

```svelte
style={`width: ${layout.values.right_dock_width}px; grid-template-columns: ${layout.values.properties_width}px 1fr; --properties-width: ${layout.values.properties_width}px;`}
```

- [x] **Step 3: Wire dock into App**

In `src/App.svelte`, replace imports:

```svelte
import InspectorDock from "./lib/components/InspectorDock.svelte";
import { layout } from "./lib/stores/layout.svelte";
```

Remove direct imports of `PropertyPanel`, `LayerPanel`, and `AssetLibrary`.

Replace:

```svelte
<aside class="sidebar-right">
  <PropertyPanel />
  <LayerPanel />
  <AssetLibrary />
</aside>
```

with:

```svelte
<InspectorDock />
```

Remove `.sidebar-right` width styles and keep `.sidebar-left` at `220px`.

- [x] **Step 4: Load layout and add shortcuts**

In `src/App.svelte`, add to the existing setup effect:

```ts
void layout.load();
```

Add a keydown effect:

```svelte
$effect(() => {
  function handleKeydown(event: KeyboardEvent) {
    const key = event.key.toLowerCase();
    if (key === "r" && event.ctrlKey && event.shiftKey && event.altKey) {
      event.preventDefault();
      void layout.reset();
      status.success("UI layout reset.");
      return;
    }
    if (key === "r" && event.ctrlKey && !event.shiftKey && !event.altKey) {
      event.preventDefault();
      editor.resetView(project.guiSize);
      status.success("Canvas view reset.");
    }
  }
  window.addEventListener("keydown", handleKeydown);
  return () => window.removeEventListener("keydown", handleKeydown);
});
```

- [x] **Step 5: Verify frontend shell**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS.

- [x] **Step 6: Commit inspector dock shell**

```bash
git add src/lib/stores/layout.svelte.ts src/lib/components/InspectorDock.svelte src/App.svelte
git commit -m "feat: add persisted inspector dock"
```

## Task 4: Hybrid Layers Panel

**Files:**
- Modify: `src/lib/components/LayerPanel.svelte`

- [x] **Step 1: Add grouped layer model**

Replace the top script in `LayerPanel.svelte` with a grouped model:

```svelte
<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import type { Element, Group, SemanticGroup } from "../types";

  type LayerRow =
    | { kind: "group"; id: string; label: string; meta: string; elements: Element[] }
    | { kind: "element"; element: Element; meta: string };

  let collapsedGroups = $state<Set<string>>(new Set());

  function displayId(id: string): string {
    return id.length > 26 ? `${id.slice(0, 23)}...` : id;
  }

  function elementMeta(el: Element): string {
    const layer = el.layer ?? "background";
    if (el.type === "slot" || el.type === "virtual_slot_cell") {
      return `${el.type} · ${layer}${el.slot_role ? ` · ${el.slot_role}` : ""}${el.slot_index !== undefined && el.slot_index !== null ? ` · #${el.slot_index}` : ""}`;
    }
    if (el.type === "progress") {
      return `${el.type} · ${layer}${el.direction ? ` · ${el.direction}` : ""}`;
    }
    if (el.width || el.height) {
      return `${el.type} · ${layer} · ${el.width ?? "?"}x${el.height ?? "?"}`;
    }
    return `${el.type} · ${layer} · ${el.x},${el.y}`;
  }

  function groupMeta(group: Group | SemanticGroup, count: number): string {
    if ("kind" in group) return `${group.kind} · ${count} elements`;
    return `${count} elements`;
  }

  function toggleGroup(id: string) {
    const next = new Set(collapsedGroups);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    collapsedGroups = next;
  }

  function groupedRows(): LayerRow[] {
    const consumed = new Set<string>();
    const rows: LayerRow[] = [];
    const reversed = [...project.elements].reverse();
    const groupsById = new Map(project.groups.map(group => [group.id, group]));
    const semanticGroups = project.semanticGroups;

    for (const semantic of semanticGroups) {
      const elements = project.elements.filter(element => element.inventory_group === semantic.id);
      if (elements.length >= 3) {
        for (const element of elements) consumed.add(element.id);
        rows.push({ kind: "group", id: semantic.id, label: semantic.id, meta: groupMeta(semantic, elements.length), elements });
      }
    }

    for (const group of project.groups) {
      if (rows.some(row => row.kind === "group" && row.id === group.id)) continue;
      const elements = group.elements.map(id => project.elementById(id)).filter(element => element !== undefined);
      if (elements.length >= 3) {
        for (const element of elements) consumed.add(element.id);
        rows.push({ kind: "group", id: group.id, label: group.id, meta: groupMeta(group, elements.length), elements });
      }
    }

    for (const element of reversed) {
      if (consumed.has(element.id)) continue;
      rows.push({ kind: "element", element, meta: elementMeta(element) });
    }

    return rows;
  }

  let rows = $derived.by(() => {
    void project.revision;
    void project.elements.length;
    void project.groups.length;
    void project.semanticGroups.length;
    void editor.selectionRevision;
    return groupedRows();
  });
```

Keep the existing `toggleVisibility`, `selectedGroupIds`, `selectedElementId`, and `selectedCount` logic below these helpers. Do not remove the current visibility/reorder behavior while changing the row layout.

- [x] **Step 2: Replace layer markup**

Add a local row snippet before the list markup:

```svelte
{#snippet elementRow(el: Element, nested = false)}
  {@const idx = project.elements.indexOf(el)}
  {@const isLast = idx === 0}
  {@const isFirst = idx === project.elements.length - 1}
  <div class="layer-row" class:nested>
    <button class="layer-item" class:selected={selectedElementId === el.id} class:hidden-el={!(el.visible ?? true)} onclick={() => editor.selectElement(el.id)}>
      <span class="layer-icon">{iconForElement(el)}</span>
      <span class="layer-text">
        <span class="layer-title">{displayId(el.id)}</span>
        <span class="layer-meta">{elementMeta(el)}</span>
      </span>
    </button>
    <div class="layer-actions">
      <button class="reorder-btn" disabled={isFirst} onclick={() => project.moveElementDown(el.id)} title="Move down">↓</button>
      <button class="reorder-btn" disabled={isLast} onclick={() => project.moveElementUp(el.id)} title="Move up">↑</button>
      <button class="visibility-btn" onclick={() => toggleVisibility(el)} title={el.visible === false ? "Show" : "Hide"}>{el.visible === false ? "◌" : "●"}</button>
    </div>
  </div>
{/snippet}
```

Then replace the list markup with:

```svelte
<div class="layer-list">
  {#each rows as row}
    {#if row.kind === "group"}
      <div class="group-row">
        <button class="group-main" onclick={() => toggleGroup(row.id)}>
          <span class="disclosure">{collapsedGroups.has(row.id) ? "▸" : "▾"}</span>
          <span class="group-title">{displayId(row.label)}</span>
          <span class="group-meta">{row.meta}</span>
        </button>
      </div>
      {#if !collapsedGroups.has(row.id)}
        {#each row.elements as el (el.id)}
          {@render elementRow(el, true)}
        {/each}
      {/if}
    {:else}
      {@render elementRow(row.element)}
    {/if}
  {/each}
</div>
```

Add:

```ts
function iconForElement(el: Element): string {
  switch (el.type) {
    case "slot": return "◻";
    case "texture": return "▣";
    case "progress": return "→";
    case "text": return "T";
    case "fluid_tank": return "▥";
    case "energy_bar": return "⚡";
    case "button": return "▭";
    case "toggle_button": return "◉";
    default: return "•";
  }
}
```

- [x] **Step 3: Update layer styles**

Replace `max-height: 260px` with full-height scrolling:

```css
.layers {
  padding: 8px;
  min-height: 0;
}
.layer-list {
  display: flex;
  flex-direction: column;
  gap: 3px;
  overflow-y: auto;
  max-height: none;
}
.group-main,
.layer-item {
  min-width: 0;
}
.group-main {
  width: 100%;
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr);
  gap: 6px;
  align-items: center;
  background: var(--surface-raised);
  border: 1px solid var(--border);
  color: var(--text);
  padding: 5px 6px;
  text-align: left;
  cursor: pointer;
  font: inherit;
}
.group-title,
.layer-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.group-meta,
.layer-meta {
  color: var(--muted-text);
  font-size: 10px;
}
.layer-item {
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr);
  align-items: center;
  min-height: 38px;
}
.layer-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.layer-row.nested {
  padding-left: 12px;
}
```

- [x] **Step 4: Verify Layers**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS.

- [x] **Step 5: Commit hybrid Layers**

```bash
git add src/lib/components/LayerPanel.svelte
git commit -m "feat: group dense layer lists"
```

## Task 5: Reusable UV Editor Dialog

**Files:**
- Create: `src/lib/components/UvEditorDialog.svelte`
- Modify: `src/lib/components/PropertyPanel.svelte`

- [x] **Step 1: Create UV editor component**

Create `src/lib/components/UvEditorDialog.svelte`:

```svelte
<script lang="ts">
  import { assetDataUrls } from "../stores/project.svelte";
  import type { UvRect } from "../types";

  let {
    title,
    assets,
    asset,
    uv = null,
    onapply,
    onclear,
    onclose,
  }: {
    title: string;
    assets: string[];
    asset: string | null;
    uv?: UvRect | null;
    onapply: (asset: string, uv: UvRect | null) => void;
    onclear: () => void;
    onclose: () => void;
  } = $props();

  let selectedAsset = $state(asset ?? assets[0] ?? "");
  let rect = $state<UvRect>({
    x: uv?.x ?? 0,
    y: uv?.y ?? 0,
    width: uv?.width ?? 16,
    height: uv?.height ?? 16,
  });
  let dragStart: { x: number; y: number } | null = null;
  let imageWrapEl: HTMLDivElement | undefined = $state();
  let imageNaturalWidth = $state(1);
  let imageNaturalHeight = $state(1);
  let zoom = $state(4);

  let dataUrl = $derived(selectedAsset ? assetDataUrls.get(selectedAsset) : undefined);

  function clampRect(next: UvRect): UvRect {
    const x = Math.max(0, Math.min(imageNaturalWidth - 1, Math.round(next.x)));
    const y = Math.max(0, Math.min(imageNaturalHeight - 1, Math.round(next.y)));
    const width = Math.max(1, Math.min(imageNaturalWidth - x, Math.round(next.width)));
    const height = Math.max(1, Math.min(imageNaturalHeight - y, Math.round(next.height)));
    return { x, y, width, height };
  }

  function updateRect(changes: Partial<UvRect>) {
    rect = clampRect({ ...rect, ...changes });
  }

  function imagePoint(event: PointerEvent): { x: number; y: number } {
    if (!imageWrapEl) return { x: 0, y: 0 };
    const bounds = imageWrapEl.getBoundingClientRect();
    return {
      x: Math.floor((event.clientX - bounds.left) / zoom),
      y: Math.floor((event.clientY - bounds.top) / zoom),
    };
  }

  function startDrag(event: PointerEvent) {
    dragStart = imagePoint(event);
    updateRect({ x: dragStart.x, y: dragStart.y, width: 1, height: 1 });
    window.addEventListener("pointermove", drag);
    window.addEventListener("pointerup", stopDrag, { once: true });
  }

  function drag(event: PointerEvent) {
    if (!dragStart) return;
    const current = imagePoint(event);
    updateRect({
      x: Math.min(dragStart.x, current.x),
      y: Math.min(dragStart.y, current.y),
      width: Math.abs(current.x - dragStart.x) + 1,
      height: Math.abs(current.y - dragStart.y) + 1,
    });
  }

  function stopDrag() {
    dragStart = null;
    window.removeEventListener("pointermove", drag);
  }

  function apply() {
    if (!selectedAsset) return;
    onapply(selectedAsset, clampRect(rect));
  }
</script>

<div class="uv-overlay" role="presentation" onclick={(event) => event.target === event.currentTarget && onclose()}>
  <div class="uv-dialog" role="dialog" aria-modal="true" aria-labelledby="uv-editor-title">
    <header>
      <h2 id="uv-editor-title">{title}</h2>
      <button onclick={onclose} aria-label="Close">×</button>
    </header>

    <div class="uv-controls">
      <label>
        Asset
        <select bind:value={selectedAsset}>
          {#each assets as name (name)}
            <option value={name}>{name}</option>
          {/each}
        </select>
      </label>
      <label>
        Zoom
        <input type="range" min="1" max="12" bind:value={zoom} />
      </label>
    </div>

    <div class="uv-body">
      {#if dataUrl}
        <div class="image-stage">
          <div bind:this={imageWrapEl} class="image-wrap" style={`width:${imageNaturalWidth * zoom}px;height:${imageNaturalHeight * zoom}px`} onpointerdown={startDrag}>
            <img
              src={dataUrl}
              alt={selectedAsset}
              style={`width:${imageNaturalWidth * zoom}px;height:${imageNaturalHeight * zoom}px`}
              onload={(event) => {
                imageNaturalWidth = event.currentTarget.naturalWidth;
                imageNaturalHeight = event.currentTarget.naturalHeight;
                rect = clampRect(rect);
              }}
            />
            <div
              class="selection"
              style={`left:${rect.x * zoom}px;top:${rect.y * zoom}px;width:${rect.width * zoom}px;height:${rect.height * zoom}px`}
            ></div>
          </div>
        </div>
      {:else}
        <div class="missing-preview">No preview data for selected asset.</div>
      {/if}

      <div class="numeric-grid">
        <label>X <input type="number" min="0" value={rect.x} oninput={(event) => updateRect({ x: Number(event.currentTarget.value) })} /></label>
        <label>Y <input type="number" min="0" value={rect.y} oninput={(event) => updateRect({ y: Number(event.currentTarget.value) })} /></label>
        <label>W <input type="number" min="1" value={rect.width} oninput={(event) => updateRect({ width: Number(event.currentTarget.value) })} /></label>
        <label>H <input type="number" min="1" value={rect.height} oninput={(event) => updateRect({ height: Number(event.currentTarget.value) })} /></label>
      </div>
    </div>

    <footer>
      <button onclick={onclear}>Clear</button>
      <button onclick={onclose}>Cancel</button>
      <button class="primary" onclick={apply} disabled={!selectedAsset}>Apply</button>
    </footer>
  </div>
</div>

<style>
  .uv-overlay {
    position: fixed;
    inset: 0;
    z-index: 1200;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
  }
  .uv-dialog {
    width: min(860px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  header, footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 10px;
    border-bottom: 1px solid var(--border);
  }
  footer {
    border-top: 1px solid var(--border);
    border-bottom: 0;
    justify-content: flex-end;
  }
  h2 { font-size: 13px; margin: 0; }
  .uv-controls {
    display: grid;
    grid-template-columns: 1fr 160px;
    gap: 10px;
    padding: 10px;
    border-bottom: 1px solid var(--border);
  }
  .uv-body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 170px;
    gap: 10px;
    padding: 10px;
    min-height: 0;
    overflow: hidden;
  }
  .image-stage {
    overflow: auto;
    background: var(--app-bg);
    border: 1px solid var(--border);
    min-height: 340px;
  }
  .image-wrap {
    position: relative;
    image-rendering: pixelated;
  }
  img {
    display: block;
    image-rendering: pixelated;
    user-select: none;
    pointer-events: none;
  }
  .selection {
    position: absolute;
    border: 1px solid var(--accent);
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    pointer-events: none;
  }
  .numeric-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 8px;
    align-content: start;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--muted-text);
    font-size: 11px;
  }
  input, select, button {
    font: inherit;
  }
  input, select {
    background: var(--app-bg);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 4px 6px;
  }
  button {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text);
    padding: 5px 9px;
    cursor: pointer;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .missing-preview {
    color: var(--muted-text);
    padding: 16px;
  }
</style>
```

- [x] **Step 2: Wire dialog state into PropertyPanel**

In `src/lib/components/PropertyPanel.svelte`, import:

```ts
import UvEditorDialog from "./UvEditorDialog.svelte";
import type { CodegenMode, Element, SlotRole, UvRect } from "../types";
```

Add state:

```ts
type UvTarget = "uv" | "icon_uv";
let uvEditorTarget = $state<UvTarget | null>(null);
```

Add helpers:

```ts
function openUvEditor(target: UvTarget) {
  uvEditorTarget = target;
}

function applyUvSelection(asset: string, uv: UvRect | null) {
  if (!selectedEl || !uvEditorTarget) return;
  if (uvEditorTarget === "icon_uv") {
    updateSelectedElement({ icon: asset, icon_uv: uv });
  } else {
    updateSelectedElement({ asset, uv });
  }
  uvEditorTarget = null;
}

function clearUvSelection() {
  if (!selectedEl || !uvEditorTarget) return;
  if (uvEditorTarget === "icon_uv") {
    updateSelectedElement({ icon_uv: null });
  } else {
    updateSelectedElement({ uv: null });
  }
  uvEditorTarget = null;
}
```

- [x] **Step 3: Add texture/progress asset controls**

Change the texture-only block:

```svelte
{#if selectedEl.type === "texture"}
```

to:

```svelte
{#if selectedEl.type === "texture" || selectedEl.type === "progress"}
```

Change label text to:

```svelte
<label for="prop-asset">{selectedEl.type === "progress" ? "Source" : "Texture"}</label>
```

Add an `Edit UV...` button inside the UV section:

```svelte
<button class="secondary-btn" onclick={() => openUvEditor("uv")}>
  Pick Region...
</button>
```

- [x] **Step 4: Add icon UV dialog button**

In the button icon section, add:

```svelte
<button class="secondary-btn" onclick={() => openUvEditor("icon_uv")} disabled={project.assets.length === 0}>
  Pick Icon Region...
</button>
```

Keep existing numeric fields as precise fallback.

- [x] **Step 5: Render UV dialog**

Before the closing `</aside>` or after it, render:

```svelte
{#if selectedEl && uvEditorTarget}
  <UvEditorDialog
    title={uvEditorTarget === "icon_uv" ? "Pick Button Icon Region" : "Pick Texture Region"}
    assets={project.assets}
    asset={uvEditorTarget === "icon_uv" ? selectedEl.icon ?? null : selectedEl.asset ?? null}
    uv={uvEditorTarget === "icon_uv" ? selectedEl.icon_uv ?? null : selectedEl.uv ?? null}
    onapply={applyUvSelection}
    onclear={clearUvSelection}
    onclose={() => uvEditorTarget = null}
  />
{/if}
```

- [x] **Step 6: Verify UV editor**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS.

- [x] **Step 7: Commit UV editor**

```bash
git add src/lib/components/UvEditorDialog.svelte src/lib/components/PropertyPanel.svelte
git commit -m "feat: add reusable uv editor"
```

## Task 6: Progress Texture Rendering And Properties Polish

**Files:**
- Modify: `src/lib/engine/renderer.ts`
- Modify: `src/lib/components/PropertyPanel.svelte`
- Modify: `src/lib/components/Canvas.svelte`

- [x] **Step 1: Add a renderer regression test or manual fixture note**

If the project already has renderer tests, add a focused test that a progress element with `asset` and `uv` creates a texture-backed display object. If renderer tests are not practical because Pixi needs browser APIs, add a short manual fixture note to this task's verification output and cover it with the existing `pnpm build` plus interactive dev check after implementation.

- [x] **Step 2: Update progress drawing to use asset and UV**

In the progress rendering branch, make it follow this order:

```ts
case "progress":
  return this.drawProgress(el);
```

Implement or update `drawProgress(el: Element)`:

```ts
private drawProgress(el: Element): Container {
  return el.asset ? this.drawTexture(el) : this.drawProgressFallback(el);
}
```

Rename the current placeholder body of `drawProgress` to `drawProgressFallback(el: Element): Container`; do not delete it, because projects without a progress asset should still render something visible.

If `drawTexture(el)` already handles `uv`, reuse it. If not, extend `drawTexture(el)` to create a `Texture` from `el.uv` via Pixi `Rectangle` before sizing the sprite. Keep nearest-neighbor/pixelated behavior consistent with existing asset rendering.

Also update generated texture fallback handling so `textures/generated/progress_arrow.png` draws an arrow-shaped fallback rather than a generic missing texture rectangle when no data URL has been loaded yet. Extract the existing arrow drawing into a helper such as:

```ts
private drawProgressArrow(g: Graphics, x: number, y: number, width: number, height: number, direction = "left_to_right") {
  // Move the current arrow graphics from drawProgressFallback here.
}
```

Then call that helper from both `drawProgressFallback` and the generated texture fallback branch for `progress_arrow.png`.

- [x] **Step 3: Ensure Canvas observes progress asset and UV**

In `src/lib/components/Canvas.svelte`, verify the reactive loop already touches:

```ts
void element.asset;
void element.uv?.x;
void element.uv?.y;
void element.uv?.width;
void element.uv?.height;
```

If missing, add those reads.

- [x] **Step 4: Add progress default asset**

In `ProjectStore.addElement()` in `src/lib/stores/project.svelte.ts`, add:

```ts
if (type === "progress") {
  element.width ??= 22;
  element.height ??= 15;
  element.asset ??= "textures/generated/progress_arrow.png";
  element.layer ??= "animatable";
  element.direction ??= "left_to_right";
}
```

If the renderer click placement already creates progress elements elsewhere, ensure it benefits from `addElement`.

- [x] **Step 5: Verify progress UX**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS.

- [x] **Step 6: Commit progress texture UX**

```bash
git add src/lib/engine/renderer.ts src/lib/components/Canvas.svelte src/lib/stores/project.svelte.ts src/lib/components/PropertyPanel.svelte
git commit -m "feat: edit progress texture regions"
```

## Task 7: Roadmap And Final Verification

**Files:**
- Modify: `docs/roadmap.md`
- No planned source edits unless verification finds a real issue.

- [ ] **Step 1: Update roadmap**

In `docs/roadmap.md`, add a checked Phase 6.x item:

```markdown
- [x] Editor UX polish: generated background elements, progress texture editing, persisted inspector dock, grouped Layers, reusable UV Editor, and UI/window layout reset
```

Add a future unchecked candidate:

```markdown
- [ ] Workspace/dock framework: movable and pinnable editor panels, workspace profiles, richer Asset/UV panes, and optional stacked/pinned Layers and Assets behavior
```

- [ ] **Step 2: Commit roadmap**

```bash
git add docs/roadmap.md
git commit -m "docs: update editor ux roadmap"
```

- [ ] **Step 3: Run full automated verification**

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
- working tree is clean except `.superpowers/` if the brainstorming visual companion directory remains untracked.

- [ ] **Step 4: Run desktop smoke**

Start the app with the Wayland workaround:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11 pnpm tauri dev
```

Smoke steps:

1. Create a new `176x166` empty GUI.
2. Verify the generated panel background is visible immediately and appears in Layers.
3. Select an element in Layers and edit it in Properties without switching panels.
4. Resize the inspector dock and Properties split.
5. Switch Layers/Assets tab, restart, and verify layout restores.
6. Move/resize the app window, close/reopen, and verify geometry restores.
7. Press `Ctrl+Shift+Alt+R`, restart if needed, and verify dock/window defaults return.
8. Press `Ctrl+R` and verify only the canvas view recenters.
9. Create a progress element and choose a source/UV region.
10. Create a button and choose an atlas icon region.

- [ ] **Step 5: Final status**

Run:

```bash
git status --short
git log --oneline -12
```

Expected: source tree clean except untracked `.superpowers/` visual companion artifacts.
