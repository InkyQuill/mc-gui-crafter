# MCP Reliability Alpha Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the closed-alpha MCP workflow reliable enough for agents to create, edit, render, validate, save, and export MCGUI Crafter projects without undocumented workarounds.

**Architecture:** Keep the existing local JSON-RPC MCP server in `src-tauri/src/mcp/mod.rs`, and add focused model/export helpers only where MCP reliability requires persistent project data or preview validation. Preserve `.mcgui` compatibility by adding optional/defaulted fields and by keeping existing tools as aliases where possible. Use Rust unit tests around the MCP handler and export preview logic as the primary safety net.

**Tech Stack:** Rust/Tauri 2 backend, Serde project format, JSON-RPC MCP over local HTTP, `image` PNG inspection/composition, inline Rust tests, Markdown docs.

---

## File Structure

- Modify `src-tauri/src/mcp/mod.rs`: tool definitions, `project_render` alias, `project_resize`, `group_upsert`, `element_update_many`, `schema_discover`, compact response payloads, and MCP tests.
- Modify `src-tauri/src/project/mod.rs`: optional semantic group member ids and any small helper needed to resize projects or inspect semantic membership.
- Modify `src-tauri/src/export/mod.rs`: semantic validation for explicit member ids and control button membership.
- Modify `docs/mcp.md`: document new/changed MCP tools, schemas, and validation behavior.
- Modify `.agents/skills/mc-gui-crafter/SKILL.md`: update the local agent skill so LLM users prefer `project_render`, `project_resize`, `group_upsert`, `element_update_many`, and schema discovery.
- Modify `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`: add the closed-alpha workflow notes and examples.
- Modify `docs/roadmap.md`: mark MCP reliability alpha plan/spec status and keep deferred items explicit.

## Task 1: Render Tool Alias And Discovery

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `docs/mcp.md`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`
- Modify: `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`

- [ ] **Step 1: Add failing discovery test for `project_render`**

Add this test in `src-tauri/src/mcp/mod.rs` inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn tools_list_exposes_project_render_as_preferred_visual_tool() {
    let tools = get_tool_definitions();
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"project_render"));
    assert!(names.contains(&"project_screenshot"));

    let render = tools
        .iter()
        .find(|tool| tool["name"] == "project_render")
        .expect("project_render should be listed");
    assert!(render["description"]
        .as_str()
        .unwrap()
        .contains("Render"));
    assert!(render["inputSchema"]["properties"]
        .as_object()
        .unwrap()
        .contains_key("include_data_url"));
}
```

- [ ] **Step 2: Add failing behavior test for `project_render`**

Add this test next to the existing `project_screenshot_writes_compact_png_metadata` tests:

```rust
#[test]
fn project_render_writes_compact_png_metadata() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Render", 64, 32, ModTarget::Forge);
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "slot_a",
                "type": "slot",
                "x": 8,
                "y": 8,
                "size": 18
            }))
            .unwrap(),
        );
        sessions.create_session(project)
    };
    let output_path = TempPath::new("mc-gui-crafter-render-test", "png");
    let output_path_string = output_path.path_string();

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "render",
            "method": "tools/call",
            "params": {
                "name": "project_render",
                "arguments": {
                    "project_id": project_id,
                    "output_path": output_path_string
                }
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    let value = tool_text_value(&response);
    assert_eq!(value["project_id"], project_id);
    assert_eq!(value["width"], 64);
    assert_eq!(value["height"], 32);
    assert!(value["bytes"].as_u64().unwrap() > 0);
    assert_eq!(value["sha256"].as_str().unwrap().len(), 64);
    assert!(value.get("data_url").is_none());
    assert_eq!(value["path"], output_path.path_string());
    assert!(output_path.path().exists());
}
```

- [ ] **Step 3: Run the focused failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tools_list_exposes_project_render_as_preferred_visual_tool project_render_writes_compact_png_metadata
```

Expected: FAIL because `project_render` is not listed or routed yet.

- [ ] **Step 4: Add `project_render` definition and non-mutating classification**

In `get_tool_definitions()`, add `project_render` before `project_screenshot` and keep `project_screenshot` as a backward-compatible alias:

```rust
td(
    "project_render",
    "Render the current project to a PNG image and return compact metadata",
    project_props(&[
        (
            "output_path",
            "string",
            "Optional PNG path to write; temp file is used when omitted",
            false,
        ),
        (
            "include_data_url",
            "boolean",
            "Include data:image/png;base64 payload; defaults to false",
            false,
        ),
    ]),
),
td(
    "project_screenshot",
    "Deprecated alias for project_render; renders the current project to a PNG image",
    project_props(&[
        (
            "output_path",
            "string",
            "Optional PNG path to write; temp file is used when omitted",
            false,
        ),
        (
            "include_data_url",
            "boolean",
            "Include data:image/png;base64 payload; defaults to false",
            false,
        ),
    ]),
),
```

Add both render tools to the non-mutating branch of `is_mutating_tool()`:

```rust
            | "project_render"
            | "project_screenshot"
```

- [ ] **Step 5: Route `project_render` through the existing renderer**

In `execute_tool()`, route both names to the same implementation:

```rust
"project_render" | "project_screenshot" => project_render(&sessions, project_id, args),
```

Rename `project_screenshot` to `project_render`, and add `project_id` to the response:

```rust
fn project_render(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(project_id)?;
    let png = crate::texture::composite_project_preview(&session.project)?;
    let path = optional_string(args, "output_path")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "mc-gui-crafter-render-{}.png",
                uuid::Uuid::new_v4()
            ))
        });
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        return Err("output_path must end with .png".to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create render directory: {error}"))?;
    }
    std::fs::write(&path, &png).map_err(|error| format!("Failed to write render PNG: {error}"))?;

    let image = image::load_from_memory(&png)
        .map_err(|error| format!("Failed to inspect render PNG: {error}"))?;
    let mut metadata = compact_asset_metadata_with_dimensions(
        path.to_string_lossy().as_ref(),
        &png,
        image.width(),
        image.height(),
    );
    metadata["project_id"] = serde_json::json!(session.id);
    metadata["path"] = serde_json::json!(path.to_string_lossy().to_string());
    if optional_bool(args, "include_data_url")?.unwrap_or(false) {
        metadata["data_url"] = serde_json::json!(data_url_for_png(&png));
    }
    Ok(metadata)
}
```

- [ ] **Step 6: Keep old screenshot tests passing**

Update existing tests named `project_screenshot_*` only if they call the renamed Rust function directly. JSON-RPC calls to `"project_screenshot"` should continue to pass unchanged because the tool remains an alias.

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project_render project_screenshot
```

