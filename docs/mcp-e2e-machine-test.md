# MCP E2E Machine GUI Test

Date: 2026-05-23

This documents an end-to-end MCP workflow against a running `pnpm tauri dev`
MCGUI Crafter instance. The test creates a Forge GUI for a machine with four
input slots, one progress animation, two output slots, and a title overlay.

## Workflow

1. `gui_template_list`
   - Confirmed the `empty` 176x166 template is available.
2. `project_new`
   - Created `Four Input Processor MCP Test`, 176x166, Forge, from `empty`.
   - New empty projects include generated base assets:
     `textures/generated/gui_panel.png`, `slot.png`, `progress_arrow.png`,
     `fluid_tank.png`, and `energy_bar.png`.
3. `element_add`
   - Added a full-size background texture using `textures/generated/gui_panel.png`.
   - Added four input slots in a 2x2 square at `(44,35)`, `(62,35)`,
     `(44,53)`, `(62,53)`.
   - Added two output slots at `(128,35)` and `(128,53)`.
   - Added a progress element at `(91,47)`, `24x17`, on the `animatable`
     layer, using `textures/generated/progress_arrow.png`.
   - Added `Four Input Processor` as a text element on the `overlay` layer so it
     is not baked into the background texture.
4. `animation_create` and `animation_bind`
   - Created a `fill` animation named `processing_progress`.
   - Bound it to the progress arrow.
5. `project_save_as`
   - Saved the project to `/tmp/four-input-processor-mcp-test-fixed.mcgui`.
6. `project_export`
   - Exported Forge files to `/tmp/four-input-processor-export-fixed`.

## Generated Output

The export generated Java, Gradle metadata, layout JSON, and textures:

- `src/main/java/net/inkyquill/mcpprocessor/FourInputProcessorScreen.java`
- `src/main/java/net/inkyquill/mcpprocessor/FourInputProcessorClient.java`
- `src/main/java/net/inkyquill/mcpprocessor/GuiLayout.java`
- `src/main/resources/assets/mcp_processor/gui/fourinputprocessor_layout.json`
- `src/main/resources/assets/mcp_processor/textures/gui/fourinputprocessor_gui.png`
- `src/main/resources/assets/mcp_processor/textures/gui/fourinputprocessor_overlay.png`
- `src/main/resources/assets/mcp_processor/textures/gui/progress_arrow.png`
- `src/main/resources/assets/mcp_processor/textures/generated/gui_panel.png`
- `src/main/resources/assets/mcp_processor/textures/generated/progress_arrow.png`

The layout JSON preserves the requested element coordinates and includes the
`processing_progress` animation. The title remains a text element on the
`overlay` layer instead of being baked into the background atlas.

## Findings

- Initial blocker: MCP exposed `project_save` but not Save As. New MCP-created
  projects have no path, so `project_save` failed with `No project path set. Use
  Save As first.`
- Initial blocker: MCP did not expose export preview or export tools, even
  though the Tauri commands already existed.
- Quality issue: animatable progress sprites were exported as a one-color
  placeholder instead of using the assigned progress arrow texture.
- Quality issue: slot elements were present in the layout JSON, but not baked
  into the generated GUI texture. The generated Java drew placeholder slot
  rectangles at runtime instead, which is not how Minecraft GUI textures are
  normally authored and makes resource-pack overrides incomplete.
- Current limitation: a text-only overlay still causes an overlay atlas file to
  be generated. That file is harmless, but in this workflow it is effectively
  empty because text is rendered by runtime code rather than rasterized into the
  overlay atlas.
- Current limitation: MCP tool metadata loaded by Codex did not refresh in the
  already-running Codex session after the app exposed new tools. Direct MCP
  JSON-RPC calls to `tools/list` and `tools/call` did see and use the new tools.
  New client sessions should load the expanded tool list normally.

## Fixes Applied

- Added MCP tools:
  - `project_save_as`
  - `project_export_preview`
  - `project_export`
- Added MCP tests for tool discovery, Save As, and export preview.
- Updated animatable sprite export to use the element asset first, then the
  bound animation texture, falling back to the placeholder only when no source
  texture is available.
- Added export coverage that checks generated animatable sprite pixels come from
  the source texture.
- Updated atlas compositing so slot elements are baked into the background or
  overlay texture using their slot texture. Generated runtime code now skips
  slots instead of drawing slot placeholders after the atlas is rendered.
- Added export coverage that checks the generated background PNG contains slot
  texture pixels and that generated runtime code no longer contains
  `renderSlot(...)`.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml mcp --locked`
- `cargo test --manifest-path src-tauri/Cargo.toml animatable --locked`
- `cargo test --manifest-path src-tauri/Cargo.toml slot --locked`
- Live MCP save/export completed successfully against `http://127.0.0.1:47381/mcp`.
- The fixed exported progress sprite reports `2 colors 24x17`; the old
  placeholder export reported `1 colors 24x17`.
