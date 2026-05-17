# ADR 003: MCP Server Integration

**Date:** 2026-05-17  
**Status:** Accepted

## Context

MCGUI Crafter exposes its project model as an MCP (Model Context Protocol) server so AI tools (Claude Desktop, Continue, Cline, etc.) can programmatically create and edit GUI projects.

## Decision

**The app starts an MCP server as part of its normal runtime.** The server runs inside the Tauri Rust backend process and shares direct access to the Rust-owned project session manager. No separate GUI process is used.

### Transport

| Transport | Use Case |
|-----------|----------|
| **Streamable HTTP-style JSON-RPC** | Default. AI tools connect to the running app's localhost `/mcp` endpoint. |

### Server Identifier

`mc-gui-crafter`

### Tool Schema

Project-scoped MCP tools accept optional `project_id`. If omitted, they operate on the active app tab/session. If no project is open, tools return a "no project open" error.

#### Project Management

| Tool | Parameters | Returns |
|------|-----------|---------|
| `project_new` | `name: string`, `template?: string`, `width: int`, `height: int`, `mod_target?: "forge"\|"fabric"\|"neoforge"` | `{ path: string }` |
| `project_open` | `path: string` | `{ project: ProjectSummary }` |
| `project_save` | — | `{ path: string }` |
| `project_export` | `format: "forge"\|"fabric"\|"neoforge", path: string` | `{ files: string[] }` |
| `project_summary` | — | `{ name, gui_size, element_count, mod_target }` |

#### Elements

| Tool | Parameters | Returns |
|------|-----------|---------|
| `element_add` | `id: string, type: ElementType, x: int, y: int, ...type_props` | `{ element: Element }` |
| `element_move` | `id: string, x: int, y: int` | `{ element: Element }` |
| `element_resize` | `id: string, width: int, height: int` | `{ element: Element }` |
| `element_remove` | `id: string` | `{ success: bool }` |
| `element_get` | `id: string` | `{ element: Element }` |
| `element_list` | — | `{ elements: Element[] }` |
| `element_set_property` | `id: string, key: string, value: any` | `{ element: Element }` |
| `element_duplicate` | `id: string, new_id: string, offset_x?: int, offset_y?: int` | `{ element: Element }` |

#### Groups

| Tool | Parameters | Returns |
|------|-----------|---------|
| `group_create` | `id: string, x: int, y: int, element_ids: string[]` | `{ group: Group }` |
| `group_add_element` | `group_id: string, element_id: string` | `{ group: Group }` |
| `group_remove_element` | `group_id: string, element_id: string` | `{ group: Group }` |
| `group_move` | `group_id: string, x: int, y: int` | `{ group: Group }` |

#### Animations

| Tool | Parameters | Returns |
|------|-----------|---------|
| `animation_create` | `id: string, type: "fill"\|"cycle"\|"pulse"\|"toggle", data_key: string` | `{ animation: Animation }` |
| `animation_bind` | `element_id: string, animation_id: string` | `{ success: bool }` |
| `animation_unbind` | `element_id: string, animation_id: string` | `{ success: bool }` |
| `animation_list` | — | `{ animations: Animation[] }` |

#### Assets

| Tool | Parameters | Returns |
|------|-----------|---------|
| `asset_import` | `file_path: string, as_name?: string` | `{ asset_name: string }` |
| `asset_list` | — | `{ assets: string[] }` |
| `asset_remove` | `name: string` | `{ success: bool }` |

#### GUI

| Tool | Parameters | Returns |
|------|-----------|---------|
| `gui_resize` | `width: int, height: int` | `{ gui_size: Size }` |
| `gui_template_list` | — | `{ templates: Template[] }` |
| `gui_template_apply` | `template_name: string` | `{ project: ProjectSummary }` |

## AI Workflow Example

A user asks an AI: "Make me a furnace GUI with an input, fuel slot, progress arrow, and output."

```
AI → MCP: project_new("My Furnace", template="furnace", width=176, height=166)
AI → MCP: element_set_property("title", "content", "Iron Furnace")
AI → MCP: element_move("input_slot", 56, 17)
AI → MCP: element_move("fuel_slot", 56, 53)
AI → MCP: asset_import("/path/to/custom_bg.png", as_name="background")
AI → MCP: element_set_property("bg", "asset", "textures/background")
AI → MCP: project_save()
AI → MCP: project_export(format="forge", path="./export/")
```

## Security

- The MCP server only exposes project manipulation tools — no filesystem access beyond the MCP transport's own scope
- Asset import requires the AI/client to provide absolute paths (the tool validates they exist)
- File write operations are scoped to the project `.mcgui` archive
- Export writes to a user-specified directory (directory must exist, no overwrite without confirmation)

## Consequences

- The MCP server is available when the app is running
- The selected localhost port is reported by `mcp_status`
- The MCP protocol's JSON-RPC overhead is acceptable for the expected interaction rate (not real-time, the editor UI handles that)
- The implementation uses direct JSON-RPC handling over a localhost HTTP endpoint