Expected: PASS.

- [ ] **Step 7: Document preferred render usage**

In `docs/mcp.md`, add a short section under the tool list:

```markdown
### Visual verification

Use `project_render` after meaningful visual edits. It writes a PNG and returns
compact metadata: `project_id`, `path`, `width`, `height`, `bytes`, and
`sha256`. Set `include_data_url: true` only when the caller explicitly needs the
PNG payload. `project_screenshot` remains available as a deprecated alias.
```

In `.agents/skills/mc-gui-crafter/SKILL.md`, replace any recommendation to use
`project_screenshot` with `project_render`.

In `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`, add this MCP call example:

```json
{
  "project_id": "11111111-2222-3333-4444-555555555555",
  "output_path": "docs/mcgui/screenshots/example.png"
}
```

- [ ] **Step 8: Run checks and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project_render project_screenshot
```

Expected: PASS.

Commit:

```bash
git add src-tauri/src/mcp/mod.rs docs/mcp.md .agents/skills/mc-gui-crafter/SKILL.md .agents/skills/mc-gui-crafter/references/mcp-workflows.md
git commit -m "feat: expose project render mcp tool"
```

## Task 2: Project Resize Tool

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `docs/mcp.md`

- [ ] **Step 1: Add failing discovery and behavior tests**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn tools_list_exposes_project_resize() {
    let tools = get_tool_definitions();
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"project_resize"));
}

#[test]
fn project_resize_changes_only_gui_size_and_preserves_elements() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Resize", 176, 166, ModTarget::Forge);
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "outside_slot",
                "type": "slot",
                "x": 200,
                "y": -12,
                "size": 18
            }))
            .unwrap(),
        );
        sessions.create_session(project)
    };

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "resize",
            "method": "tools/call",
            "params": {
                "name": "project_resize",
                "arguments": {
                    "project_id": project_id,
                    "width": 264,
                    "height": 162
                }
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    let value = tool_text_value(&response);
    assert_eq!(value["project_id"], project_id);
    assert_eq!(value["old_size"], serde_json::json!({ "width": 176, "height": 166 }));
    assert_eq!(value["new_size"], serde_json::json!({ "width": 264, "height": 162 }));

    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(Some(&project_id)).unwrap();
    assert_eq!(session.project.gui_size.width, 264);
    assert_eq!(session.project.gui_size.height, 162);
    let element = session.project.find_element("outside_slot").unwrap();
    assert_eq!(element.x, 200);
    assert_eq!(element.y, -12);
    assert_eq!(session.revision, 1);
    assert!(session.project.is_dirty);
}

#[test]
fn project_resize_no_op_does_not_change_revision() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.create_session(Project::new("Resize Noop", 176, 166, ModTarget::Forge))
    };
    let before = mutation_snapshot_for(&state, &project_id);

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "resize-noop",
            "method": "tools/call",
            "params": {
                "name": "project_resize",
                "arguments": {
                    "project_id": project_id,
                    "width": 176,
                    "height": 166
                }
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    assert_eq!(mutation_snapshot_for(&state, &project_id), before);
    assert_eq!(revision_for(&state, &project_id), 0);
}

#[test]
fn project_resize_rejects_zero_dimensions_without_mutation() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.create_session(Project::new("Resize Bad", 176, 166, ModTarget::Forge))
    };

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "resize-bad",
            "method": "tools/call",
            "params": {
                "name": "project_resize",
                "arguments": {
                    "project_id": project_id,
                    "width": 0,
                    "height": 166
                }
            }
        }),
        &state,
    );

    assert_eq!(response["error"]["message"], "Project dimensions must be greater than zero");
    assert_eq!(revision_for(&state, &project_id), 0);
}
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project_resize
```

Expected: FAIL because the tool does not exist.

- [ ] **Step 3: Add schema and route**

In `get_tool_definitions()`:

```rust
td(
    "project_resize",
    "Resize the project GUI canvas without moving or scaling elements",
    project_props(&[
        ("width", "integer", "New GUI width; must be greater than zero", true),
        ("height", "integer", "New GUI height; must be greater than zero", true),
    ]),
),
```

In `execute_tool()`:

```rust
"project_resize" => project_resize(&mut sessions, project_id, args),
```

- [ ] **Step 4: Implement resize behavior**

Add this function near `project_summary()`:

```rust
fn project_resize(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let width = required_u32(args, "width")?;
    let height = required_u32(args, "height")?;
    if width == 0 || height == 0 {
        return Err("Project dimensions must be greater than zero".to_string());
    }

    let session = sessions.resolve(project_id)?;
    let old_size = session.project.gui_size.clone();
    let new_size = crate::project::Size { width, height };
    if old_size == new_size {
        return Ok(serde_json::json!({
            "project_id": session.id,
            "old_size": old_size,
            "new_size": new_size,
            "changed": false
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.gui_size = new_size.clone();
    let session_id = session.id.clone();
    sessions.mark_changed(project_id)?;

    Ok(serde_json::json!({
        "project_id": session_id,
        "old_size": old_size,
        "new_size": new_size,
        "changed": true
    }))
}
```

- [ ] **Step 5: Document `project_resize`**

In `docs/mcp.md`, add:

