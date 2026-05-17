# MCP Server - mc-gui-crafter

MCGUI Crafter hosts an MCP Streamable HTTP endpoint from the running Tauri app
instance. MCP calls mutate the same Rust-owned project sessions used by the UI,
save/export commands, and backend undo/redo.

## Server Info

| Field | Value |
|-------|-------|
| Name | `mc-gui-crafter` |
| Version | app package version |
| Transport | Streamable HTTP-style JSON-RPC |
| Endpoint | `http://127.0.0.1:{port}/mcp` |
| Capabilities | tools |

The listener binds to localhost with an OS-assigned port when the app starts.
Use the Tauri `mcp_status` command to report the selected MCP address.

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
| `project_summary` | Get project metadata and session summary |
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
