# MCP Alpha Ergonomics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make MCP-driven GUI creation practical for slot-heavy Minecraft screens by adding bulk/grid creation, predictable template defaults, compact asset responses, visible buttons, semantic validation, and non-duplicating screen class names.

**Architecture:** Keep `.mcgui` project data compatible and continue representing grids as real slot elements plus ordinary project groups. Add MCP-only presentation helpers where agent ergonomics differ from compact project serialization. Extend generated texture/render/export paths for buttons without adding full runtime button behavior.

**Tech Stack:** Rust/Tauri 2 backend, Serde project format, inline Rust tests, PNG generation/composition in `src-tauri/src/texture`, Svelte 5 frontend, PixiJS renderer, local JSON-RPC MCP server, documentation in Markdown.

---

## File Structure

- Modify `src-tauri/src/mcp/mod.rs`: MCP tool schemas, `element_add_many`, `slot_grid_add`, compact MCP asset responses, effective layer presentation, and tests.
- Modify `src-tauri/src/commands.rs`: shared asset import/list response helpers only if MCP and Tauri command code need a common compact/full split.
- Modify `src-tauri/src/templates/mod.rs`: custom-size empty template application, generated button asset registration, reusable player inventory/hotbar grid helpers, template semantic groups, and template tests.
- Modify `src-tauri/src/texture/mod.rs`: generated button texture and export compositing for `Button`/`ToggleButton`.
- Modify `src/lib/engine/renderer.ts`: Pixi rendering for `button`/`toggle_button` with centered labels.
- Modify `src/lib/stores/project.svelte.ts` and `src/lib/engine/renderer.ts`: group-aware canvas movement if current grouped elements do not already move together.
- Modify `src-tauri/src/export/mod.rs`: screen class name normalization, semantic preview warnings, generated button baking tests, and Java path tests.
- Modify `src/lib/api.ts`: frontend type/API changes if Tauri command payloads change for assets or new grid commands need mock support.
- Modify `docs/mcp.md`: document new tools, enum values, compact assets, and MCP client tool-cache behavior.
- Modify `.agents/skills/mc-gui-crafter/SKILL.md` and `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`: update the local skill with the new bulk/grid workflow.
- Modify `docs/roadmap.md`: mark alpha MCP ergonomics according to final implementation state.

## Task 1: MCP Tool Schemas and Bulk Slot Creation

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`

- [ ] **Step 1: Add failing MCP schema and tool discovery tests**

Add tests in the existing `#[cfg(test)] mod tests` in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn tools_list_exposes_alpha_ergonomics_tools() {
    let tools = get_tool_definitions();
    let names: Vec<_> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();

    for expected in [
        "project_save_as",
        "project_export_preview",
        "project_export",
        "project_export_settings_update",
        "project_semantic_groups_update",
        "element_add_many",
        "slot_grid_add",
    ] {
        assert!(names.contains(&expected), "missing MCP tool {expected}");
    }
}

#[test]
fn semantic_groups_schema_describes_object_array_and_enums() {
    let tools = get_tool_definitions();
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == "project_semantic_groups_update")
        .unwrap();
    let semantic_groups = &tool["inputSchema"]["properties"]["semantic_groups"];
    assert_eq!(semantic_groups["type"], "array");
    assert_eq!(semantic_groups["items"]["type"], "object");
    let description = semantic_groups["description"].as_str().unwrap();
    assert!(description.contains("fixed_slots"));
    assert!(description.contains("virtual_slot_grid"));
    assert!(description.contains("player_inventory"));
}
```

- [ ] **Step 2: Add failing behavior tests for `element_add_many` and `slot_grid_add`**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn element_add_many_is_atomic_for_duplicate_ids() {
    let state = test_state_with_project();
    let response = handle_json_rpc(
        &state,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "element_add_many",
                "arguments": {
                    "elements": [
                        { "id": "slot_a", "type": "slot", "x": 8, "y": 8, "size": 18 },
                        { "id": "slot_a", "type": "slot", "x": 26, "y": 8, "size": 18 }
                    ]
                }
            }
        }),
        None,
    );

    assert!(response["error"]["message"].as_str().unwrap().contains("Duplicate element id"));
    let sessions = state.sessions.lock().unwrap();
    assert!(sessions.resolve(None).unwrap().project.elements.is_empty());
}

#[test]
fn slot_grid_add_creates_grouped_player_inventory_grid() {
    let state = test_state_with_project();
    let response = handle_json_rpc(
        &state,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "slot_grid_add",
                "arguments": {
                    "id_prefix": "player_inv",
                    "x": 8,
                    "y": 84,
                    "columns": 9,
                    "rows": 3,
                    "slot_role": "player_inventory",
                    "inventory_group": "player_inventory",
                    "slot_index_start": 9,
                    "group_id": "player_inventory_grid",
                    "semantic_group_kind": "player_inventory",
                    "slot_count": 27
                }
            }
        }),
        None,
    );

    assert!(response.get("error").is_none(), "{response:#}");
    let result = &response["result"]["content"][0]["text"];
    let value: serde_json::Value = serde_json::from_str(result.as_str().unwrap()).unwrap();
    assert_eq!(value["created_count"], 27);

    let sessions = state.sessions.lock().unwrap();
    let project = &sessions.resolve(None).unwrap().project;
    assert_eq!(project.elements.len(), 27);
    assert_eq!(project.elements[0].id, "player_inv_0");
    assert_eq!(project.elements[0].x, 8);
    assert_eq!(project.elements[1].x, 26);
    assert_eq!(project.elements[0].slot_index, Some(9));
    assert_eq!(project.groups[0].id, "player_inventory_grid");
    assert_eq!(project.groups[0].elements.len(), 27);
    assert_eq!(project.semantic_groups[0].id, "player_inventory");
}
```

If `test_state_with_project()` has a different local helper name, use the existing state/session helper from the MCP tests.

