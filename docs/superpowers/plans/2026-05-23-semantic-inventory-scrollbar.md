# Semantic Inventory and Modular Codegen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add semantic inventory metadata, a scrollable inventory template, and configurable simple/modular code generation exposed through the UI and MCP.

**Architecture:** Extend the existing `.mcgui` project schema with optional semantic fields so old projects continue to load. Keep the first implementation focused: generated textures and layout JSON become semantic-aware, while code generation gains a project/export mode switch and a lightweight modular registry file without generating complete container/menu business logic.

**Tech Stack:** Rust/Tauri 2 backend, Serde project format, PNG composition in `src-tauri/src/texture`, Svelte 5 frontend, Pixi renderer, local JSON-RPC MCP server.

---

## File Structure

- Modify `src-tauri/src/project/mod.rs`: Rust schema for `ElementType`, slot roles, semantic groups, and export settings.
- Modify `src/lib/types.ts`: matching TypeScript schema.
- Modify `src-tauri/src/templates/mod.rs`: generated scrollbar asset constants and the `scrollable_inventory_machine` template.
- Modify `src-tauri/src/texture/mod.rs`: generated scrollbar texture functions and background baking for scrollbars/virtual cells.
- Modify `src/lib/engine/renderer.ts`: Pixi rendering for scrollbar and virtual slot cell preview.
- Modify `src/lib/stores/project.svelte.ts`: hydrate and update semantic groups/export settings.
- Modify `src/lib/api.ts`: frontend API and mock support for export settings.
- Modify `src/lib/components/PropertyPanel.svelte`: controls for slot semantics, scrollbar semantics, and project export mode.
- Modify `src/lib/components/ExportDialog.svelte`: per-export simple/modular override.
- Modify `src-tauri/src/export/mod.rs`: layout JSON, preview warnings, export settings, and modular registry generation.
- Modify `src-tauri/src/mcp/mod.rs`: MCP schemas/tools for semantic fields and export settings.
- Add or update tests in the same Rust modules using existing inline `#[cfg(test)]` patterns.

## Task 1: Rust Project Schema

**Files:**
- Modify: `src-tauri/src/project/mod.rs`

- [ ] **Step 1: Add failing serialization tests**

Add tests near the existing project tests in `src-tauri/src/project/mod.rs`:

```rust
#[test]
fn project_defaults_missing_semantic_fields() {
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
    assert!(project.semantic_groups.is_empty());
    assert_eq!(project.export_settings.codegen_mode, CodegenMode::Simple);
    assert!(project.export_settings.generate_runtime_helpers);
    assert!(!project.export_settings.generate_semantic_registry);
}

#[test]
fn element_semantics_round_trip() {
    let element = Element {
        id: "buffer_slot_0".into(),
        element_type: ElementType::Slot,
        x: 34,
        y: 54,
        width: None,
        height: None,
        size: Some(18),
        asset: None,
        direction: None,
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: None,
        visible: true,
        uv: None,
        layer: Layer::Background,
        slot_role: Some(SlotRole::ScrollableInventory),
        slot_index: Some(0),
        inventory_group: Some("machine_buffer".into()),
        scroll_binding: Some("buffer_scroll".into()),
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
    };

    let value = serde_json::to_value(&element).unwrap();
    assert_eq!(value["slot_role"], "scrollable_inventory");
    assert_eq!(value["inventory_group"], "machine_buffer");
    let decoded: Element = serde_json::from_value(value).unwrap();
    assert_eq!(decoded, element);
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_defaults_missing_semantic_fields element_semantics_round_trip --locked`

Expected: FAIL because `semantic_groups`, `export_settings`, `CodegenMode`, and semantic element fields do not exist.

- [ ] **Step 3: Add enums and defaults**

