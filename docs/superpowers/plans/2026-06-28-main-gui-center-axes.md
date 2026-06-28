# Main GUI Center Axes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add project-level main GUI center axes that are saved in `.mcgui`, visible/editable in the editor, and used by generated runtime code to center asymmetric baked or modular GUIs in-game.

**Architecture:** Add a serialized `main_gui_center` point to the Rust project model with a default of `gui_size / 2`, mirror it in TypeScript project state, and add a narrow command for updating it. Export writes the center into layout JSON and generated screen classes override `leftPos/topPos` so `main_gui_center` lands at the Minecraft screen center while existing visual bounds offsets continue positioning expanded atlases.

**Tech Stack:** Rust/Tauri commands and serde models, Svelte 5 stores/components, PixiJS renderer overlay, existing export code generation for Forge/NeoForge/Fabric, existing Vitest and Cargo tests.

---

## File Map

- `src-tauri/src/project/mod.rs`: define `MainGuiCenter`, add `Project.main_gui_center`, default helpers, serialization tests.
- `src-tauri/src/commands.rs`: add `project_main_gui_center_update` command and tests for history/no-op behavior.
- `src-tauri/src/lib.rs`: register the new Tauri command.
- `src-tauri/src/export/mod.rs`: include `main_gui_center` in layout JSON, generated runtime data classes, and generated screen placement math.
- `src/lib/types.ts`: add `MainGuiCenter` and `ProjectData.main_gui_center`.
- `src/lib/api.ts`: add mock command and exported API function.
- `src/lib/stores/project.svelte.ts`: add `mainGuiCenter` state, hydration defaulting, and update method.
- `src/lib/components/PropertyPanel.svelte`: add numeric controls in the project section and update size mismatch copy.
- `src/lib/engine/renderer.ts`: draw the center axes overlay.
- `docs/superpowers/specs/2026-06-28-main-gui-center-axes-design.md`: source spec, no implementation changes needed.

---

### Task 1: Rust Project Model And Serialization

**Files:**
- Modify: `src-tauri/src/project/mod.rs`

- [ ] **Step 1: Write failing model tests**

Add these tests near existing project default/round-trip tests in `src-tauri/src/project/mod.rs`:

```rust
#[test]
fn project_defaults_main_gui_center_to_half_gui_size() {
    let json = r#"{
        "name": "Legacy",
        "gui_size": { "width": 177, "height": 167 },
        "mod_target": "forge",
        "elements": [],
        "groups": [],
        "animations": [],
        "assets": []
    }"#;

    let project: Project = serde_json::from_str(json).unwrap();

    assert_eq!(project.main_gui_center.x, 88);
    assert_eq!(project.main_gui_center.y, 83);
}

#[test]
fn project_main_gui_center_round_trips() {
    let project = Project {
        main_gui_center: MainGuiCenter { x: 132, y: 84 },
        ..Project::new("Center", 264, 168, ModTarget::Forge)
    };

    let serialized = serde_json::to_value(&project).unwrap();
    assert_eq!(serialized["main_gui_center"]["x"], serde_json::json!(132));
    assert_eq!(serialized["main_gui_center"]["y"], serde_json::json!(84));

    let deserialized: Project = serde_json::from_value(serialized).unwrap();
    assert_eq!(deserialized.main_gui_center, MainGuiCenter { x: 132, y: 84 });
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo test main_gui_center --lib
```

Expected: compile failure because `MainGuiCenter` and `Project.main_gui_center` do not exist.

- [ ] **Step 3: Add the model type and default helper**

In `src-tauri/src/project/mod.rs`, near `Size`/`VisualBounds` model types, add:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MainGuiCenter {
    pub x: i32,
    pub y: i32,
}