```markdown
### `project_resize`

Changes `gui_size` only. It does not move, scale, clamp, or delete elements,
including elements outside the new bounds. Agents should move affected elements
explicitly after resizing.
```

- [ ] **Step 6: Run checks and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project_resize
cargo test --manifest-path src-tauri/Cargo.toml project_new_empty_template_respects_requested_dimensions
```

Expected: PASS.

Commit:

```bash
git add src-tauri/src/mcp/mod.rs docs/mcp.md
git commit -m "feat: add project resize mcp tool"
```

## Task 3: Group Upsert Tool

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `docs/mcp.md`
- Modify: `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`

- [ ] **Step 1: Add failing tests for discovery, create, update, and no-op**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn tools_list_exposes_group_upsert() {
    let tools = get_tool_definitions();
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"group_upsert"));
}

#[test]
fn group_upsert_creates_and_updates_existing_group() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Group Upsert", 176, 166, ModTarget::Forge);
        for (id, x) in [("a", 8), ("b", 26), ("c", 44)] {
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": id,
                    "type": "slot",
                    "x": x,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
        }
        sessions.create_session(project)
    };

    let create_response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "group-upsert-create",
            "method": "tools/call",
            "params": {
                "name": "group_upsert",
                "arguments": {
                    "project_id": project_id,
                    "group_id": "machine",
                    "element_ids": ["a", "b"]
                }
            }
        }),
        &state,
    );
    assert!(create_response["error"].is_null(), "{create_response:#}");
    let create_value = tool_text_value(&create_response);
    assert_eq!(create_value["project_id"], project_id);
    assert_eq!(create_value["created"], true);
    assert_eq!(create_value["updated"], false);
    assert_eq!(create_value["member_count"], 2);

    let update_response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "group-upsert-update",
            "method": "tools/call",
            "params": {
                "name": "group_upsert",
                "arguments": {
                    "project_id": project_id,
                    "group_id": "machine",
                    "element_ids": ["a", "c"]
                }
            }
        }),
        &state,
    );
    assert!(update_response["error"].is_null(), "{update_response:#}");
    let update_value = tool_text_value(&update_response);
    assert_eq!(update_value["created"], false);
    assert_eq!(update_value["updated"], true);
    assert_eq!(update_value["member_count"], 2);

    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(Some(&project_id)).unwrap();
    assert_eq!(session.project.groups.len(), 1);
    assert_eq!(session.project.groups[0].elements, vec!["a".to_string(), "c".to_string()]);
    assert_eq!(session.project.groups[0].x, 8);
    assert_eq!(session.project.groups[0].y, 18);
    assert_eq!(session.revision, 2);
}

#[test]
fn group_upsert_no_op_does_not_change_revision() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Group Upsert Noop", 176, 166, ModTarget::Forge);
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "a",
                "type": "slot",
                "x": 8,
                "y": 18,
                "size": 18
            }))
            .unwrap(),
        );
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "b",
                "type": "slot",
                "x": 26,
                "y": 18,
                "size": 18
            }))
            .unwrap(),
        );
        project.groups.push(crate::project::Group {
            id: "machine".to_string(),
            x: 8,
            y: 18,
            elements: vec!["a".to_string(), "b".to_string()],
        });
        sessions.create_session(project)
    };
    let before = mutation_snapshot_for(&state, &project_id);

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "group-upsert-noop",
            "method": "tools/call",
            "params": {
                "name": "group_upsert",
                "arguments": {
                    "project_id": project_id,
                    "group_id": "machine",
                    "element_ids": ["a", "b"]
                }
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    assert_eq!(mutation_snapshot_for(&state, &project_id), before);
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml group_upsert
```

Expected: FAIL because `group_upsert` does not exist.

- [ ] **Step 3: Add schema and route**

In `get_tool_definitions()` near group tools:

```rust
td(
    "group_upsert",
    "Create or replace a project group membership without ungrouping first",
    project_props(&[
        ("group_id", "string", "Group ID", true),
        ("element_ids", "array", "Replacement element IDs for the group", true),
    ]),
),
```

In `execute_tool()`:

```rust
"group_upsert" => group_upsert(&mut sessions, project_id, args),
```

- [ ] **Step 4: Reuse group validation for upsert**

Change `validate_group_create()` into a helper that accepts existing group ids:

```rust
fn validate_group_members(
    sessions: &ProjectSessionManager,
    project_id: Option<&str>,
    element_ids: &[String],
) -> Result<Vec<String>, String> {
    let project = &sessions.resolve(project_id)?.project;
    let mut unique_ids = Vec::new();
    for id in element_ids {
        if !unique_ids.contains(id) {
            unique_ids.push(id.clone());
        }
        if project.find_element(id).is_none() {
            return Err(format!("Element not found: {id}"));
        }
    }
    if unique_ids.len() < 2 {
        return Err("At least two elements are required to create a group".to_string());
    }
    Ok(unique_ids)
}
```

Keep `group_create()` behavior by checking duplicate group id before calling this helper.

- [ ] **Step 5: Implement `group_upsert`**

Add:

```rust
fn group_upsert(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let group_id = required_str(args, "group_id")?.to_string();
    let element_ids = string_array(args, "element_ids")?;
    let element_ids = validate_group_members(sessions, project_id, &element_ids)?;
    let project = &sessions.resolve(project_id)?.project;
    let existing = project.groups.iter().find(|group| group.id == group_id);
    let created = existing.is_none();
    let x = min_element_coordinate(project, &element_ids, true)?;
    let y = min_element_coordinate(project, &element_ids, false)?;
    let next = crate::project::Group {
        id: group_id.clone(),
        x,
        y,
        elements: element_ids,
    };

    if existing == Some(&next) {
        let session = sessions.resolve(project_id)?;
        return Ok(serde_json::json!({
            "project_id": session.id,
            "group": next,
            "created": false,
            "updated": false,
            "member_count": next.elements.len()
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    if let Some(group) = session.project.groups.iter_mut().find(|group| group.id == group_id) {
        *group = next.clone();
    } else {
        session.project.groups.push(next.clone());
    }
    let session_id = session.id.clone();
    sessions.mark_changed(project_id)?;
    Ok(serde_json::json!({
        "project_id": session_id,
        "group": next,
        "created": created,
        "updated": !created,
        "member_count": next.elements.len()
    }))
}
```

