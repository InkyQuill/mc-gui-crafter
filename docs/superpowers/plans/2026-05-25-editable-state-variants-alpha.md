# Editable State Variants Alpha Implementation Plan

> **For InkyQuill:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement editable project state variants so one `.mcgui` file can hold base, collapsed, expanded, and similar visual states via minimal per-state layout overrides.

**Architecture:** Keep the base `Project` canonical and persist only `states` plus `state_overrides`. Add Rust helpers that clone a project and apply one state's overrides for render/export/preview without mutating base data. Store the editor's active state and edit scope in frontend/session state, not as canonical project data. MCP and UI tools default to base edits unless `state_id` or `edit_scope: "state"` is explicit.

**Tech Stack:** Rust/Tauri 2 backend, Serde project models, MCP JSON-RPC in `src-tauri/src/mcp/mod.rs`, Rust export/render helpers, Svelte 5 runes, PixiJS renderer, existing Tauri invoke API.

## Current Code Context

- `src-tauri/src/project/mod.rs` owns the persisted project model. `Project` already contains `elements`, `groups`, `animations`, `texture_data`, `fonts`, `semantic_groups`, `attached_regions`, `asset_metadata`, and `export_settings`. `Element` already has alpha override target fields: `visible`, `x`, `y`, `width`, `height`, `attached_region`, and `layer`.
- `AttachedRegion` already has `visible`, `x`, `y`, `width`, `height`, and `state`; add state ownership metadata here.
- `src-tauri/src/commands.rs` owns Tauri commands and `ProjectSessionManager` history/undo integration.
- `src-tauri/src/mcp/mod.rs` already exposes `project_render`, deprecated alias `project_screenshot`, `schema_discover`, `element_update`, `element_update_many`, and attached-region tools.
- `src/lib/types.ts`, `src/lib/api.ts`, and `src/lib/stores/project.svelte.ts` mirror the model and route frontend edits.
- `src/lib/engine/renderer.ts` renders `project.elements` and `project.attachedRegions`; it should render effective state data when a state is active.
- `src/lib/components/Toolbar.svelte`, `InspectorDock.svelte`, `PropertyPanel.svelte`, and `LayerPanel.svelte` are the right UI integration points.

## Deferred In Alpha

- Do not generate runtime open/close behavior in Java.
- Do not implement animated state transitions.
- Do not support arbitrary per-state texture/text/slot/semantic overrides.
- Do not implement group geometry overrides. Preserve `state_owned` on groups and allow future group visibility support only if the current group model already has a concrete `visible` field when implementation starts.

## Task 1: Add Project State Model And Effective Layout Helpers

**Files:**

- Modify: `src-tauri/src/project/mod.rs`

### Steps

- [ ] **Step 1: Add failing model round-trip tests**

Add tests inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/project/mod.rs`:

```rust
#[test]
fn project_round_trips_state_definitions_and_overrides() {
    let mut project = Project::new("State Variants".into(), GuiSize::Generic9x3);
    project.states.push(ProjectState {
        id: "collapsed".into(),
        label: "Collapsed".into(),
        description: Some("Base pouch layout".into()),
        initial: true,
        export_role: Some("collapsed".into()),
    });
    project.states.push(ProjectState {
        id: "expanded".into(),
        label: "Expanded".into(),
        description: Some("Drawer visible".into()),
        initial: false,
        export_role: Some("expanded".into()),
    });

    let mut overrides = ProjectStateOverrides::default();
    overrides.elements.insert(
        "settings_panel".into(),
        ElementStateOverride {
            visible: Some(true),
            x: Some(176),
            y: Some(0),
            width: Some(88),
            height: Some(166),
            attached_region: Some(Some("settings_drawer".into())),
            layer: Some(Layer::Overlay),
        },
    );
    overrides.attached_regions.insert(
        "settings_drawer".into(),
        AttachedRegionStateOverride {
            visible: Some(true),
            x: Some(176),
            y: Some(0),
            width: Some(88),
            height: Some(166),
        },
    );
    project.state_overrides.insert("expanded".into(), overrides);

    let value = serde_json::to_value(&project).unwrap();
    assert_eq!(value["states"][0]["id"], "collapsed");
    assert_eq!(value["state_overrides"]["expanded"]["elements"]["settings_panel"]["layer"], "overlay");

    let loaded: Project = serde_json::from_value(value).unwrap();
    assert_eq!(loaded.states.len(), 2);
    assert_eq!(loaded.state_overrides["expanded"].elements["settings_panel"].x, Some(176));
}

#[test]
fn effective_layout_applies_state_overrides_without_mutating_base() {
    let mut project = Project::new("State Variants".into(), GuiSize::Generic9x3);
    project.elements.push(test_element("settings_panel", ElementType::Texture, 0, 0));
    project.attached_regions.push(test_attached_region("settings_drawer", 176, 0, 88, 166));
    project.states.push(ProjectState {
        id: "expanded".into(),
        label: "Expanded".into(),
        description: None,
        initial: true,
        export_role: Some("expanded".into()),
    });

    let mut overrides = ProjectStateOverrides::default();
    overrides.elements.insert(
        "settings_panel".into(),
        ElementStateOverride {
            visible: Some(true),
            x: Some(176),
            y: Some(8),
            width: None,
            height: None,
            attached_region: Some(Some("settings_drawer".into())),
            layer: Some(Layer::Overlay),
        },
    );
    overrides.attached_regions.insert(
        "settings_drawer".into(),
        AttachedRegionStateOverride {
            visible: Some(true),
            x: None,
            y: Some(8),
            width: None,
            height: None,
        },
    );
    project.state_overrides.insert("expanded".into(), overrides);

    let effective = project.effective_for_state(Some("expanded")).unwrap();
    let effective_element = effective.find_element("settings_panel").unwrap();
    assert_eq!(effective_element.x, 176);
    assert_eq!(effective_element.y, 8);
    assert_eq!(effective_element.attached_region.as_deref(), Some("settings_drawer"));
    assert_eq!(effective.find_attached_region("settings_drawer").unwrap().y, 8);

    let base_element = project.find_element("settings_panel").unwrap();
    assert_eq!(base_element.x, 0);
    assert_eq!(base_element.y, 0);
    assert_eq!(base_element.attached_region, None);
}