impl MainGuiCenter {
    pub fn default_for_size(size: Size) -> Self {
        Self {
            x: (size.width / 2) as i32,
            y: (size.height / 2) as i32,
        }
    }
}
```

- [ ] **Step 4: Add `main_gui_center` to `Project`**

Add the field after `gui_size`:

```rust
#[serde(default = "default_main_gui_center")]
pub main_gui_center: MainGuiCenter,
```

Add this helper near other serde defaults:

```rust
fn default_main_gui_center() -> MainGuiCenter {
    MainGuiCenter::default_for_size(Size {
        width: 176,
        height: 166,
    })
}
```

Then update `Project::new`:

```rust
let gui_size = Size { width, height };
Self {
    name: name.to_string(),
    gui_size,
    main_gui_center: MainGuiCenter::default_for_size(gui_size),
    mod_target: target,
    elements: Vec::new(),
    groups: Vec::new(),
    states: Vec::new(),
    state_overrides: HashMap::new(),
    animations: Vec::new(),
    assets: Vec::new(),
    asset_metadata: HashMap::new(),
    semantic_groups: Vec::new(),
    attached_regions: Vec::new(),
    export_settings: ProjectExportSettings::default(),
    project_path: None,
    is_dirty: true,
    texture_data: HashMap::new(),
    fonts: Vec::new(),
}
```

- [ ] **Step 5: Fix legacy deserialization default after gui size is known**

Serde field defaults cannot see `gui_size`, so add a custom post-deserialize path. Replace `#[derive(... Deserialize ...)]` on `Project` with a manual deserialize wrapper:

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Project {
    pub name: String,
    pub gui_size: Size,
    #[serde(default = "default_main_gui_center")]
    pub main_gui_center: MainGuiCenter,
    ...
}

impl<'de> Deserialize<'de> for Project {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ProjectData {
            name: String,
            gui_size: Size,
            #[serde(default)]
            main_gui_center: Option<MainGuiCenter>,
            mod_target: ModTarget,
            elements: Vec<Element>,
            groups: Vec<Group>,
            #[serde(default)]
            states: Vec<ProjectState>,
            #[serde(default)]
            state_overrides: HashMap<String, ProjectStateOverrides>,
            animations: Vec<crate::animation::Animation>,
            assets: Vec<String>,
            #[serde(default)]
            asset_metadata: HashMap<String, AssetMetadata>,
            #[serde(default)]
            semantic_groups: Vec<SemanticGroup>,
            #[serde(default)]
            attached_regions: Vec<AttachedRegion>,
            #[serde(default)]
            export_settings: ProjectExportSettings,
            #[serde(default)]
            fonts: Vec<FontAsset>,
        }

        let data = ProjectData::deserialize(deserializer)?;
        Ok(Project {
            name: data.name,
            gui_size: data.gui_size,
            main_gui_center: data
                .main_gui_center
                .unwrap_or_else(|| MainGuiCenter::default_for_size(data.gui_size)),
            mod_target: data.mod_target,
            elements: data.elements,
            groups: data.groups,
            states: data.states,
            state_overrides: data.state_overrides,
            animations: data.animations,
            assets: data.assets,
            asset_metadata: data.asset_metadata,
            semantic_groups: data.semantic_groups,
            attached_regions: data.attached_regions,
            export_settings: data.export_settings,
            project_path: None,
            is_dirty: false,
            texture_data: HashMap::new(),
            fonts: data.fonts,
        })
    }
}
```

Do not preserve `project_path`, `is_dirty`, or `texture_data` from serialized data; those fields remain runtime-only and must be initialized exactly as shown.

- [ ] **Step 6: Run model tests**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo test main_gui_center --lib
```

Expected: both tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/project/mod.rs
git commit -m "feat: add main gui center to project model"
```

---

### Task 2: Backend Command For Updating Center Axes

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing command tests**

Add tests in `src-tauri/src/commands.rs` near project resize/export settings tests:

```rust
#[test]
fn project_main_gui_center_update_records_history() {
    let mut sessions = ProjectSessionManager::default();
    let project_id = sessions.create_session(Project::new("Center", 176, 166, ModTarget::Forge));

    let summary = update_main_gui_center_in_session(
        &mut sessions,
        Some(&project_id),
        crate::project::MainGuiCenter { x: 112, y: 64 },
    )
    .unwrap();

    let project = &sessions.resolve(Some(&project_id)).unwrap().project;
    assert_eq!(project.main_gui_center.x, 112);
    assert_eq!(project.main_gui_center.y, 64);
    assert_eq!(summary.revision, 1);
    assert!(summary.can_undo);
}