- [ ] **Step 3: Run the failing MCP tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_alpha_ergonomics_tools semantic_groups_schema_describes_object_array_and_enums element_add_many_is_atomic_for_duplicate_ids slot_grid_add_creates_grouped_player_inventory_grid --locked
```

Expected: FAIL because `element_add_many`, `slot_grid_add`, and the object-array schema do not exist yet.

- [ ] **Step 4: Add enum description constants and object-array schema helpers**

Add near the MCP schema helper functions:

```rust
const SLOT_ROLE_DESCRIPTION: &str = "Slot role. Accepted values: machine, player_inventory, hotbar, scrollable_inventory, virtual_storage, upgrade, upgrade_settings, filter, ghost, offhand.";
const SEMANTIC_GROUP_KIND_DESCRIPTION: &str = "Semantic group kind. Accepted values: fixed_slots, virtual_slot_grid, player_inventory, hotbar, upgrade_slots, upgrade_panel, search_field, control_buttons.";

fn semantic_groups_props() -> serde_json::Value {
    let mut schema = project_props(&[]);
    let properties = schema["properties"].as_object_mut().unwrap();
    properties.insert(
        "semantic_groups".into(),
        serde_json::json!({
            "type": "array",
            "description": format!("Semantic group objects. {SEMANTIC_GROUP_KIND_DESCRIPTION}"),
            "items": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "kind": { "type": "string", "description": SEMANTIC_GROUP_KIND_DESCRIPTION },
                    "columns": { "type": "integer" },
                    "visible_rows": { "type": "integer" },
                    "total_rows": { "type": "integer" },
                    "slot_count": { "type": "integer" },
                    "data_source": { "type": "string" },
                    "scroll_binding": { "type": "string" },
                    "dynamic_height": { "type": "boolean" }
                },
                "required": ["id", "kind"]
            }
        }),
    );
    schema["required"] = serde_json::json!(["semantic_groups"]);
    schema
}
```

Use `semantic_groups_props()` for `project_semantic_groups_update`.

- [ ] **Step 5: Add MCP tool definitions**

Add `td(...)` entries in `get_tool_definitions()`:

```rust
td(
    "element_add_many",
    "Add multiple elements atomically",
    project_props(&[("elements", "array", "Array of element objects", true)]),
),
td(
    "slot_grid_add",
    "Create a grouped grid of slot elements with semantic metadata",
    project_props(&[
        ("id_prefix", "string", "Element id prefix; generated ids use <prefix>_<index>", true),
        ("x", "integer", "Grid origin X", true),
        ("y", "integer", "Grid origin Y", true),
        ("columns", "integer", "Number of columns", true),
        ("rows", "integer", "Number of rows", true),
        ("slot_size", "integer", "Slot size in pixels; defaults to 18", false),
        ("spacing", "integer", "Distance between slot origins; defaults to 18", false),
        ("slot_role", "string", SLOT_ROLE_DESCRIPTION, false),
        ("inventory_group", "string", "Logical inventory group id", false),
        ("slot_index_start", "integer", "First slot index; defaults to 0", false),
        ("group_id", "string", "Optional project group id for the generated slots", false),
        ("semantic_group_kind", "string", SEMANTIC_GROUP_KIND_DESCRIPTION, false),
        ("slot_count", "integer", "Optional semantic group slot_count", false),
        ("scroll_binding", "string", "Optional scrollbar element id", false),
    ]),
),
```

- [ ] **Step 6: Implement `element_add_many` route and helper**

In `execute_tool`, add:

```rust
"element_add_many" => element_add_many(&mut sessions, project_id, args),
```

Implement:

```rust
fn element_add_many(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let values = args
        .get("elements")
        .and_then(|value| value.as_array())
        .ok_or("Missing elements array")?;
    let mut elements = Vec::with_capacity(values.len());
    let mut seen = std::collections::HashSet::new();
    for value in values {
        let element = element_from_args(value)?;
        if !seen.insert(element.id.clone()) {
            return Err(format!("Duplicate element id in request: {}", element.id));
        }
        elements.push(element);
    }
    {
        let project = &sessions.resolve(project_id)?.project;
        for element in &elements {
            if project.elements.iter().any(|existing| existing.id == element.id) {
                return Err(format!("Element already exists: {}", element.id));
            }
        }
    }
    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.elements.extend(elements.clone());
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({
        "created_count": elements.len(),
        "elements": elements,
    }))
}
```

Use the existing `element_from_args` helper if present; otherwise extract the parsing currently used by `element_add` into a shared function.

- [ ] **Step 7: Implement `slot_grid_add` route and helper**

In `execute_tool`, add:

```rust
"slot_grid_add" => slot_grid_add(&mut sessions, project_id, args),
```

Implement:

```rust
fn slot_grid_add(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id_prefix = required_str(args, "id_prefix")?;
    let x = required_i32(args, "x")?;
    let y = required_i32(args, "y")?;
    let columns = required_u32(args, "columns")?;
    let rows = required_u32(args, "rows")?;
    let slot_size = optional_u32(args, "slot_size").unwrap_or(18);
    let spacing = optional_u32(args, "spacing").unwrap_or(18) as i32;
    let slot_index_start = optional_u32(args, "slot_index_start").unwrap_or(0);
    let slot_role = optional_slot_role(args, "slot_role")?;
    let inventory_group = optional_string(args, "inventory_group");
    let scroll_binding = optional_string(args, "scroll_binding");

    let mut elements = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let local_index = row * columns + column;
            let mut element = base_slot_element(
                format!("{id_prefix}_{local_index}"),
                x + column as i32 * spacing,
                y + row as i32 * spacing,
                slot_size,
            );
            element.slot_role = slot_role.clone();
            element.inventory_group = inventory_group.clone();
            element.scroll_binding = scroll_binding.clone();
            element.slot_index = Some(slot_index_start + local_index);
            elements.push(element);
        }
    }

    let group_id = optional_string(args, "group_id");
    let semantic_group_kind = optional_semantic_group_kind(args, "semantic_group_kind")?;
    let slot_count = optional_u32(args, "slot_count");

    {
        let project = &sessions.resolve(project_id)?.project;
        for element in &elements {
            if project.elements.iter().any(|existing| existing.id == element.id) {
                return Err(format!("Element already exists: {}", element.id));
            }
        }
        if let Some(group_id) = &group_id {
            if project.groups.iter().any(|group| group.id == *group_id) {
                return Err(format!("Group already exists: {group_id}"));
            }
        }
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element_ids: Vec<String> = elements.iter().map(|element| element.id.clone()).collect();
    session.project.elements.extend(elements.clone());
    let group = group_id.map(|id| crate::project::Group { id, elements: element_ids });
    if let Some(group) = group.clone() {
        session.project.groups.push(group);
    }
    let mut semantic_group = None;
    if let (Some(kind), Some(id)) = (semantic_group_kind, inventory_group.clone()) {
        let group = crate::project::SemanticGroup {
            id,
            kind,
            columns: Some(columns),
            visible_rows: Some(rows),
            total_rows: Some(rows),
            slot_count: slot_count.or(Some(columns * rows)),
            data_source: inventory_group,
            scroll_binding,
            dynamic_height: false,
        };
        session.project.semantic_groups.retain(|existing| existing.id != group.id);
        session.project.semantic_groups.push(group.clone());
        semantic_group = Some(group);
    }
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({
        "created_count": elements.len(),
        "elements": elements,
        "group": group,
        "semantic_group": semantic_group,
    }))
}
```

Add small helpers if the module does not already have them:

```rust
fn required_u32(args: &serde_json::Value, key: &str) -> Result<u32, String> {
    args.get(key)
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| format!("Missing or invalid {key}"))
}
```

- [ ] **Step 8: Verify MCP bulk tests pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_alpha_ergonomics_tools semantic_groups_schema_describes_object_array_and_enums element_add_many_is_atomic_for_duplicate_ids slot_grid_add_creates_grouped_player_inventory_grid --locked
```