#[test]
fn clearing_state_override_field_restores_inherited_base_value() {
    let mut project = Project::new("State Variants".into(), GuiSize::Generic9x3);
    project.elements.push(test_element("panel", ElementType::Texture, 4, 6));
    project.states.push(ProjectState {
        id: "expanded".into(),
        label: "Expanded".into(),
        description: None,
        initial: true,
        export_role: None,
    });

    project.update_element_state_override(
        "expanded",
        "panel",
        ElementStateOverridePatch {
            x: Some(Some(48)),
            y: Some(Some(64)),
            ..ElementStateOverridePatch::default()
        },
    ).unwrap();
    assert_eq!(project.effective_for_state(Some("expanded")).unwrap().find_element("panel").unwrap().x, 48);

    project.clear_state_override_field("expanded", StateOverrideTarget::Element("panel".into()), "x").unwrap();
    let effective = project.effective_for_state(Some("expanded")).unwrap();
    assert_eq!(effective.find_element("panel").unwrap().x, 4);
    assert_eq!(effective.find_element("panel").unwrap().y, 64);
}
```

If helper constructors like `test_element` or `test_attached_region` do not exist, add small local test helpers in the test module using the existing required fields for `Element` and `AttachedRegion`.

- [ ] **Step 2: Add state and override structs**

Add these public model types near the project/group/attached-region model definitions:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectState {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub initial: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_role: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectStateOverrides {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub elements: HashMap<String, ElementStateOverride>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub groups: HashMap<String, GroupStateOverride>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub attached_regions: HashMap<String, AttachedRegionStateOverride>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ElementStateOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_region: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AttachedRegionStateOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GroupStateOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}
```

Import `std::collections::HashMap` if this module does not already import it.

- [ ] **Step 3: Add persisted fields with compatibility defaults**

Add to `Project`:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub states: Vec<ProjectState>,
#[serde(default, skip_serializing_if = "HashMap::is_empty")]
pub state_overrides: HashMap<String, ProjectStateOverrides>,
```

Initialize both fields in `Project::new` and any custom `Default`/template constructors.

Add to `AttachedRegion`:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub state_owned: Vec<String>,
```

Add the same `state_owned` field to `Group` if the group model can accept extra metadata without breaking existing UI logic:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub state_owned: Vec<String>,
```

- [ ] **Step 4: Add lookup, validation, and effective layout helpers**

Add helpers on `Project`:

```rust
pub fn find_state(&self, id: &str) -> Option<&ProjectState> {
    self.states.iter().find(|state| state.id == id)
}

pub fn find_state_mut(&mut self, id: &str) -> Option<&mut ProjectState> {
    self.states.iter_mut().find(|state| state.id == id)
}

pub fn initial_state_id(&self) -> Option<&str> {
    self.states
        .iter()
        .find(|state| state.initial)
        .or_else(|| self.states.first())
        .map(|state| state.id.as_str())
}

pub fn validate_state_id_available(&self, id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("state id cannot be empty".into());
    }
    if self.states.iter().any(|state| state.id == id) {
        return Err(format!("state id '{id}' already exists"));
    }
    Ok(())
}

pub fn effective_for_state(&self, state_id: Option<&str>) -> Result<Project, String> {
    let Some(state_id) = state_id else {
        return Ok(self.clone());
    };
    if self.find_state(state_id).is_none() {
        return Err(format!("unknown state '{state_id}'"));
    }

    let mut effective = self.clone();
    let Some(overrides) = self.state_overrides.get(state_id) else {
        return Ok(effective);
    };

    for (element_id, override_value) in &overrides.elements {
        let Some(element) = effective.find_element_mut(element_id) else {
            continue;
        };
        if let Some(value) = override_value.visible {
            element.visible = value;
        }
        if let Some(value) = override_value.x {
            element.x = value;
        }
        if let Some(value) = override_value.y {
            element.y = value;
        }
        if let Some(value) = override_value.width {
            element.width = value;
        }
        if let Some(value) = override_value.height {
            element.height = value;
        }
        if let Some(value) = &override_value.attached_region {
            element.attached_region = value.clone();
        }
        if let Some(value) = override_value.layer {
            element.layer = value;
        }
    }

    for (region_id, override_value) in &overrides.attached_regions {
        let Some(region) = effective.find_attached_region_mut(region_id) else {
            continue;
        };
        if let Some(value) = override_value.visible {
            region.visible = value;
        }
        if let Some(value) = override_value.x {
            region.x = value;
        }
        if let Some(value) = override_value.y {
            region.y = value;
        }
        if let Some(value) = override_value.width {
            region.width = value;
        }
        if let Some(value) = override_value.height {
            region.height = value;
        }
    }

    Ok(effective)
}
```

`effective_for_state` requirements:

- `None` returns `Ok(self.clone())`.
- `Some(id)` must return an error if `id` is not in `states`.
- Clone `self`, then apply `state_overrides[id]` to cloned `elements` and `attached_regions`.
- For element overrides, apply only `visible`, `x`, `y`, `width`, `height`, `attached_region`, and `layer`.
- For attached-region overrides, apply only `visible`, `x`, `y`, `width`, and `height`.
- Missing override targets should not panic. Keep them for validation warnings in export/MCP, but ignore them while producing the effective clone.
- The returned clone should keep `states` and `state_overrides` intact so export layout JSON can include metadata.

Add mutation helpers:

```rust
#[derive(Debug, Clone, Default)]
pub struct ElementStateOverridePatch {
    pub visible: Option<Option<bool>>,
    pub x: Option<Option<i32>>,
    pub y: Option<Option<i32>>,
    pub width: Option<Option<u32>>,
    pub height: Option<Option<u32>>,
    pub attached_region: Option<Option<Option<String>>>,
    pub layer: Option<Option<Layer>>,
}

