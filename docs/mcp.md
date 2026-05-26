# MCP Server - mc-gui-crafter

MCGUI Crafter hosts an MCP Streamable HTTP endpoint from the running Tauri app
instance. MCP calls mutate the same Rust-owned project sessions used by the UI,
save/export commands, and backend undo/redo.

The app is single-instance on desktop. If a second launch passes a `.mcgui`
project path, the running instance opens that project in a new tab and focuses
the main window.

## Server Info

| Field | Value |
|-------|-------|
| Name | `mc-gui-crafter` |
| Version | app package version |
| Transport | Streamable HTTP-style JSON-RPC |
| Endpoint | `http://127.0.0.1:47381/mcp` by default |
| Capabilities | tools |

The listener binds to localhost with the port stored in
`~/.config/mc-gui-crafter/config.json`. On first launch, the preferred MCP port
is `47381`. If that port is busy, the app falls back to an OS-assigned free port
and writes the selected port back to the same config file.

The selected address is shown in the start panel as `MCP Online`; it can also
be read through the Tauri `mcp_status` command. In the normal case, configure
MCP clients with:

```text
http://127.0.0.1:47381/mcp
```

If the app reports a different URL, update the client config with the URL shown
by the app.

## Connecting MCP Clients

Start MCGUI Crafter first. The MCP server lives inside the running Tauri app,
so clients connect to the app; they do not spawn a separate `mc-gui-crafter`
stdio process.

### Claude Code

Use a local or user-scoped HTTP MCP server entry:

```bash
claude mcp add --transport http mc-gui-crafter http://127.0.0.1:47381/mcp
claude mcp list
```

For a project-scoped `.mcp.json`, use:

```json
{
  "mcpServers": {
    "mc-gui-crafter": {
      "type": "http",
      "url": "http://127.0.0.1:47381/mcp"
    }
  }
}
```

Claude Code also accepts `"type": "streamable-http"` for this transport. Run
`/mcp` inside Claude Code to confirm the server is connected and exposing tools.

### Codex

Add the server with the Codex CLI:

```bash
codex mcp add mc-gui-crafter --url http://127.0.0.1:47381/mcp
codex mcp list
```

Or edit `~/.codex/config.toml` directly:

```toml
[mcp_servers.mc-gui-crafter]
url = "http://127.0.0.1:47381/mcp"
```

Restart the Codex session after changing the MCP configuration so the tools are
loaded for the new conversation.

### opencode

Add a remote MCP server to `opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "mc-gui-crafter": {
      "type": "remote",
      "url": "http://127.0.0.1:47381/mcp",
      "enabled": true
    }
  }
}
```

Restart opencode after editing the config.

### Other HTTP MCP Clients

Use a Streamable HTTP / HTTP MCP server configuration with:

```json
{
  "name": "mc-gui-crafter",
  "type": "http",
  "url": "http://127.0.0.1:47381/mcp"
}
```

Clients that only support stdio MCP servers need an HTTP-to-stdio MCP bridge.
Point the bridge at the MCGUI Crafter URL shown in the app.

Some MCP clients cache tool discovery for the current session. If newly added
tools are missing after upgrading MCGUI Crafter, reconnect the MCP server or
restart the MCP client session.

## Alpha response contract

Closed-alpha MCP tools return compact JSON by default. Binary fields such as
PNG data URLs are opt-in. Reliability Alpha mutating tools `project_resize`,
`group_upsert`, and `element_update_many` include `project_id` in their
responses. No-op mutations should not change project revision or trigger UI
synchronization events.

## Closed-alpha agent workflow

1. Call `schema_discover` before using unfamiliar enums or semantic groups.
2. Create or open a project with `project_new` or `project_open`.
3. Use `slot_grid_add`, `element_add_many`, and `element_update_many` for bulk
   layout work.
4. Use `group_upsert` when group membership changes.
5. Use `project_resize` only for canvas size changes; move elements explicitly.
6. Use `project_semantic_groups_update` with `member_ids` for non-rectangular
   slot groups and control button groups.
7. Use `state_list` and `schema_discover` before editing state variants.
8. Use `project_render` after visual edits and inspect the PNG when possible.
9. Use `project_export_preview` before `project_export`.
10. Save source projects with `project_save_as`.