Add helper functions:

```rust
fn string_array(value: &serde_json::Value, key: &str) -> Result<Vec<String>, String> {
    value
        .get(key)
        .and_then(|value| value.as_array())
        .ok_or(format!("Missing {key}"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or(format!("{key} must contain only strings"))
        })
        .collect()
}

fn min_element_coordinate(project: &Project, element_ids: &[String], x_axis: bool) -> Result<i32, String> {
    element_ids
        .iter()
        .filter_map(|id| project.find_element(id))
        .map(|element| if x_axis { element.x } else { element.y })
        .min()
        .ok_or("Group must contain at least one existing element".to_string())
}
```

- [ ] **Step 6: Document and commit**

In `docs/mcp.md`, add:

```markdown
### `group_upsert`

Use `group_upsert` when editing existing groups. It creates a group if missing
or replaces membership if present, preserving a single history entry and
avoiding the `group_ungroup` plus `group_create` workaround.
```

In `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`, replace any
group update workaround with `group_upsert`.

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml group_upsert group_create
```

Expected: PASS.

Commit:

```bash
git add src-tauri/src/mcp/mod.rs docs/mcp.md .agents/skills/mc-gui-crafter/references/mcp-workflows.md
git commit -m "feat: add group upsert mcp tool"
```

## Task 4: Element Update Many

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `docs/mcp.md`

- [ ] **Step 1: Add failing tests**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn tools_list_exposes_element_update_many() {
    let tools = get_tool_definitions();
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"element_update_many"));
}

#[test]
fn element_update_many_updates_multiple_elements_in_one_revision() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Update Many", 176, 166, ModTarget::Forge);
        for (id, x) in [("a", 8), ("b", 26)] {
            project.elements.push(
                parse_element_arg(&serde_json::json!({
                    "id": id,
                    "type": "slot",
                    "x": x,
                    "y": 18,
                    "size": 18
                }))
                .unwrap(),
            );
        }
        sessions.create_session(project)
    };

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "update-many",
            "method": "tools/call",
            "params": {
                "name": "element_update_many",
                "arguments": {
                    "project_id": project_id,
                    "updates": [
                        { "id": "a", "changes": { "x": 10, "y": 20 } },
                        { "id": "b", "changes": { "x": 30, "y": 40, "slot_index": 7 } }
                    ]
                }
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    let value = tool_text_value(&response);
    assert_eq!(value["project_id"], project_id);
    assert_eq!(value["updated_count"], 2);
    assert_eq!(value["results"].as_array().unwrap().len(), 2);

    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(Some(&project_id)).unwrap();
    assert_eq!(session.project.find_element("a").unwrap().x, 10);
    assert_eq!(session.project.find_element("b").unwrap().slot_index, Some(7));
    assert_eq!(session.revision, 1);
}

#[test]
fn element_update_many_strict_failure_is_atomic() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Update Many Atomic", 176, 166, ModTarget::Forge);
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "a",
                "type": "slot",
                "x": 8,
                "y": 18,
                "size": 18
            }))
            .unwrap(),
        );
        sessions.create_session(project)
    };

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "update-many-bad",
            "method": "tools/call",
            "params": {
                "name": "element_update_many",
                "arguments": {
                    "project_id": project_id,
                    "updates": [
                        { "id": "a", "changes": { "x": 10 } },
                        { "id": "missing", "changes": { "x": 30 } }
                    ]
                }
            }
        }),
        &state,
    );

    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Element not found: missing"));
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(Some(&project_id)).unwrap();
    assert_eq!(session.project.find_element("a").unwrap().x, 8);
    assert_eq!(session.revision, 0);
}

#[test]
fn element_update_many_no_op_does_not_change_revision() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Update Many Noop", 176, 166, ModTarget::Forge);
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "a",
                "type": "slot",
                "x": 8,
                "y": 18,
                "size": 18
            }))
            .unwrap(),
        );
        sessions.create_session(project)
    };
    let before = mutation_snapshot_for(&state, &project_id);

    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "update-many-noop",
            "method": "tools/call",
            "params": {
                "name": "element_update_many",
                "arguments": {
                    "project_id": project_id,
                    "updates": [
                        { "id": "a", "changes": { "x": 8, "y": 18 } }
                    ]
                }
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    assert_eq!(mutation_snapshot_for(&state, &project_id), before);
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml element_update_many
```

Expected: FAIL because the tool is not implemented.

- [ ] **Step 3: Add schema and route**

In `get_tool_definitions()` near `element_update`:

```rust
td(
    "element_update_many",
    "Update multiple elements atomically in one project revision",
    project_schema(vec![(
        "updates",
        serde_json::json!({
            "type": "array",
            "description": "Element update patches",
            "items": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "changes": { "type": "object" }
                },
                "required": ["id", "changes"]
            }
        }),
        true,
    )]),
),
```

In `execute_tool()`:

```rust
"element_update_many" => element_update_many(&mut sessions, project_id, args),
```

- [ ] **Step 4: Extract reusable element patch function**

Refactor `element_update()` to use this helper:

```rust
fn apply_element_changes(current: &Element, changes: &serde_json::Map<String, serde_json::Value>) -> Result<Element, String> {
    let mut value = serde_json::to_value(current).map_err(|error| error.to_string())?;
    let target = value
        .as_object_mut()
        .ok_or("Element payload must be an object")?;
    for (key, value) in changes {
        if key == "id" || key == "type" {
            continue;
        }
        target.insert(key.clone(), value.clone());
    }
    serde_json::from_value(value).map_err(|error| format!("Invalid element update: {error}"))
}
```