#[derive(Debug, Clone, Default)]
pub struct AttachedRegionStateOverridePatch {
    pub visible: Option<Option<bool>>,
    pub x: Option<Option<i32>>,
    pub y: Option<Option<i32>>,
    pub width: Option<Option<u32>>,
    pub height: Option<Option<u32>>,
}

pub enum StateOverrideTarget {
    Element(String),
    AttachedRegion(String),
    Group(String),
}
```

Implement:

```rust
pub fn update_element_state_override(
    &mut self,
    state_id: &str,
    element_id: &str,
    patch: ElementStateOverridePatch,
) -> Result<(), String>;

pub fn update_attached_region_state_override(
    &mut self,
    state_id: &str,
    region_id: &str,
    patch: AttachedRegionStateOverridePatch,
) -> Result<(), String>;

pub fn clear_state_override_field(
    &mut self,
    state_id: &str,
    target: StateOverrideTarget,
    field: &str,
) -> Result<(), String>;
```

Validation rules:

- Unknown `state_id` returns an error.
- Unknown element or attached-region target returns an error for mutation helpers.
- Unknown field names return an error.
- If all fields for a target become `None`, remove that target entry.
- If a state's override object becomes empty, keep or remove it consistently; prefer removing it to keep JSON compact.

- [ ] **Step 5: Verify model tests fail, then pass**

Run before implementation to confirm failures:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project_round_trips_state_definitions_and_overrides --locked
cargo test --manifest-path src-tauri/Cargo.toml effective_layout_applies_state_overrides_without_mutating_base --locked
cargo test --manifest-path src-tauri/Cargo.toml clearing_state_override_field_restores_inherited_base_value --locked
```

Run after implementation:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project::tests::project_round_trips_state_definitions_and_overrides --locked
cargo test --manifest-path src-tauri/Cargo.toml project::tests::effective_layout_applies_state_overrides_without_mutating_base --locked
cargo test --manifest-path src-tauri/Cargo.toml project::tests::clearing_state_override_field_restores_inherited_base_value --locked
```

- [ ] **Step 6: Commit model changes**

```bash
git add src-tauri/src/project/mod.rs
git commit -m "feat: add editable state variant project model"
```

## Task 2: Add Backend Tauri Commands And Session State

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`

### Steps

- [ ] **Step 1: Add command tests around state CRUD and history**

In `src-tauri/src/commands.rs`, add tests near existing project/session command tests:

```rust
#[test]
fn state_add_update_remove_records_history() {
    let manager = ProjectSessionManager::new();
    let project_id = create_test_session(&manager);

    state_add(
        StateAddRequest {
            id: "expanded".into(),
            label: "Expanded".into(),
            description: None,
            initial: true,
            export_role: Some("expanded".into()),
        },
        Some(project_id.clone()),
        tauri::State::from(&manager),
    ).unwrap();

    let session = manager.resolve(&Some(project_id.clone())).unwrap();
    assert_eq!(session.project.states.len(), 1);
    assert!(session.can_undo);

    state_update(
        "expanded".into(),
        StateUpdateRequest {
            label: Some("Expanded drawer".into()),
            description: Some(Some("Drawer visible".into())),
            initial: Some(true),
            export_role: Some(Some("expanded".into())),
        },
        Some(project_id.clone()),
        tauri::State::from(&manager),
    ).unwrap();

    let session = manager.resolve(&Some(project_id.clone())).unwrap();
    assert_eq!(session.project.states[0].label, "Expanded drawer");

    state_remove("expanded".into(), Some(project_id), tauri::State::from(&manager)).unwrap();
    let session = manager.active().unwrap();
    assert!(session.project.states.is_empty());
}

#[test]
fn state_override_update_and_clear_records_history() {
    let manager = ProjectSessionManager::new();
    let project_id = create_test_session_with_element(&manager, "panel");
    state_add(test_state_add("expanded"), Some(project_id.clone()), tauri::State::from(&manager)).unwrap();

    state_override_update(
        StateOverrideUpdateRequest {
            state_id: "expanded".into(),
            target_type: StateOverrideTargetKind::Element,
            target_id: "panel".into(),
            fields: serde_json::json!({ "x": 48, "visible": false }),
        },
        Some(project_id.clone()),
        tauri::State::from(&manager),
    ).unwrap();

    let session = manager.resolve(&Some(project_id.clone())).unwrap();
    let element_override = &session.project.state_overrides["expanded"].elements["panel"];
    assert_eq!(element_override.x, Some(48));
    assert_eq!(element_override.visible, Some(false));

    state_override_clear(
        StateOverrideClearRequest {
            state_id: "expanded".into(),
            target_type: StateOverrideTargetKind::Element,
            target_id: "panel".into(),
            field: Some("x".into()),
        },
        Some(project_id),
        tauri::State::from(&manager),
    ).unwrap();

    let session = manager.active().unwrap();
    assert_eq!(session.project.state_overrides["expanded"].elements["panel"].x, None);
}
```

If current tests do not use `tauri::State::from`, follow the existing local test pattern for command invocation. The important assertions are history recorded, state persisted, and override field clearing works.

- [ ] **Step 2: Add active state/edit scope to session summaries**

Add runtime-only fields to `ProjectSession` or the existing session summary type in `commands.rs`:

```rust
pub active_state_id: Option<String>,
pub edit_scope: EditScope,
```

Use:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditScope {
    Base,
    State,
}
```

Defaults:

- `active_state_id` is `project.initial_state_id().map(str::to_owned)` when opening/switching a project with states.
- `edit_scope` defaults to `EditScope::Base`.
- `state_set_active` should update `active_state_id` and optionally `edit_scope`, but should not record undo history because it changes editor runtime state, not project data.

- [ ] **Step 3: Add command request/response structs**

Add serializable request structs:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct StateAddRequest {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub initial: bool,
    #[serde(default)]
    pub export_role: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateUpdateRequest {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub initial: Option<bool>,
    #[serde(default)]
    pub export_role: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateOverrideTargetKind {
    Element,
    AttachedRegion,
    Group,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateOverrideUpdateRequest {
    pub state_id: String,
    pub target_type: StateOverrideTargetKind,
    pub target_id: String,
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateOverrideClearRequest {
    pub state_id: String,
    pub target_type: StateOverrideTargetKind,
    pub target_id: String,
    #[serde(default)]
    pub field: Option<String>,
}
```

