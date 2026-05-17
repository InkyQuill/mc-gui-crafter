# ADR 001: Technology Stack

**Date:** 2026-05-17  
**Status:** Accepted

## Context

MCGUI Crafter is a desktop GUI editor for Minecraft machine screens. It needs:
- Native file system access (open/save `.mcgui` project files)
- High-performance canvas rendering for texture preview and editing
- A built-in MCP server so AI agents can manipulate projects
- Export pipeline generating mod-loader-specific Screen code and texture atlases
- Cross-platform support (Windows, macOS, Linux)

## Decision

**Tauri 2 (Rust backend) + Svelte 5 (frontend) + Vite**

| Layer | Technology | Role |
|-------|-----------|------|
| Desktop shell | Tauri 2 | Window management, native menus, file dialogs, IPC |
| Backend logic | Rust | Zip I/O, texture compositing, MCP server, export codegen |
| Frontend | Svelte 5 (not SvelteKit) | Reactive UI, canvas rendering via WebGL/PixiJS |
| Build tool | Vite | Fast HMR, bundling |
| MCP transport | localhost Streamable HTTP-style endpoint | AI tool connectivity against the running app instance |
| Rendering | PixiJS (WebGL) or Canvas 2D | Texture preview, pixel-perfect rendering |
| Project format | `.mcgui` (zip archive) | See ADR 002 |

### Why Svelte 5 (not SvelteKit)

Tauri 2 apps don't use SSR. SvelteKit adds complexity without benefit. Svelte 5 runes (`$state`, `$derived`, `$effect`) provide excellent reactivity for drag-and-drop editing and canvas manipulation.

### Why Tauri 2 over Electron

| Concern | Tauri 2 | Electron |
|---------|---------|----------|
| Binary size | ~15 MB | ~150 MB+ |
| Memory | Low | High |
| Backend language | Rust (shared with MCP/export) | Node.js |
| IPC performance | High (native) | Medium (serialization) |
| MCP hosting | Can run in same Rust process | Separate process needed |

### Why Rust as primary backend language

The MCP server, project format parsing, texture compositing, and code generation all benefit from Rust's performance and type safety. Having a single language for the backend avoids context switching and allows zero-copy data sharing between subsystems.

## Alternatives Considered

1. **Electron + TypeScript** — Rejected due to binary size, the overhead of running a separate MCP process, and weaker texture processing capabilities.
2. **Python + Qt/PySide** — Rejected. Qt licensing complexity and weaker web/rendering ecosystem for interactive canvas work.
3. **Pure Rust (egui/iced)** — Rejected. Loss of HTML/CSS flexibility for UI layout and theming. Canvas rendering via wgpu is powerful but slower to iterate on.
4. **Web app only** — Rejected per decision to build a desktop app.

## Consequences

- Development requires Rust toolchain (cargo) and Node.js toolchain (npm/pnpm)
- Tauri 2 is still relatively new; breaking changes in minor versions possible
- Svelte 5 runes are a new paradigm; team familiarity reduces velocity initially
- Strong typing across the stack (Rust types mirror TypeScript types for layout elements)
