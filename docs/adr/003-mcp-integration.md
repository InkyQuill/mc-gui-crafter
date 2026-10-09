# ADR 003: MCP server integration

**Status:** Accepted; implementation notes refreshed 2026-10-09.

## Decision

The running Tauri application exposes a localhost Streamable HTTP-style JSON-RPC
endpoint at `/mcp`. It shares the Rust project session manager used by UI
commands, persistence, export and undo/redo. No separate project database or
headless editor process is required.

The preferred port is 47381. The app persists its selected port in its platform
configuration directory and falls back to a free port when necessary. The start
panel and `mcp_status` expose the actual endpoint.

Project tools accept an optional `project_id`; omission targets the active
session. Clients should discover tools and schemas through `tools/list`.
The maintained [MCP reference](../mcp.md) documents setup, capabilities, transport
constraints and examples. This ADR intentionally does not duplicate tool schemas.

## Consequences

UI and MCP changes share project history and emit updates for frontend mirrors.
MCP availability depends on the desktop process. Clients must reconnect or
refresh discovery after an application upgrade; old cached schemas are not the
current server contract.