- [ ] **Step 4: Implement Tauri commands**

Add commands:

```rust
#[tauri::command]
pub fn state_list(project_id: Option<String>, sessions: State<ProjectSessionManager>) -> Result<Vec<ProjectState>, String>;

#[tauri::command]
pub fn state_add(request: StateAddRequest, project_id: Option<String>, sessions: State<ProjectSessionManager>) -> Result<ProjectData, String>;

#[tauri::command]
pub fn state_update(id: String, request: StateUpdateRequest, project_id: Option<String>, sessions: State<ProjectSessionManager>) -> Result<ProjectData, String>;

#[tauri::command]
pub fn state_remove(id: String, project_id: Option<String>, sessions: State<ProjectSessionManager>) -> Result<ProjectData, String>;

#[tauri::command]
pub fn state_set_active(state_id: Option<String>, edit_scope: Option<EditScope>, project_id: Option<String>, sessions: State<ProjectSessionManager>) -> Result<ProjectSessionSummary, String>;

#[tauri::command]
pub fn state_override_update(request: StateOverrideUpdateRequest, project_id: Option<String>, sessions: State<ProjectSessionManager>) -> Result<ProjectData, String>;

#[tauri::command]
pub fn state_override_clear(request: StateOverrideClearRequest, project_id: Option<String>, sessions: State<ProjectSessionManager>) -> Result<ProjectData, String>;
```

Implementation details:

- `state_add` validates non-empty `id` and `label`.
- `state_add` rejects duplicate state ids.
- If adding `initial: true`, set all other states' `initial = false`.
- `state_update` validates existing id and non-empty label when label is provided.
- `state_update initial: true` clears other `initial` flags.
- `state_remove` removes state definition, its `state_overrides` entry, and references from `state_owned` arrays.
- `state_remove` clears session `active_state_id` if it points at the removed state.
- `state_override_update` parses `fields` strictly by `target_type`; unknown alpha fields return `Err`.
- `state_override_update` calls `sessions.record_history(project_id.as_deref())` only after validating the state and target exist.
- `state_override_clear` supports clearing one field or the whole target override when `field` is `None`.

- [ ] **Step 5: Register commands**

Add new commands to the `tauri::generate_handler!` list in `src-tauri/src/lib.rs`.

- [ ] **Step 6: Update frontend API and types**

In `src/lib/types.ts`, add:

```ts
export interface ProjectState {
  id: string;
  label: string;
  description?: string;
  initial?: boolean;
  export_role?: string;
}

export type EditScope = "base" | "state";
export type StateOverrideTargetKind = "element" | "attached_region" | "group";

export interface ElementStateOverride {
  visible?: boolean;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  attached_region?: string | null;
  layer?: Layer;
}

export interface AttachedRegionStateOverride {
  visible?: boolean;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
}

export interface ProjectStateOverrides {
  elements?: Record<string, ElementStateOverride>;
  groups?: Record<string, { visible?: boolean }>;
  attached_regions?: Record<string, AttachedRegionStateOverride>;
}
```

Add to `ProjectData`:

```ts
states?: ProjectState[];
state_overrides?: Record<string, ProjectStateOverrides>;
```

Add `state_owned?: string[]` to `AttachedRegion` and `Group` types.

In `src/lib/api.ts`, add wrappers:

```ts
export async function stateList(projectId?: string): Promise<ProjectState[]> {
  return invoke("state_list", { project_id: projectId }) as Promise<ProjectState[]>;
}

export async function stateAdd(request: StateAddRequest, projectId?: string): Promise<ProjectData> {
  return invoke("state_add", { request, project_id: projectId }) as Promise<ProjectData>;
}

export async function stateUpdate(id: string, request: StateUpdateRequest, projectId?: string): Promise<ProjectData> {
  return invoke("state_update", { id, request, project_id: projectId }) as Promise<ProjectData>;
}

export async function stateRemove(id: string, projectId?: string): Promise<ProjectData> {
  return invoke("state_remove", { id, project_id: projectId }) as Promise<ProjectData>;
}

export async function stateSetActive(stateId: string | null, editScope?: EditScope, projectId?: string): Promise<ProjectSessionSummary> {
  return invoke("state_set_active", { state_id: stateId, edit_scope: editScope, project_id: projectId }) as Promise<ProjectSessionSummary>;
}

export async function stateOverrideUpdate(request: StateOverrideUpdateRequest, projectId?: string): Promise<ProjectData> {
  return invoke("state_override_update", { request, project_id: projectId }) as Promise<ProjectData>;
}

export async function stateOverrideClear(request: StateOverrideClearRequest, projectId?: string): Promise<ProjectData> {
  return invoke("state_override_clear", { request, project_id: projectId }) as Promise<ProjectData>;
}
```

- [ ] **Step 7: Verify backend command tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml state_add_update_remove_records_history --locked
cargo test --manifest-path src-tauri/Cargo.toml state_override_update_and_clear_records_history --locked
cargo test --manifest-path src-tauri/Cargo.toml state_ --locked
```

- [ ] **Step 8: Commit backend command/API changes**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src/lib/types.ts src/lib/api.ts
git commit -m "feat: add state variant backend commands"
```

## Task 3: Add MCP State Tools And State-Aware Rendering

**Files:**

- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`
- Modify: `docs/mcp.md`

### Steps

- [ ] **Step 1: Add failing MCP tests**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn tools_list_exposes_state_variant_tools() {
    let tools = tools_list_value();
    let names = tool_names(&tools);
    for name in [
        "state_list",
        "state_add",
        "state_update",
        "state_remove",
        "state_set_active",
        "state_override_update",
        "state_override_clear",
    ] {
        assert!(names.contains(&name), "{name} should be listed");
    }
}

#[test]
fn state_override_update_changes_effective_render_not_base() {
    let mut sessions = test_state();
    let project_id = create_project_with_texture_element(&mut sessions, "panel", 0, 0);

    call_tool(&mut sessions, "state_add", json!({
        "project_id": project_id,
        "id": "expanded",
        "label": "Expanded",
        "initial": true,
        "export_role": "expanded"
    })).unwrap();

    call_tool(&mut sessions, "state_override_update", json!({
        "project_id": project_id,
        "state_id": "expanded",
        "target_type": "element",
        "target_id": "panel",
        "fields": { "x": 64, "visible": true }
    })).unwrap();

    let session = sessions.resolve(Some(&project_id)).unwrap();
    assert_eq!(session.project.find_element("panel").unwrap().x, 0);
    assert_eq!(
        session.project.effective_for_state(Some("expanded")).unwrap().find_element("panel").unwrap().x,
        64
    );
}

#[test]
fn project_render_accepts_state_id() {
    let mut sessions = test_state();
    let project_id = create_project_with_texture_element(&mut sessions, "panel", 0, 0);
    call_tool(&mut sessions, "state_add", json!({
        "project_id": project_id,
        "id": "expanded",
        "label": "Expanded"
    })).unwrap();
    call_tool(&mut sessions, "state_override_update", json!({
        "project_id": project_id,
        "state_id": "expanded",
        "target_type": "element",
        "target_id": "panel",
        "fields": { "x": 32 }
    })).unwrap();

    let response = call_tool(&mut sessions, "project_render", json!({
        "project_id": project_id,
        "state_id": "expanded"
    })).unwrap();
    let value = tool_text_value(&response);
    assert_eq!(value["state_id"], "expanded");
    assert!(value["path"].as_str().unwrap().ends_with(".png"));
}

#[test]
fn schema_discover_lists_state_override_fields() {
    let value = tool_text_value(&response_for(schema_discover_call(), &test_state()));
    assert!(value["state_variants"]["element_override_fields"].as_array().unwrap().contains(&json!("visible")));
    assert!(value["state_variants"]["attached_region_override_fields"].as_array().unwrap().contains(&json!("width")));
    assert!(value["tools_accepting_state_id"].as_array().unwrap().contains(&json!("project_render")));
}
```

Use existing MCP test helpers if their names differ; preserve the assertions.

- [ ] **Step 2: Add tool definitions and non-mutating classification**

In `get_tool_definitions()`, add definitions for:

- `state_list`: read-only, optional `project_id`.
- `state_add`: mutating, requires `id`, `label`, optional `description`, `initial`, `export_role`.
- `state_update`: mutating, requires `id`, optional fields.
- `state_remove`: mutating, requires `id`.
- `state_set_active`: session-only, requires optional `state_id`, optional `edit_scope`.
- `state_override_update`: mutating, requires `state_id`, `target_type`, `target_id`, `fields`.
- `state_override_clear`: mutating, requires `state_id`, `target_type`, `target_id`, optional `field`.

Add mutating tools to `is_mutating_tool()`:

```rust
"state_add"
| "state_update"
| "state_remove"
| "state_override_update"
| "state_override_clear"
```

Keep `state_list`, `state_set_active`, `project_render`, `project_screenshot`, and `schema_discover` read-only/non-mutating.

- [ ] **Step 3: Implement MCP handlers**

Route in the tool dispatcher:

```rust
"state_list" => state_list(&sessions, project_id),
"state_add" => state_add(&mut sessions, project_id, args),
"state_update" => state_update(&mut sessions, project_id, args),
"state_remove" => state_remove(&mut sessions, project_id, args),
"state_set_active" => state_set_active(&mut sessions, project_id, args),
"state_override_update" => state_override_update(&mut sessions, project_id, args),
"state_override_clear" => state_override_clear(&mut sessions, project_id, args),
```

Return compact JSON with at least:

- `project_id`
- `revision` for mutating project tools
- `states` or changed `state`
- `active_state_id` for `state_set_active`
- `state_overrides` summary for override updates

For `state_override_update`, reject fields outside the alpha allowlists before recording history.

- [ ] **Step 4: Make existing element tools state-aware only when explicit**

Extend MCP `ElementPatch`/bulk patch argument parsing to include:

```rust
#[serde(default)]
state_id: Option<String>,
#[serde(default)]
edit_scope: Option<EditScope>,
```

Behavior:

- If neither is provided, keep existing base edit behavior exactly.
- If `state_id` is provided or `edit_scope == Some(EditScope::State)`, only alpha override fields may be applied to state overrides.
- If state scope includes non-alpha fields such as `content`, `asset`, `slot_role`, `semantic_group`, `texture_data`, or `font`, return an error that names the unsupported fields.
- `element_update_many` stays atomic: validate all targets/fields first, record one history entry, then apply all state overrides.

- [ ] **Step 5: Add `state_id` to render/screenshot**

Extend `project_render` argument parsing:

```rust
#[serde(default)]
state_id: Option<String>,
```

Before rendering, resolve:

```rust
let render_project = session.project.effective_for_state(args.state_id.as_deref())?;
```

Use `render_project` for PNG generation. Do not mutate `session.project`.

Return:

```json
{
  "project_id": "active-project-id",
  "state_id": "expanded",
  "path": "/tmp/mcgui-crafter/expanded.png",
  "width": 264,
  "height": 166
}
```

`project_screenshot` remains an alias and accepts the same `state_id`.

- [ ] **Step 6: Update `schema_discover`**

Add:

```json
"state_variants": {
  "state_fields": ["id", "label", "description", "initial", "export_role"],
  "element_override_fields": ["visible", "x", "y", "width", "height", "attached_region", "layer"],
  "attached_region_override_fields": ["visible", "x", "y", "width", "height"],
  "edit_scopes": ["base", "state"]
},
"tools_accepting_state_id": [
  "element_update",
  "element_update_many",
  "project_render",
  "project_screenshot",
  "project_export_preview",
  "project_export"
]
```

- [ ] **Step 7: Update MCP documentation**