Then update `element_update()`:

```rust
let updated = apply_element_changes(current, changes)?;
```

- [ ] **Step 5: Implement `element_update_many`**

Add:

```rust
#[derive(Debug)]
struct ElementPatch {
    id: String,
    changes: serde_json::Map<String, serde_json::Value>,
}

fn parse_element_patches(args: &serde_json::Value) -> Result<Vec<ElementPatch>, String> {
    let updates = args
        .get("updates")
        .and_then(|value| value.as_array())
        .ok_or("Missing updates")?;
    if updates.is_empty() {
        return Err("updates array cannot be empty".to_string());
    }
    updates
        .iter()
        .map(|update| {
            let object = update.as_object().ok_or("Each update must be an object")?;
            let id = object
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("Each update requires an id")?
                .to_string();
            let changes = object
                .get("changes")
                .and_then(|value| value.as_object())
                .ok_or("Each update requires object changes")?
                .clone();
            Ok(ElementPatch { id, changes })
        })
        .collect()
}

fn element_update_many(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let patches = parse_element_patches(args)?;
    let session = sessions.resolve(project_id)?;
    let mut updated = Vec::new();

    for patch in &patches {
        let current = session
            .project
            .find_element(&patch.id)
            .ok_or_else(|| format!("Element not found: {}", patch.id))?;
        updated.push(apply_element_changes(current, &patch.changes)?);
    }

    let changed = updated.iter().any(|element| {
        session
            .project
            .find_element(&element.id)
            .is_some_and(|current| current != element)
    });
    if !changed {
        return Ok(serde_json::json!({
            "project_id": session.id,
            "updated_count": 0,
            "results": updated.iter().map(element_for_mcp).collect::<Vec<_>>()
        }));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    for element in &updated {
        *session
            .project
            .find_element_mut(&element.id)
            .ok_or_else(|| format!("Element not found: {}", element.id))? = element.clone();
    }
    let session_id = session.id.clone();
    sessions.mark_changed(project_id)?;

    Ok(serde_json::json!({
        "project_id": session_id,
        "updated_count": updated.len(),
        "results": updated.iter().map(element_for_mcp).collect::<Vec<_>>()
    }))
}
```

- [ ] **Step 6: Document and commit**

In `docs/mcp.md`, add:

```markdown
### `element_update_many`

Applies multiple `element_update`-style patches atomically in one revision. If
any element is missing or any patch is invalid, no element is changed.
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml element_update element_update_many
```

Expected: PASS.

Commit:

```bash
git add src-tauri/src/mcp/mod.rs docs/mcp.md
git commit -m "feat: add batch element update mcp tool"
```

## Task 5: Explicit Semantic Members And Validation

**Files:**
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `src-tauri/src/export/mod.rs`
- Modify: `docs/mcp.md`

- [ ] **Step 1: Add failing project model round-trip test**

Add this test in `src-tauri/src/project/mod.rs` under the existing test module, or create one if absent:

```rust
#[test]
fn semantic_group_member_ids_round_trip_with_default_empty() {
    let json = serde_json::json!({
        "id": "controls",
        "kind": "control_buttons",
        "member_ids": ["settings_button", "lock_button"],
        "data_source": "pouch_settings"
    });

    let group: SemanticGroup = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(group.member_ids, vec!["settings_button", "lock_button"]);
    assert_eq!(serde_json::to_value(&group).unwrap()["member_ids"], json["member_ids"]);

    let without_members: SemanticGroup = serde_json::from_value(serde_json::json!({
        "id": "inventory",
        "kind": "player_inventory"
    }))
    .unwrap();
    assert!(without_members.member_ids.is_empty());
    assert!(serde_json::to_value(&without_members)
        .unwrap()
        .get("member_ids")
        .is_none());
}
```

- [ ] **Step 2: Add failing schema test**

In `src-tauri/src/mcp/mod.rs`, update or add:

```rust
#[test]
fn semantic_groups_schema_exposes_member_ids() {
    let tools = get_tool_definitions();
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == "project_semantic_groups_update")
        .unwrap();
    let properties = &tool["inputSchema"]["properties"]["semantic_groups"]["items"]["properties"];

    assert_eq!(properties["member_ids"]["type"], "array");
    assert_eq!(properties["member_ids"]["items"]["type"], "string");
}
```

- [ ] **Step 3: Add failing export warning tests**

In `src-tauri/src/export/mod.rs`, add tests inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn semantic_warnings_report_missing_explicit_members() {
    let mut project = Project::new("Missing Members", 176, 166, crate::project::ModTarget::Forge);
    project.export_settings.codegen_mode = CodegenMode::Modular;
    project.semantic_groups.push(SemanticGroup {
        id: "controls".to_string(),
        kind: SemanticGroupKind::ControlButtons,
        columns: None,
        visible_rows: None,
        total_rows: None,
        slot_count: None,
        data_source: Some("settings".to_string()),
        scroll_binding: None,
        dynamic_height: false,
        member_ids: vec!["missing_button".to_string()],
    });

    let warnings = semantic_warnings(&project, &project.export_settings);

    assert!(warnings
        .iter()
        .any(|warning| warning.contains("references missing element 'missing_button'")));
}

#[test]
fn semantic_warnings_report_non_button_control_members() {
    let mut project = Project::new("Wrong Control Members", 176, 166, crate::project::ModTarget::Forge);
    project.export_settings.codegen_mode = CodegenMode::Modular;
    project.elements.push(crate::project::Element {
        id: "slot_as_button".to_string(),
        element_type: ElementType::Slot,
        x: 8,
        y: 18,
        width: None,
        height: None,
        size: Some(18),
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
    });
    project.semantic_groups.push(SemanticGroup {
        id: "controls".to_string(),
        kind: SemanticGroupKind::ControlButtons,
        columns: None,
        visible_rows: None,
        total_rows: None,
        slot_count: None,
        data_source: Some("settings".to_string()),
        scroll_binding: None,
        dynamic_height: false,
        member_ids: vec!["slot_as_button".to_string()],
    });

    let warnings = semantic_warnings(&project, &project.export_settings);

    assert!(warnings
        .iter()
        .any(|warning| warning.contains("references non-button element 'slot_as_button'")));
}
```

If direct `Element` construction is too verbose because fields changed, use the existing `parse_element_arg()` helper if it is available in the export tests, or add a small local `slot_element(id: &str) -> Element` helper in the test module.

- [ ] **Step 4: Run failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml semantic_group_member_ids semantic_groups_schema_exposes_member_ids semantic_warnings_report_missing_explicit_members semantic_warnings_report_non_button_control_members
```

