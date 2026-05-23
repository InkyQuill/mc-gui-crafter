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

### Screenshots

Use `project_screenshot` after major layout changes when visual inspection is
available. The default response is compact metadata: `path`, `width`, `height`,
`bytes`, and `sha256`. Request `include_data_url` only when the client cannot open
local files.

```json
{
  "name": "project_screenshot",
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

## Available Tools

### Project Sessions

| Tool | Description |
|------|-------------|
| `project_new` | Create a new GUI project session |
| `project_open` | Open an existing `.mcgui` file as a session |
| `project_save` | Save a project session |
| `project_save_as` | Save a new or existing project session to a `.mcgui` path |
| `project_export_preview` | Preview generated export files, warnings, and validation errors |
| `project_export` | Write generated mod files to disk |
| `project_screenshot` | Render a compact PNG preview of the project |
| `project_summary` | Get project metadata and session summary |
| `project_export_settings_update` | Update default simple/modular codegen settings |
| `project_semantic_groups_update` | Replace project semantic group definitions |
| `project_list_sessions` | List open project sessions |
| `project_get_active` | Get the active session and project |
| `project_undo` | Undo the last backend mutation |
| `project_redo` | Redo the last undone backend mutation |

### Elements

| Tool | Description |
|------|-------------|
| `element_add` | Add an element |
| `element_add_many` | Add multiple elements atomically |
| `slot_grid_add` | Create a grouped grid of slot elements with semantic metadata |
| `element_move` | Move an element |
| `element_update` | Update element fields |
| `element_resize` | Resize an element |
| `element_reorder` | Move an element to a z-order index |
| `element_remove` | Remove an element |
| `element_list` | List elements |

### Groups

| Tool | Description |
|------|-------------|
| `group_create` | Group two or more elements by `element_ids`; optional `group_id` |
| `group_ungroup` | Remove a group while keeping its elements |
| `group_list` | List groups |

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