In `docs/mcp.md` and `.agents/skills/mc-gui-crafter/SKILL.md`, document:

- State tools and minimal examples.
- `project_render` preferred over `project_screenshot`, both accepting `state_id`.
- Existing element tools default to base edits.
- Use `schema_discover` before state editing.
- Runtime state toggling/codegen remains deferred.

- [ ] **Step 8: Verify MCP tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_state_variant_tools --locked
cargo test --manifest-path src-tauri/Cargo.toml state_override_update_changes_effective_render_not_base --locked
cargo test --manifest-path src-tauri/Cargo.toml project_render_accepts_state_id --locked
cargo test --manifest-path src-tauri/Cargo.toml schema_discover_lists_state_override_fields --locked
cargo test --manifest-path src-tauri/Cargo.toml state_ schema_discover project_render --locked
```

- [ ] **Step 9: Commit MCP changes**

```bash
git add src-tauri/src/mcp/mod.rs docs/mcp.md .agents/skills/mc-gui-crafter/SKILL.md
git commit -m "feat: expose state variants through MCP"
```

## Task 4: Add Export Preview Validation And State Metadata

**Files:**

- Modify: `src-tauri/src/export/mod.rs`
- Modify: `src-tauri/src/texture/mod.rs` only if render/export helpers are currently located there.

### Steps

- [ ] **Step 1: Add export validation tests**

Add tests to `src-tauri/src/export/mod.rs`:

```rust
#[test]
fn export_preview_warns_for_missing_state_override_targets() {
    let mut project = Project::new("State Warnings".into(), GuiSize::Generic9x3);
    project.states.push(ProjectState {
        id: "expanded".into(),
        label: "Expanded".into(),
        description: None,
        initial: true,
        export_role: None,
    });
    let mut overrides = ProjectStateOverrides::default();
    overrides.elements.insert("missing_panel".into(), ElementStateOverride { x: Some(10), ..Default::default() });
    project.state_overrides.insert("expanded".into(), overrides);

    let preview = preview_export(&project, export_request(temp_output_path())).unwrap();
    assert!(preview.warnings.iter().any(|warning| warning.contains("missing_panel")));
}

#[test]
fn export_layout_includes_state_definitions_and_overrides() {
    let mut project = Project::new("State Layout".into(), GuiSize::Generic9x3);
    project.states.push(ProjectState {
        id: "collapsed".into(),
        label: "Collapsed".into(),
        description: None,
        initial: true,
        export_role: Some("collapsed".into()),
    });
    project.state_overrides.insert("collapsed".into(), ProjectStateOverrides::default());

    let layout = build_layout_json(&project, &project.export_settings).unwrap();
    assert_eq!(layout["states"][0]["id"], "collapsed");
    assert!(layout["state_overrides"].is_object());
}

#[test]
fn export_preview_can_target_specific_state() {
    let mut project = Project::new("State Export".into(), GuiSize::Generic9x3);
    project.elements.push(test_element("panel", ElementType::Texture, 0, 0));
    project.states.push(ProjectState {
        id: "expanded".into(),
        label: "Expanded".into(),
        description: None,
        initial: true,
        export_role: Some("expanded".into()),
    });
    let mut overrides = ProjectStateOverrides::default();
    overrides.elements.insert("panel".into(), ElementStateOverride { x: Some(96), ..Default::default() });
    project.state_overrides.insert("expanded".into(), overrides);

    let mut request = export_request(temp_output_path());
    request.state_id = Some("expanded".into());
    let preview = preview_export(&project, request).unwrap();
    assert_eq!(preview.state_id.as_deref(), Some("expanded"));
}
```

Adapt helper names to existing export test helpers.

- [ ] **Step 2: Extend export request structs**

Add optional `state_id` to export preview/export request structs used by Tauri and MCP:

```rust
#[serde(default)]
pub state_id: Option<String>,
```

For preview/export:

- If `state_id` is absent, use base project.
- If `state_id` is present, call `project.effective_for_state(Some(&state_id))?`.
- Keep layout JSON metadata from the original project, not only effective geometry.

- [ ] **Step 3: Add state validation warnings**

Add or extend validation helper to warn for:

- State duplicate ids.
- Empty or whitespace-only state labels.
- Multiple states with `initial == true`.
- Override references to missing elements, groups, or attached regions.
- `state_owned` attached region whose owner states never make the region visible in effective layout.

For invalid override fields: this should be rejected at typed mutation boundaries; export validation should still defensively warn if a malformed file was loaded through loose JSON.

- [ ] **Step 4: Include state metadata in layout JSON**

In the layout JSON builder, include:

```json
"states": [
  { "id": "expanded", "label": "Expanded", "initial": true, "export_role": "expanded" }
],
"state_overrides": {
  "expanded": {
    "elements": {
      "settings_panel": { "x": 176, "visible": true }
    }
  }
},
"effective_state": "expanded"
```

Only include `effective_state` when rendering/exporting with a state target. Generated Java may keep this as reference metadata/comments only.

- [ ] **Step 5: Wire MCP export tools to `state_id`**

If `project_export_preview` and `project_export` are in `src-tauri/src/mcp/mod.rs`, extend their argument parsing and pass `state_id` through the export request.

- [ ] **Step 6: Verify export tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml export_preview_warns_for_missing_state_override_targets --locked
cargo test --manifest-path src-tauri/Cargo.toml export_layout_includes_state_definitions_and_overrides --locked
cargo test --manifest-path src-tauri/Cargo.toml export_preview_can_target_specific_state --locked
cargo test --manifest-path src-tauri/Cargo.toml export --locked
```

- [ ] **Step 7: Commit export changes**

```bash
git add src-tauri/src/export/mod.rs src-tauri/src/texture/mod.rs src-tauri/src/mcp/mod.rs
git commit -m "feat: include state variants in export preview"
```

## Task 5: Add Frontend Store Effective State Editing

**Files:**

- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src/lib/engine/renderer.ts`
- Modify: `src/lib/components/Canvas.svelte`
- Modify: `src/lib/components/LayerPanel.svelte`
- Modify: `src/lib/components/PropertyPanel.svelte`

### Steps

- [ ] **Step 1: Add frontend state data to project store**

In `ProjectStore`, add state fields:

```ts
states = $state<ProjectState[]>([]);
stateOverrides = $state<Record<string, ProjectStateOverrides>>({});
activeStateId = $state<string | null>(null);
editScope = $state<EditScope>("base");
```

When loading/applying `ProjectData`, assign:

```ts
this.states = data.states ?? [];
this.stateOverrides = data.state_overrides ?? {};
this.activeStateId = session.active_state_id ?? this.initialStateId();
this.editScope = session.edit_scope ?? "base";
```

Add derived helpers:

```ts
get activeState(): ProjectState | null {
  return this.states.find((state) => state.id === this.activeStateId) ?? null;
}
get isStateEditing(): boolean { return this.editScope === "state" && this.activeStateId !== null; }
get effectiveElements(): Element[] {
  const elements = this.elements.map((element) => ({ ...element }));
  const stateId = this.activeStateId;
  if (!stateId) return elements;
  const overrides = this.stateOverrides[stateId]?.elements ?? {};
  return elements.map((element) => ({ ...element, ...(overrides[element.id] ?? {}) }));
}

get effectiveAttachedRegions(): AttachedRegion[] {
  const regions = this.attachedRegions.map((region) => ({ ...region }));
  const stateId = this.activeStateId;
  if (!stateId) return regions;
  const overrides = this.stateOverrides[stateId]?.attached_regions ?? {};
  return regions.map((region) => ({ ...region, ...(overrides[region.id] ?? {}) }));
}
```

`effectiveElements` and `effectiveAttachedRegions` must clone base arrays and overlay only the active state's allowed fields. They must not write back into `this.elements` or `this.attachedRegions`.

- [ ] **Step 2: Route state-scope edits through overrides**

Update existing store methods that change alpha fields:

- movement and resize methods;
- element `visible` updates;
- element `layer` updates;
- element `attached_region` updates;
- attached-region geometry/visibility updates.

Behavior:

- In base scope, keep current API behavior.
- In state scope, call `api.stateOverrideUpdate({ state_id: this.activeStateId, target_type: "element", target_id, fields }, this.activeProjectId)` or the attached-region equivalent, then apply returned project data.
- Content/texture/slot/semantic/font edits remain base edits even in state scope; the property panel should label these as base fields.

Add methods:

```ts
async setActiveState(stateId: string | null, editScope: EditScope = this.editScope): Promise<void>;
async addState(input: StateAddRequest): Promise<void>;
async updateState(id: string, input: StateUpdateRequest): Promise<void>;
async removeState(id: string): Promise<void>;
async updateStateOverride(targetType: StateOverrideTargetKind, targetId: string, fields: Record<string, unknown>): Promise<void>;
async clearStateOverride(targetType: StateOverrideTargetKind, targetId: string, field?: string): Promise<void>;
isElementFieldOverridden(elementId: string, field: keyof ElementStateOverride): boolean;
isAttachedRegionFieldOverridden(regionId: string, field: keyof AttachedRegionStateOverride): boolean;
```

- [ ] **Step 3: Update renderer to consume effective arrays**

In `src/lib/engine/renderer.ts`, replace direct reads of `project.elements` and `project.attachedRegions` with effective getters where rendering visual content:

```ts
const elements = project.effectiveElements;
const attachedRegions = project.effectiveAttachedRegions;
```

Selection should still use base ids. Effective cloned elements must preserve ids.

- [ ] **Step 4: Update Canvas reactive dependencies**

In `Canvas.svelte`, add dependencies for:

- `project.activeStateId`
- `project.editScope`
- `project.states.length`
- state override fields for active state
- effective attached regions

Keep dependency loops explicit enough for Svelte runes to re-render when override fields change.

- [ ] **Step 5: Update layer and property panels for effective view**

In `LayerPanel.svelte`:

- Render `project.effectiveElements` and `project.effectiveAttachedRegions`.
- Add a small marker on rows when a row has any active-state override.
- Add a marker on attached regions/groups whose `state_owned` includes active state.
- Keep selection ids unchanged.

In `PropertyPanel.svelte`:

- Display effective values for alpha fields when an active state is selected.
- Add field-level inherited/overridden markers for `visible`, `x`, `y`, `width`, `height`, `attached_region`, and `layer`.
- Add clear-override buttons for overridden alpha fields.
- Ensure non-alpha fields continue to write to base fields and are not marked as state-overridable.

- [ ] **Step 6: Verify frontend type/build**

Run Svelte autofixer on changed Svelte files:

```bash
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/Canvas.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/LayerPanel.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/PropertyPanel.svelte --svelte-version 5
```

Then:

```bash
pnpm run check
pnpm run build
```

- [ ] **Step 7: Commit frontend store/render changes**

```bash
git add src/lib/stores/project.svelte.ts src/lib/engine/renderer.ts src/lib/components/Canvas.svelte src/lib/components/LayerPanel.svelte src/lib/components/PropertyPanel.svelte
git commit -m "feat: render and edit effective state layouts"
```

## Task 6: Add State Variant UI

**Files:**

- Create: `src/lib/components/StateVariantsPanel.svelte`
- Modify: `src/lib/components/InspectorDock.svelte`
- Modify: `src/lib/components/Toolbar.svelte`
- Modify: `src/lib/stores/layout.svelte.ts`
- Modify: `src/lib/types.ts`

### Steps

- [ ] **Step 1: Add browser tab for States**

Extend `BrowserTab` in `src/lib/types.ts`:

```ts
export type BrowserTab = "layers" | "assets" | "states";
```

If `layout.svelte.ts` validates persisted browser tabs, add migration/default handling so old configs with `"layers"`/`"assets"` still load.

- [ ] **Step 2: Create `StateVariantsPanel.svelte`**

Build a compact operational panel, not a landing/info page. Required controls:

- State list with label and id.
- Active state selection.
- Add state button.
- Rename/update selected state label, description, initial flag, and export role.
- Remove selected state with confirmation.
- Edit scope segmented control: Base / State Override.
- Clear all overrides for selected state.

Use existing button/input styling patterns from `LayerPanel.svelte` and `PropertyPanel.svelte`.

Important behavior:

- Selecting a state calls `project.setActiveState(id, project.editScope)`.
- Switching to Base calls `project.setActiveState(project.activeStateId, "base")`.
- Switching to State Override requires an active state; if none exists, disable the control.
- Add state ids should be normalized from labels using the existing project id naming convention if one exists; otherwise lowercase, replace non-alphanumeric runs with `_`, trim `_`.

- [ ] **Step 3: Wire panel into `InspectorDock.svelte`**

Add a `States` tab alongside `Layers` and `Assets`:

```svelte
<button class:active={layout.values.browser_tab === "states"} onclick={() => setTab("states")}>States</button>
```

Render:

```svelte
{:else if layout.values.browser_tab === "states"}
  <StateVariantsPanel />
{:else}
  <AssetLibrary />
{/if}
```

- [ ] **Step 4: Add toolbar selector**

In `Toolbar.svelte`, add a compact selector between grid/zoom controls and utility buttons:

- Select options: `Base`, then each `project.states`.
- When a state is selected, call `project.setActiveState(id, project.editScope)`.
- Include a Base/State scope segmented control or icon buttons near the selector if space permits. If toolbar overflow becomes tight, keep scope control only in `StateVariantsPanel` and show a short active scope label in toolbar.

The toolbar should make it obvious whether the user is editing base or a state override.

- [ ] **Step 5: Svelte validation**

Run:

```bash
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/StateVariantsPanel.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/InspectorDock.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/Toolbar.svelte --svelte-version 5
pnpm run check
pnpm run build
```

- [ ] **Step 6: Commit UI changes**

```bash
git add src/lib/components/StateVariantsPanel.svelte src/lib/components/InspectorDock.svelte src/lib/components/Toolbar.svelte src/lib/stores/layout.svelte.ts src/lib/types.ts
git commit -m "feat: add state variant editor UI"
```

## Task 7: Documentation, Roadmap, And Final Verification

**Files:**

- Modify: `docs/mcp.md`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`
- Modify: `docs/roadmap.md`
- Optionally modify: `docs/superpowers/specs/2026-05-24-editable-state-variants-alpha-design.md` only to correct stale tool naming if implementation reveals an inconsistency.

