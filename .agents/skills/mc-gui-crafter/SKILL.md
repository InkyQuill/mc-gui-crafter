---
name: mc-gui-crafter
description: Use MCGUI Crafter through its MCP server to create, edit, verify, save, and export Minecraft GUI projects. Use when an agent is asked to design Minecraft screens, machine/container GUIs, slot layouts, scrollable inventories, progress animations, generated GUI textures, or simple/modular Minecraft GUI code with the running mc-gui-crafter app.
---

# MCGUI Crafter

Use the running MCGUI Crafter app as the source of truth. The MCP server mutates
the same project sessions visible in the editor, so every operation should be
verified against the live project instead of planned only in text.

For concrete JSON examples and command sequences, read
[references/mcp-workflows.md](references/mcp-workflows.md) when creating or
exporting a project.

## Core Workflow

1. Discover the current surface:
   - Call `gui_template_list`.
   - Prefer an existing template over manually recreating common vanilla layouts.
   - If editing an existing session, call `project_get_active`, `project_summary`,
     `element_list`, `animation_list`, and `asset_list` first.

2. Create or choose the project:
   - Use `project_new` for a new GUI.
   - Use 176x166 for most machine screens unless the user specifies otherwise.
   - Use the requested loader: `forge`, `fabric`, or `neoforge`.
   - Keep the returned `project_id` and pass it explicitly after creating a new
     session.
   - MCP-created sessions have no file path until saved; use `project_save_as`
     when the user asks for a durable `.mcgui` file.

3. Build in Minecraft pixel coordinates:
   - Coordinate origin is top-left; Y increases downward.
   - Vanilla slot cadence is 18 pixels. Adjacent slots normally touch by their
     border cadence; do not add arbitrary gaps between slot cells.
   - Slot elements normally use `size: 18`.
   - Use `slot_grid_add` for player inventory, hotbar, storage grids, and
     repeated machine slot grids.
   - Use `element_add_many` for batches of non-grid elements such as labels,
     progress arrows, bars, and buttons.
   - Keep controls inside the GUI background bounds.

4. Use layers intentionally:
   - `background`: baked into the GUI texture; use for panel textures, slots,
     static gauges, scrollbars, and static decorative elements.
   - `animatable`: exported as separate animated textures; use for progress
     arrows, filling bars, and gauges driven by runtime data.
   - `overlay`: rendered above the texture; use for titles and labels that
     should not be baked into the base GUI texture.

5. Add semantic metadata, not only pixels:
   - Machine slots: `slot_role: "machine"`, stable `slot_index`, and an
     `inventory_group` such as `machine`.
   - Player inventory/hotbar slots: use `player_inventory` and `hotbar` roles.
   - Scrollable visible cells: prefer `slot_grid_add` with
     `slot_role: "scrollable_inventory"`, `inventory_group`,
     `slot_index_start`, and `scroll_binding`; use `virtual_slot_cell` only for
     hand-built modular cells.
   - Scrollbars: use `type: "scrollbar"` with `target_group`, `columns`,
     `visible_rows`, and `total_rows`.
   - For virtual grids, update project semantic groups with
     `project_semantic_groups_update`.
   - Accepted `slot_role` values: `machine`, `player_inventory`, `hotbar`,
     `scrollable_inventory`, `virtual_storage`, `upgrade`, `upgrade_settings`,
     `filter`, `ghost`, `offhand`.
   - Accepted semantic group `kind` values: `fixed_slots`,
     `virtual_slot_grid`, `player_inventory`, `hotbar`, `upgrade_slots`,
     `upgrade_panel`, `search_field`, `control_buttons`.

6. Choose code generation mode:
   - Use `simple` for ordinary fixed machine screens.
   - Use `modular` for scrollable inventories, virtual storage grids, tabbed
     panels, upgrade/filter areas, or Sophisticated/Toms-style complex GUIs.
   - Set project defaults with `project_export_settings_update`, or pass
     one-off overrides to `project_export_preview` and `project_export`.
   - `class_name` is sanitized; exported screen classes append `Screen` only
     when the sanitized class name does not already end with `Screen`.