Expected: FAIL because `member_ids` does not exist.

- [ ] **Step 5: Add `member_ids` to the project model**

In `src-tauri/src/project/mod.rs`, extend `SemanticGroup`:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub member_ids: Vec<String>,
```

Update every `SemanticGroup { ... }` literal in Rust tests and code to include:

```rust
member_ids: Vec::new(),
```

For `slot_grid_add`, set generated member ids when creating a semantic group:

```rust
member_ids: elements.iter().map(|element| element.id.clone()).collect(),
```

- [ ] **Step 6: Update MCP semantic group schema**

In `semantic_groups_props()`, add:

```rust
"member_ids": {
    "type": "array",
    "description": "Explicit element IDs that belong to this semantic group",
    "items": { "type": "string" }
}
```

- [ ] **Step 7: Update semantic validation**

In `src-tauri/src/export/mod.rs`, add helper:

```rust
fn explicit_member_elements<'a>(project: &'a Project, group: &SemanticGroup) -> Vec<Result<&'a crate::project::Element, String>> {
    group
        .member_ids
        .iter()
        .map(|id| {
            project
                .elements
                .iter()
                .find(|element| element.id == *id)
                .ok_or_else(|| format!("Semantic group '{}' references missing element '{}'.", group.id, id))
        })
        .collect()
}
```

In `semantic_integrity_warnings()`, before slot count/control checks, append missing member warnings:

```rust
for member in explicit_member_elements(project, group) {
    if let Err(warning) = member {
        warnings.push(warning);
    }
}
```

Update `count_matching_group_elements()`:

```rust
fn count_matching_group_elements(project: &Project, group: &SemanticGroup) -> u32 {
    if !group.member_ids.is_empty() {
        return group
            .member_ids
            .iter()
            .filter_map(|id| project.elements.iter().find(|element| element.id == *id))
            .filter(|element| element.element_type == ElementType::Slot)
            .filter(|element| slot_role_matches_group(element.slot_role.as_ref(), &group.kind))
            .count() as u32;
    }

    project
        .elements
        .iter()
        .filter(|element| element.element_type == ElementType::Slot)
        .filter(|element| element.inventory_group.as_deref() == Some(group.id.as_str()))
        .filter(|element| slot_role_matches_group(element.slot_role.as_ref(), &group.kind))
        .count() as u32
}
```

Update `control_button_warnings()` to validate explicit members first:

```rust
if !group.member_ids.is_empty() {
    let mut warnings = Vec::new();
    for id in &group.member_ids {
        let Some(element) = project.elements.iter().find(|element| element.id == *id) else {
            continue;
        };
        if !matches!(element.element_type, ElementType::Button | ElementType::ToggleButton) {
            warnings.push(format!(
                "Semantic group '{}' references non-button element '{}'.",
                group.id, id
            ));
        }
    }
    return warnings;
}
```

- [ ] **Step 8: Run semantic tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml semantic_group_member_ids semantic_groups_schema_exposes_member_ids semantic_warnings
```

Expected: PASS.

Update `docs/mcp.md`:

```markdown
Semantic groups may include `member_ids` for explicit membership. Use it for
non-rectangular fixed slot groups and control button groups. Export preview
warns when explicit members are missing or have the wrong element type.
```

Commit:

```bash
git add src-tauri/src/project/mod.rs src-tauri/src/mcp/mod.rs src-tauri/src/export/mod.rs docs/mcp.md
git commit -m "feat: validate explicit semantic group members"
```

## Task 6: Schema Discovery Tool

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `docs/mcp.md`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`

- [ ] **Step 1: Add failing tests**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn tools_list_exposes_schema_discover() {
    let tools = get_tool_definitions();
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"schema_discover"));
}

#[test]
fn schema_discover_returns_agent_authoring_enums_and_defaults() {
    let state = test_state();
    let response = response_for(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "schema",
            "method": "tools/call",
            "params": {
                "name": "schema_discover",
                "arguments": {}
            }
        }),
        &state,
    );

    assert!(response["error"].is_null(), "{response:#}");
    let value = tool_text_value(&response);
    assert!(value["element_types"].as_array().unwrap().contains(&serde_json::json!("slot")));
    assert!(value["semantic_group_kinds"].as_array().unwrap().contains(&serde_json::json!("control_buttons")));
    assert!(value["slot_roles"].as_array().unwrap().contains(&serde_json::json!("player_inventory")));
    assert!(value["attached_region_anchors"].as_array().unwrap().contains(&serde_json::json!("right")));
    assert!(value["export_settings"]["codegen_modes"].as_array().unwrap().contains(&serde_json::json!("modular")));
    assert_eq!(value["serialization_defaults"]["dynamic_height_false_omitted"], true);
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml schema_discover
```

Expected: FAIL because the tool does not exist.

- [ ] **Step 3: Add schema and route**

In `get_tool_definitions()`:

```rust
td(
    "schema_discover",
    "Return accepted MCP enum values, editable fields, defaults, and alpha schema notes",
    props(&[]),
),
```

Add schema discovery to the non-mutating branch of `is_mutating_tool()`:

```rust
            | "schema_discover"
```

In `execute_tool()`:

```rust
"schema_discover" => Ok(schema_discover()),
```