Expected: PASS.

- [ ] **Step 9: Commit MCP bulk tools**

```bash
git add src-tauri/src/mcp/mod.rs
git commit -m "feat: add mcp bulk slot grid tools"
```

## Task 2: Compact MCP Asset Responses and Effective Element Layers

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `src/lib/api.ts` only if shared response types need frontend mock parity

- [ ] **Step 1: Add failing compact asset tests**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn asset_import_accepts_explicit_name_and_returns_compact_metadata() {
    let state = test_state_with_project();
    let temp = tempfile::tempdir().unwrap();
    let png_path = temp.path().join("panel.png");
    std::fs::write(&png_path, crate::texture::generated_gui_panel(32, 24).unwrap()).unwrap();

    let response = handle_json_rpc(
        &state,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "asset_import",
                "arguments": {
                    "file_path": png_path,
                    "name": "textures/generated/custom_panel.png"
                }
            }
        }),
        None,
    );

    assert!(response.get("error").is_none(), "{response:#}");
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(value["name"], "textures/generated/custom_panel.png");
    assert_eq!(value["width"], 32);
    assert_eq!(value["height"], 24);
    assert!(value["bytes"].as_u64().unwrap() > 0);
    assert_eq!(value["sha256"].as_str().unwrap().len(), 64);
    assert!(value.get("data_url").is_none());
}

#[test]
fn asset_list_is_compact_and_element_list_includes_default_layer() {
    let state = test_state_with_project();
    {
        let mut sessions = state.sessions.lock().unwrap();
        let session = sessions.resolve_mut(None).unwrap();
        session.project.assets.push("textures/generated/gui_panel.png".into());
        session.project.texture_data.insert(
            "textures/generated/gui_panel.png".into(),
            crate::texture::generated_gui_panel(16, 16).unwrap(),
        );
        session.project.elements.push(base_slot_element("slot_a".into(), 8, 8, 18));
    }

    let assets = execute_tool("asset_list", &serde_json::json!({}), &state).unwrap();
    assert_eq!(assets["assets"][0]["name"], "textures/generated/gui_panel.png");
    assert!(assets["assets"][0].get("data_url").is_none());
    assert!(assets["assets"][0]["sha256"].as_str().unwrap().len() == 64);

    let elements = execute_tool("element_list", &serde_json::json!({}), &state).unwrap();
    assert_eq!(elements["elements"][0]["layer"], "background");
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml asset_import_accepts_explicit_name_and_returns_compact_metadata asset_list_is_compact_and_element_list_includes_default_layer --locked
```

Expected: FAIL because MCP asset responses include `data_url` and `element_list` serializes compact defaults.

- [ ] **Step 3: Add compact asset metadata helper**

In `src-tauri/src/mcp/mod.rs`, add:

```rust
fn asset_metadata(name: &str, data: &[u8]) -> serde_json::Value {
    let image = image::load_from_memory(data).ok();
    let digest = sha256::digest(data);
    serde_json::json!({
        "name": name,
        "width": image.as_ref().map(|image| image.width()).unwrap_or(16),
        "height": image.as_ref().map(|image| image.height()).unwrap_or(16),
        "bytes": data.len(),
        "sha256": digest,
    })
}
```

If the crate does not already include `sha256`, use the already-present hashing dependency if one exists in `Cargo.toml`; otherwise add `sha2` and implement:

```rust
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(data))
}
```

- [ ] **Step 4: Implement explicit asset name validation**

Add helper:

```rust
fn import_asset_name(args: &serde_json::Value, file_path: &str) -> Result<String, String> {
    if let Some(name) = args
        .get("name")
        .or_else(|| args.get("asset_name"))
        .and_then(|value| value.as_str())
    {
        if name.starts_with('/') || name.contains("..") || !name.ends_with(".png") {
            return Err("Asset name must be a relative .png path without '..'".into());
        }
        return Ok(name.to_string());
    }
    let stem = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("texture");
    Ok(format!("textures/{stem}.png"))
}
```

Use this in MCP `asset_import`.

- [ ] **Step 5: Make MCP `asset_import` and `asset_list` compact**

Change MCP `asset_import` to return `asset_metadata(&asset_path, &data)`.

Change MCP `asset_list` map body to:

```rust
if let Some(data) = project.texture_data.get(name) {
    asset_metadata(name, data)
} else {
    serde_json::json!({
        "name": name,
        "width": 16,
        "height": 16,
        "bytes": 0,
        "sha256": "",
    })
}
```

Keep `asset_get_data_url` unchanged.

- [ ] **Step 6: Add MCP effective element serialization**

Add:

```rust
fn element_for_mcp(element: &crate::project::Element) -> serde_json::Value {
    let mut value = serde_json::to_value(element).unwrap();
    if value.get("layer").is_none() {
        value["layer"] = serde_json::json!("background");
    }
    value
}
```

Use it in `element_list`:

```rust
let elements: Vec<_> = session.project.elements.iter().map(element_for_mcp).collect();
Ok(serde_json::json!({ "elements": elements }))
```

- [ ] **Step 7: Verify compact asset tests pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml asset_import_accepts_explicit_name_and_returns_compact_metadata asset_list_is_compact_and_element_list_includes_default_layer --locked
```