7. Verify after each major step:
   - Call `element_list` after adding/moving elements.
   - Call `animation_list` after creating and binding animations.
   - Call `project_screenshot` after major layout changes when visual inspection
     is available.
   - Call `project_export_preview` before export.
   - Treat preview warnings as actionable. Fix semantic slot-count mismatches by
     aligning `slot_count`, slot roles, and `inventory_group`; fix scrollbar
     warnings by matching `scroll_binding` and scrollbar `target_group`.
   - After export, inspect generated layout JSON and texture files when possible.

## Design Rules

- Start from a generated GUI panel or template so the texture is visible by
  default.
- Default machine templates include `player_inventory` 9x3 at `(8,84)` and
  `hotbar` 9x1 at `(8,142)`, with semantic groups. The `empty` template stays
  empty for custom layouts.
- Slots should be real slot or virtual slot elements so export bakes slot pixels
  into the generated GUI texture. Do not rely on generated Java drawing slot
  placeholders at runtime.
- Text titles should usually be overlay elements using `font:
  "minecraft:default"` and dark gray RGB integer `4210752`.
- Use clear, stable IDs: `input_0`, `fuel_slot`, `output_0`,
  `progress_arrow`, `machine_buffer_slot_0`, `buffer_scroll`.
- Prefer exact coordinates over approximate prose. If a layout is symmetrical,
  compute positions from slot size and GUI dimensions before adding elements.
- Generated button assets use `textures/generated/button.png`. Button labels are
  visible in the editor and rendered by exported Java from the element `content`.
- For icon buttons, use `icon` plus `icon_uv` for atlas-backed icons, or `icon`
  alone for a standalone PNG. Keep `content` as label/accessibility/fallback
  metadata.
- Do not leave generated output unverified. A successful MCP tool call only
  proves the command ran; it does not prove the GUI is useful.

## Common Layout Patterns

Machine with fixed slots:
- Inputs on the left, output on the right, progress element between them.
- Use `slot_grid_add` for repeated layouts. Four inputs as a square use
  `x: 44`, `y: 35`, `columns: 2`, `rows: 2`, `spacing: 18`.
- Two outputs can be vertical at `(128,35)` and `(128,53)`.

Scrollable inventory:
- Prefer the `scrollable_inventory_machine` template.
- Visible cells are `virtual_slot_cell` elements in a fixed 18-pixel grid.
- The semantic group describes the full logical inventory, not just visible
  cells.
- The scrollbar target group must match the visible cells' `inventory_group`.

Complex modular GUI:
- Use modular codegen.
- Group related semantic areas: storage grid, player inventory, hotbar, upgrade
  slots, filters, tabs, search fields, and control buttons.
- Use semantic groups even when there are no visual group boxes; they are for
  generated code and runtime behavior.

## Failure Handling

- If an MCP tool is not visible in the client but docs/source say it exists,
  reconnect or restart the MCP client session. Some clients cache tool metadata.
- If save fails with no project path, use `project_save_as`.
- If export preview warns about missing textures or existing files, resolve the
  warning before reporting success unless the user explicitly accepts it.
- If export preview warns about semantic slot counts or scrollbar bindings, fix
  the semantic group and element metadata, then run `project_export_preview`
  again.
- If a visual element does not appear in export, inspect whether it is on the
  intended layer and whether the element has an asset or generated texture.
- Use `overwrite: true` while iterating on exports to the same generated
  directory; still fix semantic, progress, and control validation warnings.
- `asset_import` and `asset_list` return compact metadata (`name`, `width`,
  `height`, `bytes`, `sha256`). Call `asset_get_data_url` only for explicit
  binary inspection.