- [ ] **Step 4: Implement `schema_discover()`**

Add near schema helpers:

```rust
fn schema_discover() -> serde_json::Value {
    serde_json::json!({
        "element_types": [
            "texture",
            "slot",
            "progress",
            "text",
            "fluid_tank",
            "energy_bar",
            "scrollbar",
            "button",
            "toggle_button",
            "text_input",
            "tab",
            "panel",
            "virtual_slot_cell"
        ],
        "slot_roles": [
            "machine",
            "player_inventory",
            "hotbar",
            "scrollable_inventory",
            "virtual_storage",
            "upgrade",
            "upgrade_settings",
            "filter",
            "ghost",
            "offhand"
        ],
        "semantic_group_kinds": [
            "fixed_slots",
            "virtual_slot_grid",
            "player_inventory",
            "hotbar",
            "upgrade_slots",
            "upgrade_panel",
            "search_field",
            "control_buttons"
        ],
        "attached_region_anchors": ["left", "right", "top", "bottom", "free"],
        "attached_region_states": ["static", "toggleable"],
        "export_settings": {
            "codegen_modes": ["simple", "modular"],
            "generate_runtime_helpers_default": true,
            "generate_semantic_registry_default": false
        },
        "editable_element_fields": [
            "x",
            "y",
            "width",
            "height",
            "size",
            "asset",
            "icon",
            "icon_uv",
            "tooltip",
            "direction",
            "content",
            "font",
            "color",
            "shadow",
            "animation",
            "visible",
            "uv",
            "layer",
            "slot_role",
            "slot_index",
            "inventory_group",
            "scroll_binding",
            "scroll_min",
            "scroll_max",
            "visible_rows",
            "total_rows",
            "columns",
            "target_group",
            "binding",
            "dock",
            "open_width",
            "open_height",
            "attached_region"
        ],
        "serialization_defaults": {
            "layer_background_omitted_in_project_json": true,
            "visible_true_omitted": true,
            "dynamic_height_false_omitted": true,
            "attached_region_visible_true_omitted": true
        }
    })
}
```

- [ ] **Step 5: Document schema discovery**

In `docs/mcp.md`, add:

```markdown
### `schema_discover`

Call this before authoring unfamiliar projects. It returns accepted enum values,
editable element fields, export settings, attached-region values, and notes
about default fields that may be omitted from serialized project JSON.
```

In `.agents/skills/mc-gui-crafter/SKILL.md`, add an instruction:

```markdown
When unsure about enum values or editable fields, call `schema_discover`
instead of guessing.
```

- [ ] **Step 6: Run checks and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml schema_discover tools_list_contains_live_session_tools
```

Expected: PASS.

Commit:

```bash
git add src-tauri/src/mcp/mod.rs docs/mcp.md .agents/skills/mc-gui-crafter/SKILL.md
git commit -m "feat: add mcp schema discovery"
```

## Task 7: Compact Response Contract And Mutation Sync Regression Tests

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs`
- Modify: `docs/mcp.md`

- [ ] **Step 1: Add regression tests for compact responses**

Add tests in `src-tauri/src/mcp/mod.rs`:

```rust
#[test]
fn new_alpha_mutation_responses_include_project_id() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Response Contract", 176, 166, ModTarget::Forge);
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "a",
                "type": "slot",
                "x": 8,
                "y": 18,
                "size": 18
            }))
            .unwrap(),
        );
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "b",
                "type": "slot",
                "x": 26,
                "y": 18,
                "size": 18
            }))
            .unwrap(),
        );
        sessions.create_session(project)
    };

    for (tool_name, arguments) in [
        (
            "project_resize",
            serde_json::json!({ "project_id": project_id, "width": 180, "height": 166 }),
        ),
        (
            "group_upsert",
            serde_json::json!({ "project_id": project_id, "group_id": "machine", "element_ids": ["a", "b"] }),
        ),
        (
            "element_update_many",
            serde_json::json!({ "project_id": project_id, "updates": [{ "id": "a", "changes": { "x": 10 } }] }),
        ),
    ] {
        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": tool_name,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": arguments
                }
            }),
            &state,
        );
        assert!(response["error"].is_null(), "{tool_name}: {response:#}");
        let value = tool_text_value(&response);
        assert_eq!(value["project_id"], project_id, "{tool_name}");
    }
}

#[test]
fn project_render_and_asset_list_do_not_inline_binary_payloads_by_default() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Compact", 32, 24, ModTarget::Forge);
        let asset = "textures/generated/gui_panel.png";
        project.assets.push(asset.to_string());
        project
            .texture_data
            .insert(asset.to_string(), crate::texture::generated_gui_panel(16, 16).unwrap());
        sessions.create_session(project)
    };

    for tool_name in ["project_render", "asset_list"] {
        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": tool_name,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": { "project_id": project_id }
                }
            }),
            &state,
        );
        assert!(response["error"].is_null(), "{tool_name}: {response:#}");
        let value = tool_text_value(&response);
        assert!(
            !serde_json::to_string(&value).unwrap().contains("data:image/png;base64"),
            "{tool_name} should be compact by default"
        );
    }
}
```