Expected: PASS.

- [ ] **Step 8: Commit compact MCP asset responses**

```bash
git add src-tauri/src/mcp/mod.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: compact mcp asset responses"
```

## Task 3: Custom-Size Empty Projects and Default Inventory Grids

**Files:**
- Modify: `src-tauri/src/templates/mod.rs`
- Modify: `src-tauri/src/mcp/mod.rs` only for `project_new` custom-size template application

- [ ] **Step 1: Add failing template/project_new tests**

Add tests in `src-tauri/src/templates/mod.rs`:

```rust
#[test]
fn empty_template_preserves_custom_size_and_generated_panel() {
    let mut project = Project::new("Wide", 264, 162, ModTarget::Forge);
    apply_template_preserving_size(&mut project, "empty", Some((264, 162))).unwrap();

    assert_eq!(project.gui_size.width, 264);
    assert_eq!(project.gui_size.height, 162);
    let panel = project.texture_data.get(GENERATED_GUI_PANEL).unwrap();
    let image = image::load_from_memory(panel).unwrap();
    assert_eq!(image.width(), 264);
    assert_eq!(image.height(), 162);
}

#[test]
fn machine_templates_include_player_inventory_and_hotbar_groups() {
    for name in [
        "furnace",
        "advanced_machine",
        "fluid_tank",
        "brewing_stand",
        "anvil",
        "scrollable_inventory_machine",
        "custom_grid",
    ] {
        let template = get_template(name).unwrap();
        let player = template
            .elements
            .iter()
            .filter(|element| element.slot_role == Some(SlotRole::PlayerInventory))
            .count();
        let hotbar = template
            .elements
            .iter()
            .filter(|element| element.slot_role == Some(SlotRole::Hotbar))
            .count();
        assert_eq!(player, 27, "{name} should include 27 player inventory slots");
        assert_eq!(hotbar, 9, "{name} should include 9 hotbar slots");
        assert!(template.semantic_groups.iter().any(|group| group.id == "player_inventory"));
        assert!(template.semantic_groups.iter().any(|group| group.id == "hotbar"));
    }
}
```

Add a MCP test in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn project_new_empty_template_respects_requested_dimensions() {
    let state = test_state();
    let response = handle_json_rpc(
        &state,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "project_new",
                "arguments": {
                    "name": "Wide Empty",
                    "template": "empty",
                    "width": 264,
                    "height": 162
                }
            }
        }),
        None,
    );
    assert!(response.get("error").is_none(), "{response:#}");
    let sessions = state.sessions.lock().unwrap();
    let project = &sessions.active().unwrap().project;
    assert_eq!(project.gui_size.width, 264);
    assert_eq!(project.gui_size.height, 162);
    assert!(project.texture_data.contains_key(crate::templates::GENERATED_GUI_PANEL));
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml empty_template_preserves_custom_size_and_generated_panel machine_templates_include_player_inventory_and_hotbar_groups project_new_empty_template_respects_requested_dimensions --locked
```

Expected: FAIL because templates do not include default inventory/hotbar grids and empty template applies fixed default size.

- [ ] **Step 3: Add reusable grid helpers in templates**

In `src-tauri/src/templates/mod.rs`, add:

```rust
fn slot_grid(
    id_prefix: &str,
    x: i32,
    y: i32,
    columns: u32,
    rows: u32,
    role: SlotRole,
    inventory_group: &str,
    slot_index_start: u32,
) -> Vec<Element> {
    let mut elements = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let local_index = row * columns + column;
            let mut element = base_element(
                &format!("{id_prefix}_{local_index}"),
                ElementType::Slot,
                x + column as i32 * 18,
                y + row as i32 * 18,
            );
            element.size = Some(18);
            element.slot_role = Some(role.clone());
            element.inventory_group = Some(inventory_group.into());
            element.slot_index = Some(slot_index_start + local_index);
            elements.push(element);
        }
    }
    elements
}

fn player_inventory_grid(x: i32, y: i32) -> Vec<Element> {
    slot_grid("player_inv", x, y, 9, 3, SlotRole::PlayerInventory, "player_inventory", 9)
}

fn hotbar_grid(x: i32, y: i32) -> Vec<Element> {
    slot_grid("hotbar", x, y, 9, 1, SlotRole::Hotbar, "hotbar", 0)
}

fn player_inventory_semantic_groups() -> Vec<SemanticGroup> {
    vec![
        SemanticGroup {
            id: "player_inventory".into(),
            kind: SemanticGroupKind::PlayerInventory,
            columns: Some(9),
            visible_rows: Some(3),
            total_rows: Some(3),
            slot_count: Some(27),
            data_source: Some("player_inventory".into()),
            scroll_binding: None,
            dynamic_height: false,
        },
        SemanticGroup {
            id: "hotbar".into(),
            kind: SemanticGroupKind::Hotbar,
            columns: Some(9),
            visible_rows: Some(1),
            total_rows: Some(1),
            slot_count: Some(9),
            data_source: Some("hotbar".into()),
            scroll_binding: None,
            dynamic_height: false,
        },
    ]
}