### Steps

- [ ] **Step 1: Update docs and roadmap**

Update docs to state:

- State variants are alpha metadata and layout overrides.
- Base remains canonical.
- Only geometry/visibility/attached-region/layer are state-overridable.
- `project_render` and `project_screenshot` accept `state_id`.
- Export includes state metadata but does not generate runtime toggles.

In `docs/roadmap.md`, mark `Editable State Variants Alpha` complete only after implementation and verification pass.

- [ ] **Step 2: Full backend verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

If full backend tests are too slow or fail for unrelated pre-existing reasons, run and record this focused fallback:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project:: --locked
cargo test --manifest-path src-tauri/Cargo.toml state_ --locked
cargo test --manifest-path src-tauri/Cargo.toml schema_discover --locked
cargo test --manifest-path src-tauri/Cargo.toml project_render --locked
cargo test --manifest-path src-tauri/Cargo.toml export --locked
```

- [ ] **Step 3: Full frontend verification**

Run:

```bash
pnpm run check
pnpm run build
```

- [ ] **Step 4: Manual smoke test**

Start the app/dev server as appropriate for the repo:

```bash
pnpm run dev
```

Manual checks:

- Create a project.
- Add one attached region named `settings_drawer`.
- Add collapsed and expanded states.
- Mark `expanded` initial or active.
- Switch to State Override scope.
- Move/resize a drawer element in expanded state.
- Switch back to Base and confirm base geometry did not change.
- Clear one override and confirm effective value returns to base.
- Render both states through MCP `project_render` with and without `state_id`.
- Save, reopen, and confirm states and overrides persist.

- [ ] **Step 5: Lint diff and inspect changed files**

```bash
git diff --check
git status --short
```

Review:

```bash
git diff --stat
git diff
```

- [ ] **Step 6: Final docs commit**

```bash
git add docs/mcp.md .agents/skills/mc-gui-crafter/SKILL.md docs/roadmap.md docs/superpowers/specs/2026-05-24-editable-state-variants-alpha-design.md
git commit -m "docs: document editable state variants alpha"
```

## Implementation Notes

- Do not add `active_state_id` to persisted `Project`; it is editor/session state. Persist `states` and `state_overrides` only.
- Keep `.mcgui` compatibility through `#[serde(default)]` and optional fields.
- Prefer strict mutation validation over loose JSON. Invalid fields should fail at MCP/Tauri command boundaries.
- `state_owned` is metadata only. It should not hide data from export or delete anything by itself.
- Effective layout helpers must clone and overlay; they must never mutate base project data during render/export.
- Existing MCP agents must not be surprised: `element_update` and `element_update_many` remain base edits unless state scope is explicit.
- `project_render` is canonical. `project_screenshot` remains a deprecated alias and must accept identical state arguments.
- Keep UI compact and work-focused. Add controls where users perform repeated editing, not as explanatory copy.

## Self-Review Checklist

Before marking complete:

- [ ] `Project` round-trips with empty `states` and no `state_overrides`.
- [ ] Existing projects without states open, render, save, and export unchanged.
- [ ] Effective layout applies overrides without mutating base.
- [ ] Clearing override fields restores inherited base values.
- [ ] Invalid override fields are rejected by Tauri/MCP commands.
- [ ] `state_id` render/export paths use effective layout.
- [ ] `schema_discover` lists state tools and state override fields.
- [ ] UI clearly shows Base vs State Override scope.
- [ ] Layer and property markers identify overridden/state-owned items.
- [ ] Svelte autofixer ran on changed `.svelte` files.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked` passed or focused fallback and unrelated failures are documented.
- [ ] `pnpm run check` passed.
- [ ] `pnpm run build` passed.
- [ ] `git diff --check` passed.