- [ ] **Step 2: Run contract tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml new_alpha_mutation_responses_include_project_id project_render_and_asset_list_do_not_inline_binary_payloads_by_default
```

Expected: PASS if previous tasks already met the response contract. If it fails, fix the specific response payload rather than adding broad wrappers.

- [ ] **Step 3: Add no-op mutation sync regression test**

Add:

```rust
#[test]
fn no_op_batch_tools_do_not_change_revision() {
    let state = test_state();
    let project_id = {
        let mut sessions = state.sessions.lock().unwrap();
        let mut project = Project::new("Noop Batch", 176, 166, ModTarget::Forge);
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "a",
                "type": "slot",
                "x": 8,
                "y": 18,
                "size": 18
            }))
            .unwrap(),
        );
        project.elements.push(
            parse_element_arg(&serde_json::json!({
                "id": "b",
                "type": "slot",
                "x": 26,
                "y": 18,
                "size": 18
            }))
            .unwrap(),
        );
        project.groups.push(crate::project::Group {
            id: "machine".to_string(),
            x: 8,
            y: 18,
            elements: vec!["a".to_string(), "b".to_string()],
        });
        sessions.create_session(project)
    };

    let calls = [
        (
            "project_resize",
            serde_json::json!({ "project_id": project_id, "width": 176, "height": 166 }),
        ),
        (
            "group_upsert",
            serde_json::json!({ "project_id": project_id, "group_id": "machine", "element_ids": ["a", "b"] }),
        ),
        (
            "element_update_many",
            serde_json::json!({ "project_id": project_id, "updates": [{ "id": "a", "changes": { "x": 8 } }] }),
        ),
    ];

    for (tool_name, arguments) in calls {
        let before = mutation_snapshot_for(&state, &project_id);
        let response = response_for(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": tool_name,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": arguments
                }
            }),
            &state,
        );
        assert!(response["error"].is_null(), "{tool_name}: {response:#}");
        assert_eq!(mutation_snapshot_for(&state, &project_id), before, "{tool_name}");
    }
}
```

- [ ] **Step 4: Run focused and full MCP tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml no_op_batch_tools_do_not_change_revision
cargo test --manifest-path src-tauri/Cargo.toml mcp
```

Expected: PASS.

- [ ] **Step 5: Document response contract**

In `docs/mcp.md`, add:

```markdown
## Alpha response contract

Closed-alpha MCP tools return compact JSON by default. Binary fields such as
PNG data URLs are opt-in. Reliability Alpha mutating tools `project_resize`,
`group_upsert`, and `element_update_many` include `project_id` in their
responses. No-op mutations should not change project revision or trigger UI
synchronization events.
```

- [ ] **Step 6: Commit**

Commit:

```bash
git add src-tauri/src/mcp/mod.rs docs/mcp.md
git commit -m "test: lock mcp alpha response contract"
```

## Task 8: Documentation, Roadmap, And End-To-End Verification

**Files:**
- Modify: `docs/mcp.md`
- Modify: `.agents/skills/mc-gui-crafter/SKILL.md`
- Modify: `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Add closed-alpha MCP workflow documentation**

In `docs/mcp.md`, add a section:

```markdown
## Closed-alpha agent workflow

1. Call `schema_discover` before using unfamiliar enums or semantic groups.
2. Create or open a project with `project_new` or `project_open`.
3. Use `slot_grid_add`, `element_add_many`, and `element_update_many` for bulk
   layout work.
4. Use `group_upsert` when group membership changes.
5. Use `project_resize` only for canvas size changes; move elements explicitly.
6. Use `project_semantic_groups_update` with `member_ids` for non-rectangular
   slot groups and control button groups.
7. Use `project_render` after visual edits and inspect the PNG when possible.
8. Use `project_export_preview` before `project_export`.
9. Save source projects with `project_save_as`.
```

- [ ] **Step 2: Update the local MCGUI Crafter skill**

In `.agents/skills/mc-gui-crafter/SKILL.md`, ensure the workflow says:

```markdown
Prefer these closed-alpha MCP tools:

- `schema_discover` for accepted enums and editable fields.
- `project_render` for visual verification.
- `project_resize` for canvas size changes only.
- `slot_grid_add`, `element_add_many`, and `element_update_many` for bulk edits.
- `group_upsert` for creating or replacing group membership.
- `project_semantic_groups_update` with `member_ids` for explicit semantics.
```

In `.agents/skills/mc-gui-crafter/references/mcp-workflows.md`, add one compact
workflow example that uses `schema_discover`, `project_new`, `slot_grid_add`,
`group_upsert`, `project_render`, `project_export_preview`, and
`project_save_as`.

- [ ] **Step 3: Update roadmap**

In `docs/roadmap.md`, add or update the closed-alpha section with:

```markdown
- [x] MCP Reliability Alpha design/spec written.
- [x] MCP render tool exposed as `project_render`.
- [x] MCP project resize, group upsert, and batch element update workflows.
- [x] MCP schema discovery and explicit semantic member validation.
- [ ] Visual Authoring Alpha.
- [ ] Editable State Variants Alpha.
- [ ] GUI Polish Alpha.
```

If the roadmap already tracks implementation separately from spec status, keep
both statuses explicit instead of replacing existing entries.

- [ ] **Step 4: Run the full Rust test suite**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 5: Run frontend/project checks**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS.

- [ ] **Step 6: Run final diff checks**

Run:

```bash
git diff --check
```

Expected: `git diff --check` exits 0.

- [ ] **Step 7: Commit final docs**

Commit:

```bash
git add docs/mcp.md .agents/skills/mc-gui-crafter/SKILL.md .agents/skills/mc-gui-crafter/references/mcp-workflows.md docs/roadmap.md
git commit -m "docs: update mcp alpha workflow"
```

## Final Verification

After all tasks are complete, run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
pnpm check
pnpm build
git status --short
```

Expected:

- Rust tests pass.
- Svelte/TypeScript check passes.
- Vite build passes.
- `git status --short` contains no unintended files. Local `.superpowers/` artifacts may remain untracked and should not be committed.

## Self-Review Checklist

- MCP Reliability Alpha spec coverage:
  - `project_render`: Task 1.
  - `project_resize`: Task 2.
  - `group_upsert`: Task 3.
  - `element_update_many`: Task 4.
  - explicit semantic members and validation: Task 5.
  - schema discovery: Task 6.
  - compact responses/no-op revision behavior: Task 7.
  - docs/skill/roadmap: Task 8.
- Scope control:
  - no editable state variants;
  - no nine-slice rendering;
  - no desktop UI redesign;
  - no public release packaging.
- Implementation order:
  - each task can be committed independently;
  - each task has focused tests before implementation;
  - docs are updated only after the relevant behavior exists.