fn append_player_inventory(elements: &mut Vec<Element>, semantic_groups: &mut Vec<SemanticGroup>) {
    elements.extend(player_inventory_grid(8, 84));
    elements.extend(hotbar_grid(8, 142));
    semantic_groups.extend(player_inventory_semantic_groups());
}
```

- [ ] **Step 4: Apply inventory helpers to machine/container templates**

For each listed machine/container template constructor, change:

```rust
Template {
    name: "furnace",
    description: "...",
    default_width: 176,
    default_height: 166,
    elements: vec![/* existing elements */],
    semantic_groups: vec![],
}
```

to this pattern:

```rust
let mut elements = vec![/* existing elements */];
let mut semantic_groups = vec![/* existing semantic groups, if any */];
append_player_inventory(&mut elements, &mut semantic_groups);
Template {
    name: "furnace",
    description: "Furnace: input, fuel, progress arrow, output, player inventory",
    default_width: 176,
    default_height: 166,
    elements,
    semantic_groups,
}
```

Apply this to `furnace`, `advanced_machine`, `fluid_tank`, `brewing_stand`, `anvil`, `scrollable_inventory_machine`, and `custom_grid_default`.

- [ ] **Step 5: Preserve custom size for empty template**

Replace `apply_template` internals with a size-aware helper:

```rust
pub fn apply_template_preserving_size(
    project: &mut Project,
    template_name: &str,
    size_override: Option<(u32, u32)>,
) -> Result<(), String> {
    let template =
        get_template(template_name).ok_or_else(|| format!("Unknown template: {template_name}"))?;
    let (width, height) = size_override.unwrap_or((template.default_width, template.default_height));
    project.gui_size.width = width;
    project.gui_size.height = height;
    project.elements = template.elements;
    project.semantic_groups = template.semantic_groups;
    project.groups.clear();
    project.animations.clear();
    add_generated_template_assets(project)?;
    project.is_dirty = true;
    Ok(())
}

pub fn apply_template(project: &mut Project, template_name: &str) -> Result<(), String> {
    apply_template_preserving_size(project, template_name, None)
}
```

In MCP `project_new`, when `template == "empty"` and width/height were supplied, call:

```rust
templates::apply_template_preserving_size(&mut project, template, Some((width, height)))?;
```

For non-empty templates, keep template default dimensions.

- [ ] **Step 6: Verify template tests pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml empty_template_preserves_custom_size_and_generated_panel machine_templates_include_player_inventory_and_hotbar_groups project_new_empty_template_respects_requested_dimensions --locked
```

Expected: PASS.

- [ ] **Step 7: Commit template defaults**

```bash
git add src-tauri/src/templates/mod.rs src-tauri/src/mcp/mod.rs
git commit -m "feat: add default inventory grids to templates"
```

## Task 4: Button Texture Generation, Rendering, and Export

**Files:**
- Modify: `src-tauri/src/texture/mod.rs`
- Modify: `src-tauri/src/templates/mod.rs`
- Modify: `src/lib/engine/renderer.ts`

- [ ] **Step 1: Add failing button texture/export tests**

Add tests in `src-tauri/src/texture/mod.rs`:

```rust
#[test]
fn generated_button_has_expected_size_and_pixels() {
    let png = generated_button(64, 20).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.width(), 64);
    assert_eq!(image.height(), 20);
    assert_ne!(image.get_pixel(0, 0).0[3], 0);
    assert_ne!(image.get_pixel(32, 10).0[3], 0);
}

#[test]
fn background_export_bakes_button_pixels() {
    let mut project = Project::new("Button", 176, 166, ModTarget::Forge);
    let mut button = crate::templates::base_element_for_test("button", ElementType::Button, 20, 20);
    button.width = Some(64);
    button.height = Some(20);
    project.elements.push(button);

    let png = composite_atlas_for_layer(&project, Layer::Background).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_ne!(image.get_pixel(20, 20).0[3], 0);
}
```

If `base_element_for_test` does not exist, create the element inline using the same full `Element` struct shape used by nearby tests.

- [ ] **Step 2: Run tests and confirm they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml generated_button_has_expected_size_and_pixels background_export_bakes_button_pixels --locked
```

Expected: FAIL because generated button textures are not implemented or composited.

- [ ] **Step 3: Implement generated button texture**

In `src-tauri/src/texture/mod.rs`, add:

```rust
pub fn generated_button(width: u32, height: u32) -> Result<Vec<u8>, String> {
    let width = width.max(8);
    let height = height.max(8);
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
    for y in 0..height {
        for x in 0..width {
            let top_or_left = x == 0 || y == 0;
            let bottom_or_right = x + 1 == width || y + 1 == height;
            let color = if top_or_left {
                image::Rgba([255, 255, 255, 255])
            } else if bottom_or_right {
                image::Rgba([64, 64, 64, 255])
            } else if x == 1 || y == 1 {
                image::Rgba([198, 198, 198, 255])
            } else if x + 2 == width || y + 2 == height {
                image::Rgba([92, 92, 92, 255])
            } else {
                image::Rgba([128, 128, 128, 255])
            };
            image.put_pixel(x, y, color);
        }
    }
    encode_png(image)
}
```

- [ ] **Step 4: Bake button and toggle button elements**

In the texture compositing path, add `ElementType::Button | ElementType::ToggleButton` branch:

```rust
ElementType::Button | ElementType::ToggleButton => {
    let width = element.width.unwrap_or(64);
    let height = element.height.unwrap_or(20);
    let button = generated_button(width, height)?;
    composite_png(&mut canvas, &button, element.x, element.y, None)?;
}
```

Use the local compositor helper names already present in `texture/mod.rs`.

- [ ] **Step 5: Register generated button asset**

In `src-tauri/src/templates/mod.rs`, add a constant:

```rust
pub const GENERATED_BUTTON: &str = "textures/generated/button.png";
```

In `add_generated_template_assets`, add:

```rust
add_static_generated_asset(project, GENERATED_BUTTON, crate::texture::generated_button(64, 20)?);
```

- [ ] **Step 6: Add Pixi button rendering**

In `src/lib/engine/renderer.ts`, add generated texture path:

```ts
"textures/generated/button.png",
```

Add cases in `drawElement`:

```ts
case "button":
case "toggle_button":
  return this.drawButton(el);