## Generating GUIs With an Agent

Once the client is connected, ask the agent to use the `mc-gui-crafter` MCP
tools and keep the app open so you can see changes in the editor. A good
starting prompt is:

```text
Use the mc-gui-crafter MCP server to create a Minecraft machine GUI.
First call gui_template_list, then create a 176x166 Forge project from the
closest template. Add and position slots, labels, progress indicators, and
fluid or energy bars using Minecraft pixel coordinates. Use slot_grid_add for
regular slot grids and element_add_many for batches of non-grid elements. After
each major step, call project_summary and element_list to verify the result.
```

For more precise results, include:

- Target loader: `forge`, `fabric`, or `neoforge`.
- GUI size in pixels.
- Slot count and exact layout requirements.
- Labels and text colors. Text color values are integer RGB values, for example
  `4210752` for dark gray.
- Progress direction: `left_to_right`, `right_to_left`, `top_to_bottom`, or
  `bottom_to_top`.
- Whether elements should be on `background`, `overlay`, or `animatable` layer.

Vanilla Minecraft slot coordinates use 18-pixel origins with no extra gap. A
9-column row at `x=8` has slots at `8`, `26`, `44`, and so on. Slot border pixels
create the visual separation, so use `slot_size: 18` and `spacing: 18` unless a
custom texture intentionally breaks vanilla cadence.

Example request:

```text
Use mc-gui-crafter to make a 176x166 NeoForge alloy smelter GUI. It needs two
input slots on the left, one fuel slot below them, a right-facing progress arrow
in the center, one output slot on the right, an energy bar on the far left, and
the title "Alloy Smelter" in dark gray. Use clear element IDs and verify the
final element list.
```

MCP-created projects are live editor sessions. If `project_id` is omitted, tools
target the active tab in the app. New sessions created by `project_new` do not
have a file path yet; save them with `project_save_as`. `project_save` is useful
for projects that were opened from an existing `.mcgui` file or already saved.

### Bulk Slot And Grid Tools

Use `slot_grid_add` for player inventories, hotbars, storage grids, repeated
machine slot blocks, and visible scrollable cells. It creates real `slot`
elements in one history entry and can also create a project group and semantic
group.

Common vanilla coordinates for a 176x166 container:

- Player inventory: `x=8`, `y=84`, `columns=9`, `rows=3`,
  `slot_index_start=9`
- Hotbar: `x=8`, `y=142`, `columns=9`, `rows=1`, `slot_index_start=0`

Player inventory grid:

```json
{
  "name": "slot_grid_add",
  "arguments": {
    "id_prefix": "player_inventory",
    "x": 8,
    "y": 84,
    "columns": 9,
    "rows": 3,
    "slot_role": "player_inventory",
    "inventory_group": "player_inventory",
    "slot_index_start": 9,
    "group_id": "player_inventory",
    "semantic_group_kind": "player_inventory",
    "slot_count": 27
  }
}
```

