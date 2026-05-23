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

## Generating GUIs With an Agent

Once the client is connected, ask the agent to use the `mc-gui-crafter` MCP
tools and keep the app open so you can see changes in the editor. A good
starting prompt is:

```text
Use the mc-gui-crafter MCP server to create a Minecraft machine GUI.
First call gui_template_list, then create a 176x166 Forge project from the
closest template. Add and position slots, labels, progress indicators, and
fluid or energy bars using Minecraft pixel coordinates. After each major step,
call project_summary and element_list to verify the result.
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
have a file path yet; save them with the app's Save As workflow. `project_save`
is useful for projects that were opened from an existing `.mcgui` file.

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