```

Add:

```ts
private drawButton(el: Element): Container {
  const container = new Container();
  const g = new Graphics();
  const w = el.width ?? 64;
  const h = el.height ?? 20;
  g.rect(el.x, el.y, w, h);
  g.fill({ color: 0x808080 });
  g.rect(el.x, el.y, w, 1);
  g.fill({ color: 0xffffff });
  g.rect(el.x, el.y, 1, h);
  g.fill({ color: 0xffffff });
  g.rect(el.x, el.y + h - 1, w, 1);
  g.fill({ color: 0x404040 });
  g.rect(el.x + w - 1, el.y, 1, h);
  g.fill({ color: 0x404040 });
  container.addChild(g);

  if (el.content) {
    const label = new Text({
      text: el.content,
      style: new TextStyle({
        fontSize: 10,
        fill: 0xffffff,
        fontFamily: "monospace",
      }),
    });
    label.anchor.set(0.5);
    label.x = el.x + w / 2;
    label.y = el.y + h / 2;
    container.addChild(label);
  }

  return container;
}
```

- [ ] **Step 7: Verify button tests and frontend checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml generated_button_has_expected_size_and_pixels background_export_bakes_button_pixels --locked
pnpm check
```

Expected: both PASS.

- [ ] **Step 8: Commit button rendering**

```bash
git add src-tauri/src/texture/mod.rs src-tauri/src/templates/mod.rs src/lib/engine/renderer.ts
git commit -m "feat: render generated gui buttons"
```

## Task 5: Export Naming and Semantic Preview Validation

**Files:**
- Modify: `src-tauri/src/export/mod.rs`

- [ ] **Step 1: Add failing export tests**

Add tests in `src-tauri/src/export/mod.rs`:

```rust
#[test]
fn class_name_ending_screen_does_not_duplicate_screen_suffix() {
    let temp = tempfile::tempdir().unwrap();
    let project = Project::new("Named", 176, 166, ModTarget::Forge);
    let config = ExportConfig {
        mod_id: "demo".into(),
        package: "com.example.demo".into(),
        class_name: "AutoCutterGeneratedScreen".into(),
        output_dir: temp.path().to_string_lossy().to_string(),
        settings_override: None,
    };

    let preview = preview_export(&project, &config, "forge").unwrap();
    assert!(preview
        .files
        .iter()
        .any(|path| path.ends_with("AutoCutterGeneratedScreen.java")));
    assert!(!preview
        .files
        .iter()
        .any(|path| path.ends_with("AutoCutterGeneratedScreenScreen.java")));
}

#[test]
fn preview_warns_when_semantic_slot_count_exceeds_matching_elements() {
    let mut project = Project::new("Mismatch", 176, 166, ModTarget::Forge);
    project.semantic_groups.push(SemanticGroup {
        id: "player_inventory".into(),
        kind: SemanticGroupKind::PlayerInventory,
        columns: Some(9),
        visible_rows: Some(3),
        total_rows: Some(3),
        slot_count: Some(27),
        data_source: Some("player_inventory".into()),
        scroll_binding: None,
        dynamic_height: false,
    });
    for index in 0..6 {
        let mut slot = slot_element_for_test(format!("player_inv_{index}"), 8 + index as i32 * 18, 84);
        slot.slot_role = Some(SlotRole::PlayerInventory);
        slot.inventory_group = Some("player_inventory".into());
        slot.slot_index = Some(9 + index);
        project.elements.push(slot);
    }

    let temp = tempfile::tempdir().unwrap();
    let config = ExportConfig {
        mod_id: "demo".into(),
        package: "com.example.demo".into(),
        class_name: "MismatchGui".into(),
        output_dir: temp.path().to_string_lossy().to_string(),
        settings_override: None,
    };
    let preview = preview_export(&project, &config, "forge").unwrap();
    assert!(preview.warnings.iter().any(|warning| {
        warning.contains("player_inventory") && warning.contains("27") && warning.contains("6")
    }));
}

#[test]
fn preview_warns_for_missing_scrollbar_targets_and_bindings() {
    let mut project = Project::new("Bad Scroll", 176, 166, ModTarget::Forge);
    let mut scrollbar = scrollbar_element_for_test("scroll", 130, 58);
    scrollbar.target_group = Some("missing_group".into());
    project.elements.push(scrollbar);
    let mut slot = slot_element_for_test("slot", 8, 8);
    slot.scroll_binding = Some("missing_scrollbar".into());
    project.elements.push(slot);

    let temp = tempfile::tempdir().unwrap();
    let config = ExportConfig {
        mod_id: "demo".into(),
        package: "com.example.demo".into(),
        class_name: "BadScrollGui".into(),
        output_dir: temp.path().to_string_lossy().to_string(),
        settings_override: None,
    };
    let preview = preview_export(&project, &config, "forge").unwrap();
    assert!(preview.warnings.iter().any(|warning| warning.contains("missing_group")));
    assert!(preview.warnings.iter().any(|warning| warning.contains("missing_scrollbar")));
}
```

Add local test helpers `slot_element_for_test` and `scrollbar_element_for_test` using the existing full `Element` struct shape if they do not already exist.