Add after `ElementType`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SlotRole {
    #[serde(alias = "Machine")]
    Machine,
    #[serde(alias = "PlayerInventory")]
    PlayerInventory,
    #[serde(alias = "Hotbar")]
    Hotbar,
    #[serde(alias = "ScrollableInventory")]
    ScrollableInventory,
    #[serde(alias = "VirtualStorage")]
    VirtualStorage,
    #[serde(alias = "Upgrade")]
    Upgrade,
    #[serde(alias = "UpgradeSettings")]
    UpgradeSettings,
    #[serde(alias = "Filter")]
    Filter,
    #[serde(alias = "Ghost")]
    Ghost,
    #[serde(alias = "Offhand")]
    Offhand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticGroupKind {
    FixedSlots,
    VirtualSlotGrid,
    PlayerInventory,
    Hotbar,
    UpgradeSlots,
    UpgradePanel,
    SearchField,
    ControlButtons,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticGroup {
    pub id: String,
    pub kind: SemanticGroupKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_rows: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_rows: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_binding: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dynamic_height: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodegenMode {
    Simple,
    Modular,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectExportSettings {
    pub codegen_mode: CodegenMode,
    #[serde(default = "default_true")]
    pub generate_runtime_helpers: bool,
    #[serde(default)]
    pub generate_semantic_registry: bool,
}

impl Default for ProjectExportSettings {
    fn default() -> Self {
        Self {
            codegen_mode: CodegenMode::Simple,
            generate_runtime_helpers: true,
            generate_semantic_registry: false,
        }
    }
}
```

- [ ] **Step 4: Extend `ElementType`, `Element`, and `Project`**

Add variants to `ElementType`:

```rust
#[serde(alias = "Scrollbar")]
Scrollbar,
#[serde(alias = "Button")]
Button,
#[serde(alias = "ToggleButton")]
ToggleButton,
#[serde(alias = "TextInput")]
TextInput,
#[serde(alias = "Tab")]
Tab,
#[serde(alias = "Panel")]
Panel,
#[serde(alias = "VirtualSlotCell")]
VirtualSlotCell,
```

Add fields to `Element` after `layer`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub slot_role: Option<SlotRole>,
#[serde(skip_serializing_if = "Option::is_none")]
pub slot_index: Option<u32>,
#[serde(skip_serializing_if = "Option::is_none")]
pub inventory_group: Option<String>,
#[serde(skip_serializing_if = "Option::is_none")]
pub scroll_binding: Option<String>,
#[serde(skip_serializing_if = "Option::is_none")]
pub scroll_min: Option<u32>,
#[serde(skip_serializing_if = "Option::is_none")]
pub scroll_max: Option<u32>,
#[serde(skip_serializing_if = "Option::is_none")]
pub visible_rows: Option<u32>,
#[serde(skip_serializing_if = "Option::is_none")]
pub total_rows: Option<u32>,
#[serde(skip_serializing_if = "Option::is_none")]
pub columns: Option<u32>,
#[serde(skip_serializing_if = "Option::is_none")]
pub target_group: Option<String>,
#[serde(skip_serializing_if = "Option::is_none")]
pub binding: Option<String>,
#[serde(skip_serializing_if = "Option::is_none")]
pub dock: Option<String>,
#[serde(skip_serializing_if = "Option::is_none")]
pub open_width: Option<u32>,
#[serde(skip_serializing_if = "Option::is_none")]
pub open_height: Option<u32>,
```

Add fields to `Project`:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub semantic_groups: Vec<SemanticGroup>,
#[serde(default)]
pub export_settings: ProjectExportSettings,
```

Update every explicit `Element { ... }` constructor in Rust by adding the new fields with `None`. Prefer a helper later if repetition becomes error-prone, but keep this task mechanical.

- [ ] **Step 5: Run schema tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_defaults_missing_semantic_fields element_semantics_round_trip --locked`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/project/mod.rs
git commit -m "feat: add semantic project schema"
```

## Task 2: TypeScript Schema and Store Support

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Extend frontend types**

In `src/lib/types.ts`, replace `ElementType` with:

```ts
export type ElementType =
  | "texture"
  | "slot"
  | "progress"
  | "text"
  | "fluid_tank"
  | "energy_bar"
  | "scrollbar"
  | "button"
  | "toggle_button"
  | "text_input"
  | "tab"
  | "panel"
  | "virtual_slot_cell";
```

Add:

```ts
export type SlotRole =
  | "machine"
  | "player_inventory"
  | "hotbar"
  | "scrollable_inventory"
  | "virtual_storage"
  | "upgrade"
  | "upgrade_settings"
  | "filter"
  | "ghost"
  | "offhand";

export type SemanticGroupKind =
  | "fixed_slots"
  | "virtual_slot_grid"
  | "player_inventory"
  | "hotbar"
  | "upgrade_slots"
  | "upgrade_panel"
  | "search_field"
  | "control_buttons";

export interface SemanticGroup {
  id: string;
  kind: SemanticGroupKind;
  columns?: number;
  visible_rows?: number;
  total_rows?: number;
  slot_count?: number;
  data_source?: string;
  scroll_binding?: string;
  dynamic_height?: boolean;
}

export type CodegenMode = "simple" | "modular";

export interface ProjectExportSettings {
  codegen_mode: CodegenMode;
  generate_runtime_helpers: boolean;
  generate_semantic_registry: boolean;
}
```

Extend `Element` with the matching optional fields from Task 1. Extend `ProjectData` with:

```ts
semantic_groups?: SemanticGroup[];
export_settings?: ProjectExportSettings;
```

- [ ] **Step 2: Store project settings**

In `ProjectStore`, add state:

```ts
semanticGroups = $state<SemanticGroup[]>([]);
exportSettings = $state<ProjectExportSettings>({
  codegen_mode: "simple",
  generate_runtime_helpers: true,
  generate_semantic_registry: false,
});
```

In `applyActivePayload`, assign:

```ts
this.semanticGroups = project.semantic_groups ?? [];
this.exportSettings = project.export_settings ?? {
  codegen_mode: "simple",
  generate_runtime_helpers: true,
  generate_semantic_registry: false,
};
```

In `clearActiveProject`, reset both fields to the same defaults.

- [ ] **Step 3: Add store update methods**

Add methods to `ProjectStore`:

```ts
async updateExportSettings(changes: Partial<ProjectExportSettings>) {
  const next: ProjectExportSettings = {
    ...this.exportSettings,
    ...changes,
  };
  if (changes.generate_semantic_registry === undefined) {
    next.generate_semantic_registry = next.codegen_mode === "modular";
  }
  const updated = await api.projectExportSettingsUpdate(next, this.activeProjectId ?? undefined);
  this.exportSettings = updated;
  await this.refreshSessions();
  await this.hydrateActiveProject();
}

async updateSemanticGroups(groups: SemanticGroup[]) {
  const updated = await api.projectSemanticGroupsUpdate(groups, this.activeProjectId ?? undefined);
  this.semanticGroups = updated;
  await this.refreshSessions();
  await this.hydrateActiveProject();
}
```

- [ ] **Step 4: Add API functions and mock behavior**

In `src/lib/api.ts`, import `ProjectExportSettings` and `SemanticGroup`. Add:

```ts
export async function projectExportSettingsUpdate(settings: ProjectExportSettings, projectId?: string): Promise<ProjectExportSettings> {
  return invoke<ProjectExportSettings>("project_export_settings_update", { projectId, settings });
}

export async function projectSemanticGroupsUpdate(groups: SemanticGroup[], projectId?: string): Promise<SemanticGroup[]> {
  return invoke<SemanticGroup[]>("project_semantic_groups_update", { projectId, groups });
}
```

In `mockInvoke`, add cases that update `session.project.export_settings` and `session.project.semantic_groups`, marking history with `markMockChanged(session, previous)`.

- [ ] **Step 5: Run frontend typecheck**

Run: `pnpm check`

Expected: PASS after all type references compile.

- [ ] **Step 6: Commit**

```bash
git add src/lib/types.ts src/lib/stores/project.svelte.ts src/lib/api.ts
git commit -m "feat: add frontend semantic project state"
```

## Task 3: Scrollbar and Virtual Cell Rendering

**Files:**
- Modify: `src-tauri/src/texture/mod.rs`
- Modify: `src/lib/engine/renderer.ts`

- [ ] **Step 1: Add Rust texture tests**

Add tests in `src-tauri/src/texture/mod.rs`:

```rust
#[test]
fn generated_scrollbar_has_expected_size() {
    let png = generated_scrollbar(12, 54).unwrap();
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!(img.width(), 12);
    assert_eq!(img.height(), 54);
}

#[test]
fn background_export_bakes_scrollbar_pixels() {
    let mut project = Project::new("Scroll", 176, 166, ModTarget::Forge);
    project.elements.push(Element {
        id: "scroll".into(),
        element_type: ElementType::Scrollbar,
        x: 130,
        y: 54,
        width: Some(12),
        height: Some(54),
        size: None,
        asset: None,
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
        scroll_min: Some(0),
        scroll_max: Some(3),
        visible_rows: Some(3),
        total_rows: Some(6),
        columns: Some(5),
        target_group: Some("machine_buffer".into()),
        binding: None,
        dock: None,
        open_width: None,
        open_height: None,
    });

    let png = composite_atlas_for_layer(&project, Layer::Background).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_ne!(img.get_pixel(130, 54).0[3], 0);
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml scrollbar --locked`

Expected: FAIL because `generated_scrollbar` and compositing support do not exist.

- [ ] **Step 3: Implement generated scrollbar texture**

Add:

```rust
pub fn generated_scrollbar(width: u32, height: u32) -> Result<Vec<u8>, String> {
    let width = width.max(5);
    let height = height.max(9);
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
    for y in 0..height {
        for x in 0..width {
            let border = x == 0 || y == 0 || x + 1 == width || y + 1 == height;
            let color = if border {
                image::Rgba([38, 38, 38, 255])
            } else {
                image::Rgba([92, 92, 92, 255])
            };
            image.put_pixel(x, y, color);
        }
    }
    let thumb_h = 15.min(height);
    for y in 1..thumb_h.saturating_sub(1) {
        for x in 2..width.saturating_sub(2) {
            image.put_pixel(x, y, image::Rgba([198, 198, 198, 255]));
        }
    }
    encode_png(image)
}
```

If `encode_png` has a different local name, use the existing PNG encoder helper from the same file.

- [ ] **Step 4: Bake scrollbar and virtual cells**

In `composite_atlas_for_layer`, treat `ElementType::VirtualSlotCell` like `ElementType::Slot`, and treat `ElementType::Scrollbar` by compositing `generated_scrollbar(width, height)`.

- [ ] **Step 5: Update Pixi renderer**

In `src/lib/engine/renderer.ts`, render:

```ts
case "virtual_slot_cell":
  // Use the same visual as slot but keep the semantic element type intact.
  drawSlot(container, element);
  break;
case "scrollbar":
  drawScrollbar(container, element);
  break;
```

Implement `drawScrollbar` using existing Pixi graphics helpers:

```ts
function drawScrollbar(container: Container, element: Element) {
  const width = element.width ?? 12;
  const height = element.height ?? 54;
  const track = new Graphics();
  track.rect(element.x, element.y, width, height);
  track.fill({ color: 0x5c5c5c });
  track.stroke({ color: 0x262626, width: 1 });
  container.addChild(track);

  const thumbHeight = Math.min(15, height);
  const thumb = new Graphics();
  thumb.rect(element.x + 2, element.y + 1, Math.max(1, width - 4), Math.max(1, thumbHeight - 2));
  thumb.fill({ color: 0xc6c6c6 });
  thumb.stroke({ color: 0xffffff, width: 1 });
  container.addChild(thumb);
}
```

- [ ] **Step 6: Verify rendering and texture tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml scrollbar --locked`

Run: `pnpm check`

Expected: both PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/texture/mod.rs src/lib/engine/renderer.ts
git commit -m "feat: render semantic scrollbars"
```

## Task 4: Scrollable Inventory Template

**Files:**
- Modify: `src-tauri/src/templates/mod.rs`

- [ ] **Step 1: Add template tests**

Add tests in `src-tauri/src/templates/mod.rs`:

```rust
#[test]
fn scrollable_inventory_template_is_listed() {
    let templates = list_template_info();
    let template = templates
        .iter()
        .find(|template| template.name == "scrollable_inventory_machine")
        .unwrap();
    assert_eq!(template.default_width, 176);
    assert_eq!(template.default_height, 166);
}

#[test]
fn scrollable_inventory_template_has_semantic_slots_and_scrollbar() {
    let mut project = Project::new("Scrollable", 176, 166, ModTarget::Forge);
    apply_template(&mut project, "scrollable_inventory_machine").unwrap();

    let scrollable_slots = project
        .elements
        .iter()
        .filter(|element| element.slot_role == Some(SlotRole::ScrollableInventory))
        .count();
    assert_eq!(scrollable_slots, 15);
    assert!(project.elements.iter().any(|element| element.element_type == ElementType::Scrollbar));
    assert!(project.semantic_groups.iter().any(|group| group.id == "machine_buffer"));
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml scrollable_inventory_template --locked`

Expected: FAIL because the template does not exist.

- [ ] **Step 3: Add helper constructors**

In `src-tauri/src/templates/mod.rs`, add local helper functions to reduce repeated semantic fields:

```rust
fn base_element(id: &str, element_type: ElementType, x: i32, y: i32) -> Element {
    Element {
        id: id.into(),
        element_type,
        x,
        y,
        width: None,
        height: None,
        size: None,
        asset: None,
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
    }
}
```

Use this helper only for new template elements in this task.

- [ ] **Step 4: Implement `scrollable_inventory_machine`**

Add the template to `list_templates()` after `advanced_machine()` and implement:

```rust
fn scrollable_inventory_machine() -> Template {
    let mut elements = Vec::new();
    let mut bg = base_element("bg", ElementType::Texture, 0, 0);
    bg.width = Some(176);
    bg.height = Some(166);
    bg.asset = Some(GENERATED_GUI_PANEL.into());
    elements.push(bg);

    let mut title = base_element("title", ElementType::Text, 8, 6);
    title.content = Some("Scrollable Machine".into());
    title.font = Some("minecraft:default".into());
    title.color = Some(0x404040);
    title.shadow = Some(true);
    title.layer = Layer::Overlay;
    elements.push(title);

    for (id, x, y, index) in [
        ("input_left", 44, 22, 0),
        ("input_right", 62, 22, 1),
        ("output", 116, 22, 2),
    ] {
        let mut slot = base_element(id, ElementType::Slot, x, y);
        slot.size = Some(18);
        slot.slot_role = Some(SlotRole::Machine);
        slot.slot_index = Some(index);
        slot.inventory_group = Some("machine".into());
        elements.push(slot);
    }

    let mut progress = base_element("progress_arrow", ElementType::Progress, 86, 24);
    progress.width = Some(22);
    progress.height = Some(15);
    progress.direction = Some(crate::project::FillDirection::LeftToRight);
    progress.animation = Some("progress".into());
    progress.layer = Layer::Animatable;
    elements.push(progress);

    let grid_x = 34;
    let grid_y = 58;
    let columns = 5;
    for row in 0..3 {
        for column in 0..columns {
            let visible_index = row * columns + column;
            let mut slot = base_element(
                &format!("buffer_slot_{row}_{column}"),
                ElementType::Slot,
                grid_x + column as i32 * 18,
                grid_y + row as i32 * 18,
            );
            slot.size = Some(18);
            slot.slot_role = Some(SlotRole::ScrollableInventory);
            slot.slot_index = Some(visible_index);
            slot.inventory_group = Some("machine_buffer".into());
            slot.scroll_binding = Some("buffer_scroll".into());
            elements.push(slot);
        }
    }

    let mut scrollbar = base_element("buffer_scroll", ElementType::Scrollbar, 130, 58);
    scrollbar.width = Some(12);
    scrollbar.height = Some(54);
    scrollbar.scroll_min = Some(0);
    scrollbar.scroll_max = Some(3);
    scrollbar.visible_rows = Some(3);
    scrollbar.total_rows = Some(6);
    scrollbar.columns = Some(5);
    scrollbar.target_group = Some("machine_buffer".into());
    elements.push(scrollbar);

    Template {
        name: "scrollable_inventory_machine",
        description: "Machine with a scrollable 5x3 inventory viewport",
        default_width: 176,
        default_height: 166,
        elements,
        semantic_groups: vec![SemanticGroup {
            id: "machine_buffer".into(),
            kind: SemanticGroupKind::VirtualSlotGrid,
            columns: Some(5),
            visible_rows: Some(3),
            total_rows: Some(6),
            slot_count: Some(30),
            data_source: Some("machine_buffer".into()),
            scroll_binding: Some("buffer_scroll".into()),
            dynamic_height: false,
        }],
    }
}
```

This requires adding `semantic_groups: Vec<SemanticGroup>` to `Template` and updating `apply_template` so it copies groups into `project.semantic_groups`.

- [ ] **Step 5: Verify template tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml scrollable_inventory_template --locked`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/templates/mod.rs
git commit -m "feat: add scrollable inventory template"
```

## Task 5: Export Layout and Codegen Modes

**Files:**
- Modify: `src-tauri/src/export/mod.rs`

- [ ] **Step 1: Add export tests**

Add tests in `src-tauri/src/export/mod.rs`:

```rust
#[test]
fn layout_json_contains_semantic_groups_and_export_settings() {
    let mut project = Project::new("Scrollable", 176, 166, ModTarget::Forge);
    crate::templates::apply_template(&mut project, "scrollable_inventory_machine").unwrap();
    project.export_settings.codegen_mode = CodegenMode::Modular;
    project.export_settings.generate_semantic_registry = true;

    let layout = layout_json_value(&project, &textures_json_for_test());
    assert_eq!(layout["semantic_groups"][0]["id"], "machine_buffer");
    assert_eq!(layout["export_settings"]["codegen_mode"], "modular");
}

#[test]
fn modular_export_plans_semantic_registry() {
    let temp = tempfile::tempdir().unwrap();
    let mut project = Project::new("Scrollable", 176, 166, ModTarget::Forge);
    crate::templates::apply_template(&mut project, "scrollable_inventory_machine").unwrap();
    project.export_settings.codegen_mode = CodegenMode::Modular;
    project.export_settings.generate_semantic_registry = true;
    let config = ExportConfig {
        mod_id: "demo".into(),
        package: "com.example.demo".into(),
        class_name: "ScrollableGui".into(),
        output_dir: temp.path().to_string_lossy().to_string(),
    };

    let preview = preview_export(&project, &config, "forge").unwrap();
    assert!(preview.files.iter().any(|path| path.ends_with("GuiSemanticRegistry.java")));
}
```

If `layout_json_value` does not exist yet, this test drives creating it as a helper extracted from `plan_export`.

- [ ] **Step 2: Run tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml layout_json_contains_semantic_groups_and_export_settings modular_export_plans_semantic_registry --locked`

Expected: FAIL because layout helper and registry output do not exist.

- [ ] **Step 3: Extract layout helper**

Move layout construction from `plan_export` into:

```rust
fn layout_json_value(project: &Project, textures_json: serde_json::Value) -> serde_json::Value {
    let elements_json: Vec<serde_json::Value> = project
        .elements
        .iter()
        .map(|e| {
            let mut val = serde_json::to_value(e).unwrap();
            if e.layer == Layer::Animatable {
                val["texture"] = serde_json::json!(format!("textures/gui/{}.png", e.id));
            }
            val
        })
        .collect();

    serde_json::json!({
        "gui_size": project.gui_size,
        "textures": textures_json,
        "elements": elements_json,
        "groups": project.groups,
        "semantic_groups": project.semantic_groups,
        "animations": project.animations,
        "export_settings": project.export_settings,
    })
}
```

Use it from `plan_export`.

- [ ] **Step 4: Add export settings overrides**

Extend `ExportConfig`:

```rust
pub settings_override: Option<ProjectExportSettings>,
```

Add helper:

```rust
fn effective_export_settings(project: &Project, config: &ExportConfig) -> ProjectExportSettings {
    config
        .settings_override
        .clone()
        .unwrap_or_else(|| project.export_settings.clone())
}
```

Use the effective settings for layout and file planning.

- [ ] **Step 5: Plan modular registry**

When `effective_settings.codegen_mode == CodegenMode::Modular` and `effective_settings.generate_semantic_registry`, add:

```rust
let registry_path = export.java_dir().join("GuiSemanticRegistry.java");
plan_file(
    &mut files,
    registry_path,
    generate_semantic_registry_java(&export, project).into_bytes(),
)?;
```

Implement `generate_semantic_registry_java` as a small Java class with string constants and JSON text:

```rust
fn generate_semantic_registry_java(export: &SanitizedExport, project: &Project) -> String {
    let groups = serde_json::to_string_pretty(&project.semantic_groups).unwrap_or_else(|_| "[]".into());
    format!(
        r#"package {};

public final class GuiSemanticRegistry {{
    public static final String CODEGEN_MODE = "modular";
    public static final String GROUPS_JSON = "{}";

    private GuiSemanticRegistry() {{}}
}}
"#,
        export.package,
        groups.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
    )
}
```

- [ ] **Step 6: Add modular warnings**

In `preview_export`, include warnings from:

```rust
fn semantic_warnings(project: &Project, settings: &ProjectExportSettings) -> Vec<String> {
    if settings.codegen_mode != CodegenMode::Modular {
        return Vec::new();
    }
    let mut warnings = Vec::new();
    if project.semantic_groups.is_empty() {
        warnings.push("Modular code generation is enabled, but the project has no semantic groups.".into());
    }
    for element in &project.elements {
        if matches!(element.element_type, ElementType::Panel | ElementType::Tab | ElementType::VirtualSlotCell)
            && element.inventory_group.is_none()
            && element.target_group.is_none()
        {
            warnings.push(format!("Element '{}' is modular but has no semantic group binding.", element.id));
        }
    }
    warnings
}
```

Append these to `existing_file_warnings`.

- [ ] **Step 7: Verify export tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml layout_json_contains_semantic_groups_and_export_settings modular_export_plans_semantic_registry --locked`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/export/mod.rs
git commit -m "feat: add modular export settings"
```

## Task 6: MCP Surface

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`

- [ ] **Step 1: Add MCP tests**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn tools_list_exposes_export_settings_update() {
    let tools = get_tool_definitions();
    assert!(tools.iter().any(|tool| tool["name"] == "project_export_settings_update"));
}

#[test]
fn export_props_accept_codegen_override() {
    let schema = export_props();
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("codegen_mode"));
    assert!(properties.contains_key("generate_runtime_helpers"));
    assert!(properties.contains_key("generate_semantic_registry"));
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_export_settings_update export_props_accept_codegen_override --locked`

Expected: FAIL because the MCP tool/schema fields are not present.

- [ ] **Step 3: Add tool definitions**

Add to `get_tool_definitions()`:

```rust
td(
    "project_export_settings_update",
    "Update project code generation/export settings",
    project_props(&[
        ("codegen_mode", "string", "simple or modular", false),
        ("generate_runtime_helpers", "boolean", "Generate runtime helper hooks", false),
        ("generate_semantic_registry", "boolean", "Generate semantic registry in modular mode", false),
    ]),
),
td(
    "project_semantic_groups_update",
    "Replace project semantic group definitions",
    project_props(&[("semantic_groups", "array", "Semantic group array", true)]),
),
```

Add the same three optional fields to `export_props()`.

- [ ] **Step 4: Add tool handlers**

In `execute_tool`, route:

```rust
"project_export_settings_update" => project_export_settings_update(&mut sessions, project_id, args),
"project_semantic_groups_update" => project_semantic_groups_update(&mut sessions, project_id, args),
```

Implement:

```rust
fn project_export_settings_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let current = sessions.resolve(project_id)?.project.export_settings.clone();
    let mut next = current;
    if let Some(mode) = args.get("codegen_mode").and_then(|value| value.as_str()) {
        next.codegen_mode = match mode {
            "simple" => CodegenMode::Simple,
            "modular" => CodegenMode::Modular,
            other => return Err(format!("Unknown codegen_mode: {other}")),
        };
    }
    if let Some(value) = args.get("generate_runtime_helpers").and_then(|value| value.as_bool()) {
        next.generate_runtime_helpers = value;
    }
    let has_explicit_semantic_registry = args.get("generate_semantic_registry").is_some();
    if let Some(value) = args.get("generate_semantic_registry").and_then(|value| value.as_bool()) {
        next.generate_semantic_registry = value;
    }
    if !has_explicit_semantic_registry {
        next.generate_semantic_registry = next.codegen_mode == CodegenMode::Modular;
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.export_settings = next.clone();
    sessions.mark_changed(project_id)?;
    serde_json::to_value(next).map_err(|error| error.to_string())
}

fn project_semantic_groups_update(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let groups_value = args
        .get("semantic_groups")
        .ok_or("Missing semantic_groups")?
        .clone();
    let groups: Vec<SemanticGroup> = serde_json::from_value(groups_value)
        .map_err(|error| format!("Invalid semantic_groups: {error}"))?;
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.semantic_groups = groups.clone();
    sessions.mark_changed(project_id)?;
    serde_json::to_value(groups).map_err(|error| error.to_string())
}
```

Import `CodegenMode`, `ProjectExportSettings`, and `SemanticGroup`.

- [ ] **Step 5: Parse export overrides**

In `export_request`, build `settings_override` when any export override field is present:

```rust
let mut settings = sessions.resolve(project_id)?.project.export_settings.clone();
if let Some(mode) = optional_string(args, "codegen_mode") {
    settings.codegen_mode = match mode.as_str() {
        "simple" => CodegenMode::Simple,
        "modular" => CodegenMode::Modular,
        other => return Err(format!("Unknown codegen_mode: {other}")),
    };
}
if let Some(value) = args.get("generate_runtime_helpers").and_then(|value| value.as_bool()) {
    settings.generate_runtime_helpers = value;
}
if let Some(value) = args.get("generate_semantic_registry").and_then(|value| value.as_bool()) {
    settings.generate_semantic_registry = value;
}
```

Set `ExportConfig { settings_override: Some(settings), ... }` only if at least one override key was present; otherwise use `None`.

- [ ] **Step 6: Ensure project reads include settings**

`project_get_active` already serializes `active.project`; no special handling is needed after Task 1. Add `export_settings` to `project_summary` JSON so MCP clients can inspect settings without fetching the full project.

- [ ] **Step 7: Verify MCP tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_export_settings_update export_props_accept_codegen_override --locked`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/mcp/mod.rs
git commit -m "feat: expose codegen settings over mcp"
```

## Task 7: UI Controls

**Files:**
- Modify: `src/lib/components/PropertyPanel.svelte`
- Modify: `src/lib/components/ExportDialog.svelte`

- [ ] **Step 1: Add export dialog mode state**

In `ExportDialog.svelte`, import `CodegenMode` and add:

```ts
let codegenMode = $state<CodegenMode>(project.exportSettings.codegen_mode);
let generateRuntimeHelpers = $state(project.exportSettings.generate_runtime_helpers);
let generateSemanticRegistry = $derived(codegenMode === "modular");
```

Pass these values to preview/export API calls as optional override args.

- [ ] **Step 2: Add export mode controls**

In the form, add after class name:

```svelte
<div class="form-row">
  <label for="exp-codegen">Code Generation</label>
  <select id="exp-codegen" bind:value={codegenMode}>
    <option value="simple">Simple</option>
    <option value="modular">Modular</option>
  </select>
</div>

<label class="check-row">
  <input type="checkbox" bind:checked={generateRuntimeHelpers} />
  <span>Generate runtime helpers</span>
</label>
```

Show a short warning block when `codegenMode === "modular" && project.semanticGroups.length === 0`.

- [ ] **Step 3: Add project-level controls to PropertyPanel**

At the top-level project section in `PropertyPanel.svelte`, add:

```svelte
<div class="field">
  <label for="project-codegen-mode">Code Generation</label>
  <select
    id="project-codegen-mode"
    value={project.exportSettings.codegen_mode}
    onchange={(event) => project.updateExportSettings({ codegen_mode: event.currentTarget.value as CodegenMode })}
  >
    <option value="simple">Simple</option>
    <option value="modular">Modular</option>
  </select>
</div>
```

Add a checkbox for `generate_runtime_helpers`.

- [ ] **Step 4: Add selected element semantic controls**

When selected element type is `slot` or `virtual_slot_cell`, render selects/inputs for `slot_role`, `inventory_group`, `slot_index`, and `scroll_binding`. Use `project.updateElement(selected.id, { slot_role: value })` style updates.

When selected element type is `scrollbar`, render number inputs for `columns`, `visible_rows`, `total_rows`, and text input for `target_group`.

- [ ] **Step 5: Verify frontend**

Run: `pnpm check`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/PropertyPanel.svelte src/lib/components/ExportDialog.svelte
git commit -m "feat: add semantic export controls"
```

## Task 8: End-to-End Verification

**Files:**
- No new source files unless failures require fixes.
- Update docs after verification if command names or behavior differ.

- [ ] **Step 1: Run Rust targeted tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project_defaults_missing_semantic_fields element_semantics_round_trip --locked
cargo test --manifest-path src-tauri/Cargo.toml scrollbar --locked
cargo test --manifest-path src-tauri/Cargo.toml scrollable_inventory_template --locked
cargo test --manifest-path src-tauri/Cargo.toml modular_export_plans_semantic_registry --locked
cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_export_settings_update export_props_accept_codegen_override --locked
```

Expected: all PASS.

- [ ] **Step 2: Run full backend/frontend checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm check
pnpm build
```

Expected: all PASS.

- [ ] **Step 3: Manual MCP workflow**

Use the existing MCP HTTP endpoint from the running dev app. Call:

1. `gui_template_list` and verify `scrollable_inventory_machine`.
2. `project_new` with `template: "scrollable_inventory_machine"`.
3. `project_export_settings_update` with `codegen_mode: "modular"`.
4. `project_get_active` and verify `export_settings.codegen_mode == "modular"`.
5. `project_export_preview` and verify `GuiSemanticRegistry.java` appears.
6. `project_export` and inspect the exported layout JSON for `semantic_groups`, `export_settings`, and scrollable slot metadata.

- [ ] **Step 4: Manual UI workflow**

Run: `pnpm tauri dev`

Expected:

- New project dialog can create `scrollable_inventory_machine`.
- The editor renders 15 visible scrollable slots and a scrollbar.
- Selecting a scrollable slot shows semantic slot fields.
- Selecting the scrollbar shows target group/row fields.
- Export dialog can switch simple/modular mode.
- Modular preview includes a semantic registry file.

- [ ] **Step 5: Update docs**

Update `docs/mcp.md` with `project_export_settings_update`, `project_semantic_groups_update`, and export override examples:

```json
{
  "name": "project_export_settings_update",
  "arguments": {
    "codegen_mode": "modular",
    "generate_runtime_helpers": true,
    "generate_semantic_registry": true
  }
}
```

Update `docs/roadmap.md` to mark semantic slots, scrollbar template, and configurable simple/modular codegen as planned/in progress according to project convention.

- [ ] **Step 6: Final verification**

Run:

```bash
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

Expected: both PASS.

- [ ] **Step 7: Commit docs and final changes**

```bash
git add docs/mcp.md docs/roadmap.md
git commit -m "docs: describe semantic mcp export settings"
```

## Self-Review

- Spec coverage: schema, semantic groups, slot roles, scrollbar element, widget element types, template, texture export, layout JSON, simple/modular codegen, MCP settings, UI controls, compatibility defaults, and verification are each mapped to tasks.
- Scope: the plan does not generate full Minecraft container/menu implementations, search/sort logic, or SophisticatedCore-like runtime systems.
- Type consistency: Rust and TypeScript names use `snake_case` serialized fields; UI/store/API uses the same JSON field names sent to the backend and MCP.
- Verification: each backend/frontend area has targeted tests plus full `cargo test`, `pnpm check`, and `pnpm build`.