#[test]
fn project_main_gui_center_update_noop_preserves_redo() {
    let mut sessions = ProjectSessionManager::default();
    let project_id = sessions.create_session(Project::new("Center", 176, 166, ModTarget::Forge));
    sessions.record_history(Some(&project_id)).unwrap();
    sessions.mark_changed(Some(&project_id)).unwrap();
    sessions.undo(Some(&project_id)).unwrap();

    let result = update_main_gui_center_in_session(
        &mut sessions,
        Some(&project_id),
        crate::project::MainGuiCenter { x: 88, y: 83 },
    )
    .unwrap();

    assert_eq!(result.revision, 2);
    assert!(!result.can_undo);
    assert!(result.can_redo);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo test project_main_gui_center_update --lib
```

Expected: compile failure because `update_main_gui_center_in_session` does not exist.

- [ ] **Step 3: Add command and helper**

In imports at the top of `src-tauri/src/commands.rs`, include `MainGuiCenter`:

```rust
Element, ElementAttachedRegionStateOverride, ElementStateOverridePatch, Group, MainGuiCenter,
ModTarget, Project, ProjectExportSettings, ProjectSessionSummary, ProjectState, SemanticGroup,
```

Add command near `project_resize`:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn project_main_gui_center_update(
    state: State<AppState>,
    center: MainGuiCenter,
    project_id: Option<String>,
) -> Result<ProjectSessionSummary, String> {
    let mut sessions = state.sessions.lock().unwrap();
    update_main_gui_center_in_session(&mut sessions, project_id.as_deref(), center)
}
```

Add helper near `resize_project_in_session`:

```rust
fn update_main_gui_center_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    center: MainGuiCenter,
) -> Result<ProjectSessionSummary, String> {
    {
        let project = &sessions.resolve(project_id)?.project;
        if project.main_gui_center == center {
            let id = sessions.resolve_id(project_id)?.to_string();
            return session_summary(sessions, &id);
        }
    }

    sessions.record_history(project_id)?;
    sessions.resolve_mut(project_id)?.project.main_gui_center = center;
    sessions.mark_changed(project_id)
}
```

- [ ] **Step 4: Register command**

In `src-tauri/src/lib.rs`, add to `tauri::generate_handler!` near `project_resize`:

```rust
commands::project_main_gui_center_update,
```

- [ ] **Step 5: Run command tests**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo test project_main_gui_center_update --lib
```

Expected: both tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add main gui center update command"
```

---

### Task 3: Export Layout JSON And Runtime Placement

**Files:**
- Modify: `src-tauri/src/export/mod.rs`

- [ ] **Step 1: Write failing export tests**

Add tests near `layout_json_includes_visual_bounds_offsets_and_attached_regions` and `generated_runtime_draws_background_at_visual_offset_but_keeps_main_size`:

```rust
#[test]
fn layout_json_includes_main_gui_center() {
    let mut project = Project::new("Centered", 176, 166, ModTarget::Forge);
    project.main_gui_center = crate::project::MainGuiCenter { x: 120, y: 80 };

    let layout = layout_json_value(&project, textures_json_for_test());

    assert_eq!(layout["main_gui_center"]["x"], 120);
    assert_eq!(layout["main_gui_center"]["y"], 80);
}

#[test]
fn generated_runtime_uses_main_gui_center_for_screen_position() {
    let output_dir = TempExportDir::new("main-gui-center-runtime");
    let mut project = Project::new("Centered", 100, 80, ModTarget::Forge);
    project.main_gui_center = crate::project::MainGuiCenter { x: 70, y: 30 };

    export_project(
        &project,
        &ExportConfig {
            mod_id: "testmod".into(),
            package: "com.example.test".into(),
            class_name: "CenteredScreen".into(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: Some(ProjectExportSettings {
                codegen_mode: CodegenMode::Modular,
                generate_runtime_helpers: true,
                generate_semantic_registry: false,
            }),
            overwrite: true,
            scope: ExportScope::FullMod,
        },
        "forge",
    )
    .unwrap();

    let screen = read(&output_dir.join("src/main/java/com/example/test/CenteredScreen.java"));
    let layout = read(&output_dir.join("src/main/java/com/example/test/GuiLayout.java"));
    let layout_json = read(&output_dir.join("src/main/resources/assets/testmod/gui/centered_layout.json"));

    assert!(screen.contains("this.leftPos = (this.width / 2) - 70;"));
    assert!(screen.contains("this.topPos = (this.height / 2) - 30;"));
    assert!(layout.contains("MainGuiCenter mainGuiCenter;"));
    assert!(layout_json.contains(r#""main_gui_center""#));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo test main_gui_center --lib
```

Expected: layout JSON and generated runtime assertions fail.

- [ ] **Step 3: Include center in layout JSON**

In `layout_json_value_for_state`, add `main_gui_center`:

```rust
let mut layout = serde_json::json!({
    "gui_size": project.gui_size,
    "main_gui_center": project.main_gui_center,
    "visual_bounds": visual_bounds,
    "textures": textures_json,
    "elements": elements_json,
    "groups": project.groups,
    "states": project.states,
    "state_overrides": project.state_overrides,
    "semantic_groups": project.semantic_groups,
    "attached_regions": project.attached_regions,
    "animations": project.animations,
    "export_settings": project.export_settings,
});
```

- [ ] **Step 4: Update Forge/NeoForge generated screen placement**

In `generate_forge_screen`, after `super.init();`, insert:

```rust
        this.leftPos = (this.width / 2) - {center_x};
        this.topPos = (this.height / 2) - {center_y};
```

Add format args:

```rust
center_x = project.main_gui_center.x,
center_y = project.main_gui_center.y,
```

Do the same for the NeoForge/simple Forge screen generator if the file has separate functions for each target.

- [ ] **Step 5: Update Fabric generated screen placement**

In the Fabric screen generator, after any existing init/screen setup and before rendering, ensure the equivalent generated class sets:

```java
this.leftPos = (this.width / 2) - {center_x};
this.topPos = (this.height / 2) - {center_y};
```

If the Fabric class uses `x`/`y` rather than `leftPos`/`topPos`, generate:

```java
this.x = (this.width / 2) - {center_x};
this.y = (this.height / 2) - {center_y};
```

Use the actual field names already present in `src-tauri/src/export/mod.rs`.

- [ ] **Step 6: Update generated runtime data classes**

In `generate_forge_like_layout_java`, add a nullable field to `LayoutData`:

```java
@SerializedName("main_gui_center")
MainGuiCenter mainGuiCenter;
```

Add class near `VisualBounds`:

```java
private static final class MainGuiCenter {
    int x = WIDTH / 2;
    int y = HEIGHT / 2;
}
```

Repeat the same in Fabric generated `GuiLayout` code. This keeps layout helper compatibility with layout JSON written by newer exports and older JSON missing the field.

- [ ] **Step 7: Run export tests**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo test main_gui_center --lib
```

Expected: tests pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/export/mod.rs
git commit -m "feat: use main gui center during export"
```

---

### Task 4: Export Warnings For Suspicious Center Axes

**Files:**
- Modify: `src-tauri/src/export/mod.rs`

- [ ] **Step 1: Write failing warning tests**

Add tests near visual authoring warning tests:

```rust
#[test]
fn preview_warns_when_main_gui_center_is_outside_visual_bounds() {
    let output_dir = TempExportDir::new("center-outside-visual-bounds");
    let mut project = Project::new("Center Warning", 100, 80, ModTarget::Forge);
    project.main_gui_center = crate::project::MainGuiCenter { x: 200, y: 40 };
    let config = export_config(output_dir.path(), "CenterWarningGui");

    let preview = preview_export(&project, &config, "forge").unwrap();

    assert!(preview
        .warnings
        .iter()
        .any(|warning| warning.contains("main GUI center") && warning.contains("outside visible content")));
}

#[test]
fn preview_warns_when_main_gui_center_is_outside_declared_gui_size() {
    let output_dir = TempExportDir::new("center-outside-project-size");
    let mut project = Project::new("Center Warning", 100, 80, ModTarget::Forge);
    project.main_gui_center = crate::project::MainGuiCenter { x: -12, y: 40 };
    let config = export_config(output_dir.path(), "CenterWarningGui");

    let preview = preview_export(&project, &config, "forge").unwrap();

    assert!(preview
        .warnings
        .iter()
        .any(|warning| warning.contains("main GUI center") && warning.contains("outside project size")));
}
```

These tests use the existing `TempExportDir::new(...)` and `export_config(output_dir.path(), "...")` helpers from the export test module.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo test main_gui_center_is_outside --lib
```

Expected: warning assertions fail.

- [ ] **Step 3: Add warning logic**

In `visual_authoring_validation(project: &Project)`, after size mismatch warning logic, add:

```rust
let center = project.main_gui_center;
let visual_bounds = project.visual_bounds();
let center_inside_visual = center.x >= visual_bounds.x
    && center.y >= visual_bounds.y
    && i64::from(center.x) < i64::from(visual_bounds.x) + i64::from(visual_bounds.width)
    && i64::from(center.y) < i64::from(visual_bounds.y) + i64::from(visual_bounds.height);
if !center_inside_visual {
    warnings.push(format!(
        "The main GUI center ({}, {}) is outside visible content bounds at {},{} size {}x{}; export will still run, but the screen may appear off-center.",
        center.x,
        center.y,
        visual_bounds.x,
        visual_bounds.y,
        visual_bounds.width,
        visual_bounds.height
    ));
}

let center_inside_gui = center.x >= 0
    && center.y >= 0
    && center.x < project.gui_size.width as i32
    && center.y < project.gui_size.height as i32;
if !center_inside_gui {
    warnings.push(format!(
        "The main GUI center ({}, {}) is outside project size {}x{}; this is allowed for advanced layouts but should be intentional.",
        center.x,
        center.y,
        project.gui_size.width,
        project.gui_size.height
    ));
}
```

- [ ] **Step 4: Update existing size mismatch warning copy**

Find the current warning text for visible content bounds mismatch and replace the final guidance with wording that includes center axes:

```rust
"Visible content extends beyond the declared project size. This is allowed for side panels and baked extensions; adjust the main GUI center axes if the main screen should remain anchored, or resize/shrink the project if the bounds are unintentional."
```

Keep existing dimensions in the warning if they are currently included.

- [ ] **Step 5: Run warning tests**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo test main_gui_center --lib
```

Expected: tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/export/mod.rs
git commit -m "feat: warn about suspicious main gui center axes"
```

---

### Task 5: TypeScript API And Store State

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores/project.svelte.ts`

- [ ] **Step 1: Write failing Vitest coverage**

In `src/lib/api.test.ts`, add a mock API test:

```ts
it("updates main GUI center in the mock project", async () => {
  const summary = await api.projectNew("Center", 176, 166, "forge");

  const updated = await api.projectMainGuiCenterUpdate({ x: 120, y: 80 }, summary.project_id);
  const active = await api.projectGetActive();

  expect(updated.revision).toBeGreaterThan(summary.revision);
  expect(active.project.main_gui_center).toEqual({ x: 120, y: 80 });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cd /home/inky/Development/gui-crafter
npx --yes pnpm@10 test -- src/lib/api.test.ts
```

Expected: compile/test failure because `projectMainGuiCenterUpdate` and `main_gui_center` types do not exist.

- [ ] **Step 3: Add frontend types**

In `src/lib/types.ts`, add:

```ts
export interface MainGuiCenter {
  x: number;
  y: number;
}
```

In `ProjectData`, add:

```ts
main_gui_center?: MainGuiCenter | null;
```

- [ ] **Step 4: Add API mock and invoke wrapper**

In `src/lib/api.ts`, import/use the type:

```ts
import type { MainGuiCenter } from "./types";
```

Add mock case near `project_resize`:

```ts
case "project_main_gui_center_update": {
  const session = mockSession(args?.project_id);
  const center = clone(args?.center as MainGuiCenter);
  if (!center || !Number.isFinite(center.x) || !Number.isFinite(center.y)) {
    throw "Invalid main GUI center";
  }
  const next = { x: Math.round(center.x), y: Math.round(center.y) };
  const current = session.project.main_gui_center ?? {
    x: Math.floor(session.project.gui_size.width / 2),
    y: Math.floor(session.project.gui_size.height / 2),
  };
  if (current.x !== next.x || current.y !== next.y) {
    const previous = clone(session.project);
    session.project.main_gui_center = next;
    markMockChanged(session, previous);
  }
  return mockSummary(session);
}
```

Add exported wrapper near `projectResize`:

```ts
export async function projectMainGuiCenterUpdate(center: MainGuiCenter, projectId?: string): Promise<ProjectSessionSummary> {
  const invoke = await getInvoke();
  return invoke("project_main_gui_center_update", { center, project_id: projectId }) as Promise<ProjectSessionSummary>;
}
```

Update mock `project_new` payload to include:

```ts
main_gui_center: {
  x: Math.floor(((args?.width as number) || 176) / 2),
  y: Math.floor(((args?.height as number) || 166) / 2),
},
```

- [ ] **Step 5: Add store state and update method**

In `src/lib/stores/project.svelte.ts`, add state:

```ts
mainGuiCenter = $state<MainGuiCenter>({ x: 88, y: 83 });
```

Ensure `MainGuiCenter` is imported from `../types`.

In `applyProjectData`, set:

```ts
this.mainGuiCenter = project.main_gui_center ?? {
  x: Math.floor(project.gui_size.width / 2),
  y: Math.floor(project.gui_size.height / 2),
};
```

In `clearActiveProject`, reset:

```ts
this.mainGuiCenter = { x: 88, y: 83 };
```

Add method:

```ts
async updateMainGuiCenter(center: MainGuiCenter) {
  const next = {
    x: Math.round(center.x),
    y: Math.round(center.y),
  };
  await api.projectMainGuiCenterUpdate(next, this.activeProjectId ?? undefined);
  this.mainGuiCenter = next;
  await this.refreshSessions();
  await this.hydrateActiveProject();
  this.bumpRenderVersion();
}
```

- [ ] **Step 6: Run frontend tests**

Run:

```bash
cd /home/inky/Development/gui-crafter
npx --yes pnpm@10 test -- src/lib/api.test.ts
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: tests and typecheck pass.

- [ ] **Step 7: Commit**

```bash
git add src/lib/types.ts src/lib/api.ts src/lib/stores/project.svelte.ts src/lib/api.test.ts
git commit -m "feat: expose main gui center in frontend state"
```

---

### Task 6: Editor Controls And Axis Overlay

**Files:**
- Modify: `src/lib/components/PropertyPanel.svelte`
- Modify: `src/lib/engine/renderer.ts`

- [ ] **Step 1: Add numeric project controls**

In `src/lib/components/PropertyPanel.svelte`, add helpers near other project-level functions:

```ts
function updateMainGuiCenterAxis(axis: "x" | "y", value: number) {
  if (!Number.isFinite(value)) return;
  void project.updateMainGuiCenter({
    ...project.mainGuiCenter,
    [axis]: Math.round(value),
  });
}

function resetMainGuiCenter() {
  void project.updateMainGuiCenter({
    x: Math.floor(project.guiSize.width / 2),
    y: Math.floor(project.guiSize.height / 2),
  });
}
```

In the project properties section, add rows after GUI size fields:

```svelte
<div class="prop-row">
  <label for="project-center-x">Center X</label>
  <input
    id="project-center-x"
    type="number"
    value={project.mainGuiCenter.x}
    onchange={(event) => updateMainGuiCenterAxis("x", event.currentTarget.valueAsNumber)}
  />
</div>
<div class="prop-row">
  <label for="project-center-y">Center Y</label>
  <input
    id="project-center-y"
    type="number"
    value={project.mainGuiCenter.y}
    onchange={(event) => updateMainGuiCenterAxis("y", event.currentTarget.valueAsNumber)}
  />
</div>
<button class="secondary-btn" type="button" onclick={resetMainGuiCenter}>
  Reset center to project middle
</button>
```

- [ ] **Step 2: Update size mismatch copy**

Replace the project warning paragraph:

```svelte
<p>
  Exported textures use visible bounds, while generated screen code uses the main GUI center axes.
</p>
```

For the non-resize-only case, replace the move-only guidance with:

```svelte
<p>
  If this is intentional side content, adjust Center X/Y so the main GUI stays anchored in-game.
  If it is accidental, move visible content by {-visibleContentSizeMismatch.bounds.x},{-visibleContentSizeMismatch.bounds.y}, then resize to
  {visibleContentSizeMismatch.bounds.width}x{visibleContentSizeMismatch.bounds.height}.
</p>
```

- [ ] **Step 3: Draw center axes in renderer**

In `src/lib/engine/renderer.ts`, inside `drawGrid()` after the visual bounds outline and before minor grid, add:

```ts
const center = project.mainGuiCenter;
const axisMinX = Math.min(0, bounds.x);
const axisMinY = Math.min(0, bounds.y);
const axisMaxX = Math.max(gw, bounds.x + bounds.width);
const axisMaxY = Math.max(gh, bounds.y + bounds.height);
g.moveTo(center.x, axisMinY);
g.lineTo(center.x, axisMaxY);
g.stroke({ width: 1, color: 0xff3355, alpha: 0.95 });
g.moveTo(axisMinX, center.y);
g.lineTo(axisMaxX, center.y);
g.stroke({ width: 1, color: 0xff3355, alpha: 0.95 });
```

- [ ] **Step 4: Run Svelte checks**

Run:

```bash
cd /home/inky/Development/gui-crafter
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/PropertyPanel.svelte --svelte-version 5
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: no errors or warnings from touched component.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/PropertyPanel.svelte src/lib/engine/renderer.ts
git commit -m "feat: edit and draw main gui center axes"
```

---

### Task 7: Final Validation And Integration

**Files:**
- Validate all modified files.

- [ ] **Step 1: Run full Rust validation**

Run:

```bash
cd /home/inky/Development/gui-crafter/src-tauri
cargo fmt
cargo test
```

Expected: all tests pass.

- [ ] **Step 2: Run full frontend validation**

Run:

```bash
cd /home/inky/Development/gui-crafter
npx svelte-check --tsconfig ./tsconfig.json
npx --yes pnpm@10 test
npx --yes pnpm@10 build
```

Expected: typecheck and tests pass. Vite may still emit the existing large chunk warning; that warning is acceptable if the build exits successfully.

- [ ] **Step 3: Manual smoke test**

Run the app:

```bash
cd /home/inky/Development/gui-crafter
npx --yes pnpm@10 tauri dev
```

Manual checks:

- Open or create a project.
- Add a texture extending to the right of the main GUI.
- Set Center X to a value left of the full visual bounds center.
- Confirm red axes move in the editor.
- Export preview/export and inspect generated layout JSON for `main_gui_center`.

- [ ] **Step 4: Commit validation fixes if needed**

If validation required small fixes, stage the concrete modified implementation files from `git status --short`:

```bash
git status --short
git add src-tauri/src/project/mod.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/export/mod.rs src/lib/types.ts src/lib/api.ts src/lib/stores/project.svelte.ts src/lib/components/PropertyPanel.svelte src/lib/engine/renderer.ts
git commit -m "fix: validate main gui center axes"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review

Spec coverage:

- Project-level metadata is covered in Task 1.
- Numeric editor controls and visible guide lines are covered in Task 6.
- Export layout JSON and runtime placement are covered in Task 3.
- Warnings for suspicious axes and updated off-bounds guidance are covered in Task 4 and Task 6.
- Old-project defaults are covered in Task 1 and Task 5.

Placeholder scan:

- No `TBD`, `TODO`, or unspecified implementation steps are intentionally left in this plan.

Type consistency:

- Rust uses `MainGuiCenter { x: i32, y: i32 }`.
- JSON uses `main_gui_center`.
- TypeScript uses `MainGuiCenter` and store state `mainGuiCenter`.
- Tauri command is `project_main_gui_center_update`.