- [ ] **Step 2: Run tests and confirm they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml class_name_ending_screen_does_not_duplicate_screen_suffix preview_warns_when_semantic_slot_count_exceeds_matching_elements preview_warns_for_missing_scrollbar_targets_and_bindings --locked
```

Expected: FAIL because class names append `Screen` unconditionally and semantic warnings are incomplete.

- [ ] **Step 3: Normalize screen class names**

In `SanitizedExport`, add:

```rust
screen_class_name: String,
```

In `SanitizedExport::new`:

```rust
let class_name = sanitize_class_name(&config.class_name);
let screen_class_name = if class_name.ends_with("Screen") {
    class_name.clone()
} else {
    format!("{class_name}Screen")
};
```

Set `screen_class_name` in the struct.

Change screen file path:

```rust
let screen_path = export.java_dir().join(format!("{}.java", export.screen_class_name));
```

In Forge/Fabric/NeoForge screen generators, use:

```rust
class_name = export.screen_class_name,
```

Change comments that currently append `Screen` manually to use `screen_class_name` where they refer to the screen class.

- [ ] **Step 4: Add semantic warning helpers**

In `src-tauri/src/export/mod.rs`, extend existing `semantic_warnings` or add a helper it calls:

```rust
fn semantic_integrity_warnings(project: &Project) -> Vec<String> {
    let mut warnings = Vec::new();
    let scrollbar_ids: std::collections::HashSet<_> = project
        .elements
        .iter()
        .filter(|element| element.visible && element.element_type == ElementType::Scrollbar)
        .map(|element| element.id.as_str())
        .collect();
    let semantic_group_ids: std::collections::HashSet<_> =
        project.semantic_groups.iter().map(|group| group.id.as_str()).collect();

    for group in &project.semantic_groups {
        let matching = count_matching_group_elements(project, group);
        let expected = match group.kind {
            SemanticGroupKind::PlayerInventory => group.slot_count.unwrap_or(27),
            SemanticGroupKind::Hotbar => group.slot_count.unwrap_or(9),
            SemanticGroupKind::FixedSlots => group.slot_count.unwrap_or(matching),
            SemanticGroupKind::VirtualSlotGrid => {
                group.columns.unwrap_or(0).saturating_mul(group.visible_rows.unwrap_or(0))
            }
            _ => group.slot_count.unwrap_or(matching),
        };
        if expected > matching {
            warnings.push(format!(
                "Semantic group '{}' declares {} slots/cells, but only {} matching visible elements were found.",
                group.id, expected, matching
            ));
        }
        if let Some(binding) = &group.scroll_binding {
            if !scrollbar_ids.contains(binding.as_str()) {
                warnings.push(format!(
                    "Semantic group '{}' references missing scrollbar '{}'.",
                    group.id, binding
                ));
            }
        }
    }

    for element in &project.elements {
        if !element.visible {
            continue;
        }
        if element.element_type == ElementType::Scrollbar {
            if let Some(target) = &element.target_group {
                if !semantic_group_ids.contains(target.as_str()) {
                    warnings.push(format!(
                        "Scrollbar '{}' targets missing semantic group '{}'.",
                        element.id, target
                    ));
                }
            }
        }
        if let Some(binding) = &element.scroll_binding {
            if !scrollbar_ids.contains(binding.as_str()) {
                warnings.push(format!(
                    "Element '{}' references missing scrollbar '{}'.",
                    element.id, binding
                ));
            }
        }
    }

    warnings
}
```

Add:

```rust
fn count_matching_group_elements(project: &Project, group: &SemanticGroup) -> u32 {
    project
        .elements
        .iter()
        .filter(|element| element.visible)
        .filter(|element| {
            element.inventory_group.as_deref() == Some(group.id.as_str())
                || matches!(
                    (&group.kind, &element.slot_role),
                    (SemanticGroupKind::PlayerInventory, Some(SlotRole::PlayerInventory))
                        | (SemanticGroupKind::Hotbar, Some(SlotRole::Hotbar))
                )
        })
        .filter(|element| {
            matches!(
                element.element_type,
                ElementType::Slot | ElementType::VirtualSlotCell
            )
        })
        .count() as u32
}
```

Append these warnings in `preview_export`.

- [ ] **Step 5: Verify export tests pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml class_name_ending_screen_does_not_duplicate_screen_suffix preview_warns_when_semantic_slot_count_exceeds_matching_elements preview_warns_for_missing_scrollbar_targets_and_bindings --locked
```

Expected: PASS.

- [ ] **Step 6: Commit export validation and naming**

```bash
git add src-tauri/src/export/mod.rs
git commit -m "feat: validate semantic export metadata"
```

## Task 6: Group-Aware Canvas Movement

**Files:**
- Modify: `src/lib/engine/renderer.ts`
- Modify: `src/lib/stores/project.svelte.ts` if the existing store lacks a grouped move helper

- [ ] **Step 1: Inspect existing group movement behavior**

Run:

```bash
rg -n "groupForElement|moveElement|selectedIds|dragStartPositions|group" src/lib/engine/renderer.ts src/lib/stores/project.svelte.ts
```

Expected: identify whether moving a selected grouped element already moves the group. If it does, skip implementation steps in this task and keep the verification command in Step 5.

- [ ] **Step 2: Add a grouped move store helper if missing**

If no existing helper moves grouped elements together, add to `ProjectStore`:

```ts
async moveElementOrGroup(elementId: string, x: number, y: number) {
  const element = this.elementById(elementId);
  if (!element) return;
  const group = this.groupForElement(elementId);
  if (!group) {
    await this.moveElement(elementId, x, y);
    return;
  }

  const dx = x - element.x;
  const dy = y - element.y;
  for (const id of group.elements) {
    const grouped = this.elementById(id);
    if (!grouped) continue;
    await this.moveElement(id, grouped.x + dx, grouped.y + dy);
  }
}
```

If repeated backend calls make undo too noisy, add a backend group move command later. For this alpha task, correctness matters more than undo compaction.

- [ ] **Step 3: Use grouped move during canvas drag**

In `src/lib/engine/renderer.ts`, where drag completes and calls `project.moveElement(...)`, replace the selected primary element movement with:

```ts
void project.moveElementOrGroup(el.id, nextX, nextY);
```

If current code already moves all `editor.selectedIds`, ensure grouped ids are added to the movement set once to avoid double movement.

- [ ] **Step 4: Verify frontend typecheck**

Run:

```bash
pnpm check
```

Expected: PASS.

- [ ] **Step 5: Commit group movement**

```bash
git add src/lib/engine/renderer.ts src/lib/stores/project.svelte.ts
git commit -m "feat: move grouped grid elements together"
```

## Task 7: Documentation and Skill Updates

**Files:**
- Modify: `docs/mcp.md`
- Modify: `docs/roadmap.md`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`
- Modify: `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`

- [ ] **Step 1: Update MCP docs**

In `docs/mcp.md`, add a "Bulk Slot and Grid Tools" section with:

```markdown
### Bulk Slot and Grid Tools

Use `element_add_many` when creating several arbitrary elements in one history
entry. Use `slot_grid_add` for vanilla inventories, hotbars, storage grids, and
scrollable visible cells.

Common vanilla coordinates for a 176x166 container:

