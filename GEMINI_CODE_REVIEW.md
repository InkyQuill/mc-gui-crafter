# Gemini Code Review: mc-gui-crafter

A comprehensive review of the `mc-gui-crafter` project, focusing on architecture, code quality, performance, and security.

## 1. Project Overview
`mc-gui-crafter` is a desktop application for designing and exporting Minecraft machine/container GUIs. It uses a modern tech stack:
- **Frontend**: Svelte 5 (using Runes), PixiJS 8, TypeScript.
- **Backend**: Rust, Tauri v2.
- **Features**: Visual GUI editing, asset management, multi-loader export (Forge, Fabric, NeoForge), MCP integration.

## 2. Architecture & Design

### Frontend (Svelte 5 + PixiJS)
- **State Management**: Excellent use of Svelte 5 Runes (`$state`, `$derived`, `get`). The `ProjectStore` centralizes complex logic effectively.
- **Renderer**: The `GuiRenderer` is a robust PixiJS-based implementation. It handles Minecraft-specific rendering (slots, progress bars) well. The separation of grid, elements, and overlay into containers is clean.
- **Optimistic UI**: Moving and resizing elements are performed locally before committing to the backend, ensuring a snappy user experience.

### Backend (Rust + Tauri)
- **Session Management**: `ProjectSessionManager` handles multiple open projects and undo/redo stacks using full snapshots. While memory-intensive for massive projects, it's appropriate for Minecraft GUIs.
- **Command Structure**: Large but well-structured commands in `commands.rs`. Most commands are scoped to projects and use `Result<T, String>` for error handling.
- **Export Logic**: Highly sophisticated export system in `export/mod.rs` that generates entire Gradle projects and Java code.

### IPC (Integration)
- Standard Tauri `invoke` pattern.
- The `api.ts` file includes a large `mockInvoke` function, suggesting a commitment to testability or a potential web-only target.

---

## 3. Detailed Review

### Frontend: Svelte 5 & TypeScript
- **Pros**:
    - Clean use of `$state` for reactive properties.
    - Effective use of `SvelteMap` for asset and font data.
    - Strong typing across the board.
- **Improvements**:
    - `nextId` in `ProjectStore` is a global variable. It might be better stored within the project state itself to avoid issues when switching projects rapidly or in a multi-window scenario.
    - `textTextureCache` in `GuiRenderer` grows indefinitely. Consider a LRU cache or clearing it periodically.

### Backend: Rust
- **Pros**:
    - Idiomatic Rust usage (mostly).
    - Good use of `thiserror` for error management.
    - Strong validation logic in the export module.
- **Improvements**:
    - **Performance Bottleneck**: `asset_list` command decodes every image using the `image` crate and base64-encodes it *every time* it's called. This will scale poorly with many assets. Recommendation: Cache the base64 strings or return a list of asset names/metadata and fetch data URLs only when needed.
    - **Lock Contention**: `AppState` uses a single `Mutex<ProjectSessionManager>`. For heavy operations like export or texture generation, this could block other commands. Consider more granular locking or using `dashmap` for sessions.
    - **Code Consistency**: `rename_all` in Tauri commands is inconsistent. Some use `snake_case`, others `camelCase`. It's better to stick to one (usually `camelCase` for JS compatibility, but Tauri v2 handles conversion if specified).

### Texture Generation & Compositing
- The project does a lot of image processing in Rust. Using the `image` crate is appropriate, but be mindful of CPU usage during live previews if they trigger compositing.

---

## 4. Performance & Optimization

### 1. Asset Loading
As noted, `api.assetList()` returns all assets with full data URLs. For a project with 50+ textures, this payload can be several megabytes, processed on every project load or sync.
- **Fix**: Return metadata first; fetch actual image data on demand or use Tauri's custom protocol (e.g., `asset://`).

### 2. Snapshots for Undo/Redo
Storing full `Project` clones for every action is simple but grows memory linearly with history depth.
- **Fix**: Consider delta-based undo/redo or limiting the history stack size.

### 3. PixiJS Resource Management
Ensure that `Texture` objects created from data URLs are properly destroyed. The current `GuiRenderer::destroy` handles this, but rapid project switching might lead to leaks if not careful.

---

## 5. Security Considerations

### 1. IPC Exposure
Tauri commands that take file paths (like `project_open`, `font_import`) should be carefully audited. Tauri v2's permission system (capabilities) is used, which is good.

### 2. Path Traversal
The `export` module sanitizes names (mod ID, package), but ensure that `output_dir` provided by the user is validated to prevent writing files outside intended directories.

---

## 6. Recommendations

1.  **Optimize Asset Sync**: Refactor `asset_list` to avoid repeated image decoding and base64 encoding.
2.  **Refactor `api.ts`**: The `mockInvoke` logic is massive. If it's only for testing, consider moving it to a separate mock layer or using a proper MSW (Mock Service Worker) setup.
3.  **Linter Integration**: Add a linter for Rust (clippy) and ensure Svelte 5 snippets are used more extensively for repetitive UI patterns.
4.  **Testing**: While there are some Rust tests (e.g., in `startup.rs`), the frontend could benefit from Playwright or Vitest coverage for the `ProjectStore` logic.
5.  **Modularize `commands.rs`**: It's currently ~3000 lines. Splitting it into `commands/project.rs`, `commands/assets.rs`, etc., would improve maintainability.

## 7. Conclusion
The `mc-gui-crafter` project is built on a solid foundation. The choice of Svelte 5 and Tauri v2 is forward-thinking, and the implementation of the Minecraft GUI logic is very thorough. With some performance optimizations around asset handling and code modularization, it will be a highly professional tool.