Hotbar grid:

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
    "group_id": "hotbar",
    "semantic_group_kind": "hotbar",
    "slot_count": 9
  }
}
```

Use `element_add_many` when the elements are not a regular grid, such as adding
a title, progress arrow, energy bar, and button together:

```json
{
  "name": "element_add_many",
  "arguments": {
    "elements": [
      {
        "id": "title",
        "type": "text",
        "x": 8,
        "y": 6,
        "content": "Alloy Smelter",
        "font": "minecraft:default",
        "color": 4210752,
        "layer": "overlay"
      },
      {
        "id": "progress_arrow",
        "type": "progress",
        "x": 79,
        "y": 35,
        "width": 24,
        "height": 17,
        "direction": "left_to_right",
        "asset": "textures/generated/progress_arrow.png",
        "layer": "animatable"
      },
      {
        "id": "start_button",
        "type": "button",
        "x": 116,
        "y": 60,
        "width": 46,
        "height": 20,
        "content": "Start",
        "asset": "textures/generated/button.png",
        "layer": "background"
      }
    ]
  }
}
```

`element_add_many` is atomic: if any element payload is invalid or conflicts with
an existing ID, no elements are added.

### `element_update_many`

Applies multiple `element_update`-style patches atomically in one revision. If
any element is missing or any patch is invalid, no element is changed.

Button and toggle-button icons can use a standalone PNG or an atlas region. Keep
`content` as label, accessibility, and fallback metadata even when the visible
control is icon-only:

```json
{
  "id": "start_button",
  "type": "button",
  "x": 116,
  "y": 60,
  "width": 20,
  "height": 20,
  "content": "Start",
  "tooltip": "Start processing",
  "icon": "textures/gui/icons.png",
  "icon_uv": {
    "x": 0,
    "y": 0,
    "width": 16,
    "height": 16
  },
  "asset": "textures/generated/button.png"
}
```

For standalone icon PNGs, set `icon` to that PNG and omit `icon_uv`.

### Visual Authoring Alpha

Use `asset_metadata_update` to attach reusable authoring metadata to assets that
already exist in the project. Asset-level `nine_slice` guides are the preferred
way to reuse a panel atlas across multiple texture elements:

```json
{
  "name": "asset_metadata_update",
  "arguments": {
    "name": "textures/gui/panel_atlas.png",
    "metadata": {
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
}
```

Texture elements support `render_mode`. Leave the default mode for ordinary
textures. Set `render_mode: "nine_slice"` only when nine-slice guides are
defined either on the element or on its referenced asset. Element-level guides
take precedence; asset-level guides are the fallback for elements that share the
same atlas. Update existing elements in one atomic call with
`element_update_many`:

```json
{
  "name": "element_update_many",
  "arguments": {
    "updates": [
      {
        "id": "background_panel",
        "changes": {
          "asset": "textures/gui/panel_atlas.png",
          "render_mode": "nine_slice",
          "width": 176,
          "height": 166
        }
      }
    ]
  }
}
```

Button and toggle-button elements can render atlas-backed icons with `icon` plus
`icon_uv`. Keep `content` as label, accessibility, and fallback metadata; for
standalone icon PNGs, set `icon` and omit `icon_uv`.

After changing guides or render modes, verify the result with `project_render`
and inspect the PNG before export:

```json
{
  "name": "project_render",
  "arguments": {
    "output_path": "/tmp/mcgui-nine-slice-check.png",
    "include_data_url": false
  }
}
```

MCP responses stay compact by default. Binary image payloads are opt-in: use
`include_data_url: true` on `project_render` or call `asset_get_data_url` only
when the caller explicitly needs inline PNG data.

### Editable State Variants Alpha

Use state variants for alternate editor layouts such as collapsed and expanded
attached panels. State tooling is authoring metadata in this alpha; generated
runtime toggling and codegen behavior remain deferred.

Call `schema_discover` before state editing. It lists `state_variants`, accepted
override fields, and tools that accept `state_id`.

State tools:

- `state_list`: read-only list of states plus session `active_state_id` and
  `edit_scope`.
- `state_add`: create a state with `id`, `label`, optional `description`,
  `initial`, and `export_role`.
- `state_update`: update state metadata. Use `description: null` or
  `export_role: null` to clear those optional fields.
- `state_remove`: remove a state, its overrides, and `state_owned` references.
- `state_set_active`: session-only selector for the editor/MCP session; it does
  not change project data or undo history.
- `state_override_update`: write alpha overrides for `element`,
  `attached_region`, or `group` targets.
- `state_override_clear`: clear one override field or the whole target override.

Element state overrides allow only `visible`, `x`, `y`, `width`, `height`,
`attached_region`, and `layer`. Attached-region overrides allow only `visible`,
`x`, `y`, `width`, and `height`. Group overrides currently allow `visible`.
Existing `element_update` and `element_update_many` default to base-project
edits. They write state overrides only when `state_id` is provided or
`edit_scope` is `"state"`; base-only fields such as `content`, `asset`,
`slot_role`, or semantic metadata are rejected in state scope.

Example:

```json
{
  "name": "state_add",
  "arguments": {
    "id": "expanded",
    "label": "Expanded",
    "initial": true,
    "export_role": "expanded"
  }
}
```

```json
{
  "name": "state_override_update",
  "arguments": {
    "state_id": "expanded",
    "target_type": "element",
    "target_id": "returns_slot_0",
    "fields": {
      "x": 214,
      "visible": true,
      "attached_region": "returns_pocket"
    }
  }
}
```

Render an effective state without mutating the base project:

```json
{
  "name": "project_render",
  "arguments": {
    "state_id": "expanded",
    "output_path": "/tmp/mcgui-expanded.png"
  }
}
```

`project_render` is preferred for visual verification. `project_screenshot` is a
deprecated alias and accepts the same `state_id`.

### Attached Regions

Use attached regions when a GUI has visible or interactive elements outside the
main `gui_size` rectangle: side toggles, upgrade panels, return pockets, or
decorative flair. Coordinates remain absolute relative to the main GUI origin.

Create the region:

```json
{
  "name": "attached_region_add",
  "arguments": {
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
}
```

Add elements using normal absolute coordinates and set `attached_region`:

```json
{
  "name": "element_add",
  "arguments": {
    "id": "returns_0",
    "type": "slot",
    "x": 184,
    "y": 26,
    "size": 18,
    "slot_role": "machine",
    "inventory_group": "food_returns",
    "attached_region": "returns_pocket"
  }
}
```

Add semantic groups separately. The region describes geometry; semantic groups
describe runtime meaning. Use `attached_region_move_with_elements` when
repositioning a region after adding children.

`state: "toggleable"` is preserved as metadata, but generated runtime open/close
behavior is deferred to the toggleable attached-region roadmap item. Use
`static` for fully supported exports today.

### Simple And Modular Codegen

Projects default to simple code generation. Simple mode keeps the generated
screen compact while still exporting slot and semantic metadata in layout JSON.
Use modular mode for complex storage screens, scrollable inventories, tabbed
panels, or Sophisticated/Toms-style GUIs where generated code should have a
semantic registry.

Set the project default with `project_export_settings_update`:

```json
{
  "codegen_mode": "modular",
  "generate_runtime_helpers": true,
  "generate_semantic_registry": true
}
```

For a one-off export, pass the same fields to `project_export_preview` or
`project_export`. These arguments override the project setting for that export
run only:

```json
{
  "target": "neoforge",
  "mod_id": "examplemod",
  "package": "net.example.gui",
  "class_name": "ScrollableMachineScreen",
  "output_dir": "/tmp/mcgui-export",
  "codegen_mode": "simple",
  "generate_runtime_helpers": true,
  "generate_semantic_registry": false
}
```

`codegen_mode` accepts `simple` or `modular`. In simple mode,
`generate_semantic_registry` is normalized to `false`; in modular mode it is
normalized to `true`.

### Semantic Inventory Metadata

Use `project_semantic_groups_update` to replace the project semantic group list.
Elements can then reference those groups with fields such as `slot_role`,
`slot_index`, `inventory_group`, `scroll_binding`, and scrollbar fields like
`target_group`, `columns`, `visible_rows`, and `total_rows`.

Semantic groups may include `member_ids` for explicit membership. Use it for
non-rectangular fixed slot groups and control button groups. Export preview
warns when explicit members are missing or have the wrong element type.

Accepted slot roles:

- `machine`
- `player_inventory`
- `hotbar`
- `scrollable_inventory`
- `virtual_storage`
- `upgrade`
- `upgrade_settings`
- `filter`
- `ghost`
- `offhand`

Accepted semantic group kinds:

- `fixed_slots`
- `virtual_slot_grid`
- `player_inventory`
- `hotbar`
- `upgrade_slots`
- `upgrade_panel`
- `search_field`
- `control_buttons`

Example semantic group for a 5x3 visible scrollable inventory backed by 30
logical slots:

```json
{
  "semantic_groups": [
    {
      "id": "machine_buffer",
      "kind": "virtual_slot_grid",
      "columns": 5,
      "visible_rows": 3,
      "total_rows": 6,
      "slot_count": 30,
      "scroll_binding": "buffer_scroll"
    }
  ]
}
```

Example visible cell and scrollbar elements:

```json
{
  "id": "buffer_slot_0",
  "type": "virtual_slot_cell",
  "x": 34,
  "y": 58,
  "size": 18,
  "layer": "background",
  "slot_role": "scrollable_inventory",
  "slot_index": 0,
  "inventory_group": "machine_buffer",
  "scroll_binding": "buffer_scroll"
}
```

```json
{
  "id": "buffer_scroll",
  "type": "scrollbar",
  "x": 130,
  "y": 58,
  "width": 12,
  "height": 54,
  "layer": "background",
  "target_group": "machine_buffer",
  "columns": 5,
  "visible_rows": 3,
  "total_rows": 6
}
```

The exported layout JSON includes `semantic_groups` and `export_settings`, and
modular exports can emit `GuiSemanticRegistry.java` for runtime lookup.

`project_export_preview` returns planned files plus warnings before anything is
written. In modular mode, warnings include semantic slot-count mismatches such
as a `player_inventory` group declaring 27 slots while fewer or more matching
slot elements exist. Scrollable `virtual_slot_grid` groups can also warn when a
required `scroll_binding` is missing or when no matching scrollbar element is
found. Fix these by aligning `slot_count`, `slot_role`, `inventory_group`, and
scrollbar `target_group`/`scroll_binding` values, then preview again.

### Template Defaults

Machine-style default templates include a vanilla `player_inventory` 9x3 grid at
`(8,84)` and a `hotbar` 9x1 grid at `(8,142)`, with project groups and semantic
groups using the same IDs. The blank `empty` template remains empty so agents can
create custom-size projects without inherited slots.

The app provides generated assets such as `textures/generated/gui_panel.png`,
`textures/generated/progress_arrow.png`, and `textures/generated/button.png`.
Button and toggle-button elements using the generated button texture are visible
in the editor and baked into the exported GUI texture. Their `content` labels are
rendered by the generated Java screen classes.

### Visual Verification

Use `project_render` after meaningful visual edits. It writes a PNG and returns
compact metadata: `project_id`, optional `state_id`, `path`, `width`, `height`,
`bytes`, and `sha256`. Pass `state_id` to render the effective state layout
without mutating the base project. Set `include_data_url: true` only when the
caller explicitly needs the PNG payload. `project_screenshot` remains available
as a deprecated alias with the same arguments.

```json
{
  "name": "project_render",
  "arguments": {
    "output_path": "/tmp/mcgui-preview.png",
    "include_data_url": false
  }
}
```

### Asset Payloads

`asset_import` and `asset_list` return compact asset metadata instead of large
base64 payloads:

```json
{
  "name": "textures/generated/button.png",
  "width": 16,
  "height": 16,
  "bytes": 128,
  "sha256": "..."
}
```

Use `asset_get_data_url` when a client explicitly needs the full PNG data URL:

```json
{
  "name": "asset_get_data_url",
  "arguments": {
    "name": "textures/generated/button.png"
  }
}
```

### Save And Export

`project_save_as` is available for MCP-created projects without an existing file
path:

```json
{
  "name": "project_save_as",
  "arguments": {
    "path": "/tmp/alloy-smelter.mcgui"
  }
}
```

Always call `project_export_preview` before `project_export`. Export settings can
be stored on the project with `project_export_settings_update` or passed as
one-off override fields to preview/export. `class_name` is sanitized for Java;
the generated screen class appends `Screen` only when the sanitized name does not
already end with `Screen`.

Pass `state_id` to `project_export_preview` or `project_export` to preview or
write generated assets from an effective editable state layout. State metadata is
preserved in the layout JSON; runtime state toggling remains deferred.

During repeated exports to the same generated directory, pass `overwrite: true`
to `project_export_preview` and `project_export` to suppress existing-file
warnings. This does not suppress semantic, progress, or control validation
warnings.

## Protocol Notes

- `POST /mcp` accepts JSON-RPC 2.0 requests with `Content-Type: application/json`.
- `Accept` must allow `application/json`.
- Browser `Origin` headers must be localhost (`localhost`, `127.0.0.1`, or `::1`).
- `GET /mcp` returns `405` because SSE streaming is not implemented.
- The server responds to `initialize`, `tools/list`, and `tools/call`.
- The server does not emit an unsolicited `notifications/initialized`; clients may send that notification after `initialize`.

## Project Targeting

Tools that read or mutate project data accept optional `project_id`. When
`project_id` is omitted, the active app tab/session is targeted. This means MCP
edits are visible in the current GUI instance and use the same undo/redo stacks
as UI-driven edits.

### `schema_discover`

Call this before authoring unfamiliar projects. It returns accepted enum values,
editable element fields, state variant fields, state override allowlists, export
settings, attached-region values, layer values, progress direction values, and
notes about default fields that may be omitted from serialized project JSON.

### `project_resize`

Changes `gui_size` only. It does not move, scale, clamp, or delete elements,
including elements outside the new bounds. Agents should move affected elements
explicitly after resizing.

## Available Tools

### Discovery

| Tool | Description |
|------|-------------|
| `schema_discover` | Return accepted enum values, editable fields, export settings, and serialization defaults |

### Project Sessions

| Tool | Description |
|------|-------------|
| `project_new` | Create a new GUI project session |
| `project_open` | Open an existing `.mcgui` file as a session |
| `project_save` | Save a project session |
| `project_save_as` | Save a new or existing project session to a `.mcgui` path |
| `project_resize` | Resize the project GUI canvas without moving elements |
| `project_export_preview` | Preview generated export files, warnings, and validation errors |
| `project_export` | Write generated mod files to disk |
| `project_render` | Render a compact PNG preview of the project |
| `project_screenshot` | Deprecated alias for `project_render` |
| `project_summary` | Get project metadata and session summary |
| `project_export_settings_update` | Update default simple/modular codegen settings |
| `project_semantic_groups_update` | Replace project semantic group definitions |
| `project_list_sessions` | List open project sessions |
| `project_get_active` | Get the active session and project |
| `project_undo` | Undo the last backend mutation |
| `project_redo` | Redo the last undone backend mutation |

### States

| Tool | Description |
|------|-------------|
| `state_list` | List editable state variants and active session state |
| `state_add` | Add an editable state variant |
| `state_update` | Update editable state metadata |
| `state_remove` | Remove a state and its overrides |
| `state_set_active` | Set session active state/edit scope without project mutation |
| `state_override_update` | Update element, attached-region, or group state overrides |
| `state_override_clear` | Clear a state override target or field |

### Elements

| Tool | Description |
|------|-------------|
| `element_add` | Add an element |
| `element_add_many` | Add multiple elements atomically |
| `slot_grid_add` | Create a grouped grid of slot elements with semantic metadata |
| `element_move` | Move an element |
| `element_update` | Update element fields |
| `element_update_many` | Update multiple elements atomically |
| `element_resize` | Resize an element |
| `element_reorder` | Move an element to a z-order index |
| `element_remove` | Remove an element |
| `element_list` | List elements |

### Groups

| Tool | Description |
|------|-------------|
| `group_create` | Group two or more elements by `element_ids`; optional `group_id` |
| `group_upsert` | Create or replace a group membership |
| `group_ungroup` | Remove a group while keeping its elements |
| `group_list` | List groups |

### `group_upsert`

Use `group_upsert` when editing existing groups. It creates a group if missing
or replaces membership if present, preserving a single history entry and
avoiding the `group_ungroup` plus `group_create` workaround.

### Animations

| Tool | Description |
|------|-------------|
| `animation_create` | Create an animation |
| `animation_update` | Update animation fields |
| `animation_remove` | Remove an animation |
| `animation_bind` | Bind animation to an element |
| `animation_unbind` | Unbind an element animation |
| `animation_list` | List animations |

### Assets

| Tool | Description |
|------|-------------|
| `asset_import` | Import a PNG from disk |
| `asset_update` | Replace an existing asset from a PNG data URL |
| `asset_metadata_update` | Update metadata such as dimensions and nine-slice guides |
| `asset_remove` | Remove an asset |
| `asset_get_data_url` | Read an asset as a data URL |
| `asset_list` | List imported assets |

### Templates

| Tool | Description |
|------|-------------|
| `gui_template_list` | List available templates |

## Example JSON-RPC Call

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "element_add",
    "arguments": {
      "id": "input_slot",
      "type": "slot",
      "x": 44,
      "y": 17,
      "size": 18
    }
  }
}
```

All coordinates use Minecraft convention: top-left origin, Y increases
downward, pixel units.