- player inventory: `x=8`, `y=84`, `columns=9`, `rows=3`, `slot_index_start=9`
- hotbar: `x=8`, `y=142`, `columns=9`, `rows=1`, `slot_index_start=0`
```

Add enum lists for `SlotRole` and `SemanticGroupKind`.

Add compact asset behavior:

```markdown
`asset_import` and `asset_list` return compact metadata: `name`, `width`,
`height`, `bytes`, and `sha256`. Use `asset_get_data_url` when the full base64
payload is needed.
```

Add the MCP client tool cache note:

```markdown
Some MCP clients cache tool discovery for the current session. Restart the MCP
client session after upgrading MCGUI Crafter if newly added tools are missing.
```

- [ ] **Step 2: Update local skill**

In `.agents/skills/mc-gui-crafter/SKILL.md`, change the slot-heavy guidance to prefer:

```markdown
Use `slot_grid_add` for player inventory, hotbar, storage grids, and repeated
machine slot grids. Use `element_add_many` only when the elements are not a
regular grid.
```

In `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`, replace repeated player inventory/hotbar slot examples with:

```json
{
  "name": "slot_grid_add",
  "arguments": {
    "id_prefix": "player_inv",
    "x": 8,
    "y": 84,
    "columns": 9,
    "rows": 3,
    "slot_role": "player_inventory",
    "inventory_group": "player_inventory",
    "slot_index_start": 9,
    "group_id": "player_inventory_grid",
    "semantic_group_kind": "player_inventory",
    "slot_count": 27
  }
}
```

And:

```json
{
  "name": "slot_grid_add",
  "arguments": {
    "id_prefix": "hotbar",
    "x": 8,
    "y": 142,
    "columns": 9,
    "rows": 1,
    "slot_role": "hotbar",
    "inventory_group": "hotbar",
    "slot_index_start": 0,
    "group_id": "hotbar_grid",
    "semantic_group_kind": "hotbar",
    "slot_count": 9
  }
}
```

- [ ] **Step 3: Update roadmap**

In `docs/roadmap.md`, add a checked Phase 6.x item:

```markdown
- [x] MCP alpha ergonomics: bulk element/grid creation, compact asset metadata, default player inventory/hotbar grids, generated button visuals, and semantic preview warnings
```

- [ ] **Step 4: Validate docs and skill**

Run:

```bash
python /home/inky/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/mc-gui-crafter
git diff --check
```

Expected: skill validator prints `Skill is valid!`; `git diff --check` exits 0.

- [ ] **Step 5: Commit docs and skill**

```bash
git add docs/mcp.md docs/roadmap.md .agents/skills/mc-gui-crafter
git commit -m "docs: describe mcp grid generation workflow"
```

## Task 8: End-to-End Verification

**Files:**
- No source edits unless verification exposes a narrow regression from the previous tasks.
- Update docs only if command names or behavior differ from the implemented surface.

- [ ] **Step 1: Run full backend tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

Expected: PASS with all tests passing.

- [ ] **Step 2: Run frontend checks and build**

Run:

```bash
pnpm check
pnpm build
```

Expected: `pnpm check` reports 0 errors and 0 warnings. `pnpm build` exits 0; the existing Vite large chunk warning is acceptable.

- [ ] **Step 3: Verify live MCP workflow**

Against a running `pnpm tauri dev` app, call:

1. `gui_template_list` and verify existing templates still list.
2. `project_new` with:

```json
{
  "name": "Alpha Ergonomics Wide Empty",
  "template": "empty",
  "width": 264,
  "height": 162,
  "mod_target": "forge"
}
```

Expected: project size is `264x162` and generated assets include `textures/generated/gui_panel.png`.

3. `slot_grid_add` for player inventory and hotbar using the examples from Task 7.

Expected: `element_list` shows 36 total player/hotbar slots with effective `layer: "background"`; `group_list` includes `player_inventory_grid` and `hotbar_grid`.

4. `element_add` two buttons:

```json
{
  "element": {
    "id": "threshold_button",
    "type": "button",
    "x": 184,
    "y": 18,
    "width": 64,
    "height": 20,
    "content": "Half",
    "layer": "overlay"
  }
}
```

Expected: editor and export preview treat the buttons as visible elements.

5. `project_export_preview` with `class_name: "AlphaGeneratedScreen"`.

Expected: file list contains `AlphaGeneratedScreen.java`, not `AlphaGeneratedScreenScreen.java`.

6. Add a semantic mismatch project or update semantic groups so `player_inventory.slot_count = 27` with fewer matching slots.

Expected: `project_export_preview` returns a warning mentioning the semantic group id, expected count, and actual matching count.

- [ ] **Step 4: Inspect exported files**

Run an export to `/tmp/mcgui-alpha-ergonomics-export` and inspect:

```bash
find /tmp/mcgui-alpha-ergonomics-export -type f | sort
```

Expected:

- Java screen file does not duplicate `Screen`.
- Layout JSON includes player inventory/hotbar semantic groups.
- GUI or overlay PNG contains button background pixels.
- `GuiSemanticRegistry.java` appears only when modular export and semantic registry are enabled.

- [ ] **Step 5: Final verification**

Run:

```bash
git diff --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm check
```

Expected: all commands exit 0.

- [ ] **Step 6: Commit verification notes if docs changed**

If E2E behavior required documentation corrections:

```bash
git add docs/mcp.md .agents/skills/mc-gui-crafter
git commit -m "docs: align mcp alpha workflow with verification"
```

If no docs changed, do not create an empty commit.

## Self-Review

- Spec coverage: MCP bulk tools, schema enum documentation, compact asset responses, effective layer presentation, empty custom size, default player inventory/hotbar grids, grouped grids, button visuals, non-duplicating `Screen` names, semantic preview warnings, docs, skill updates, and E2E verification are each mapped to tasks.
- Scope: the plan does not implement full runtime button click behavior or full Minecraft menu/container generation.
- Type consistency: MCP fields use `snake_case` matching existing project JSON (`slot_role`, `inventory_group`, `semantic_group_kind`, `slot_index_start`).
- Verification: each backend feature has focused Rust tests, frontend rendering has `pnpm check`, and Task 8 covers live MCP behavior plus full backend/frontend checks.
