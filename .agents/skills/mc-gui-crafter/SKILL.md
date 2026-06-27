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

Prefer these closed-alpha MCP tools:

- `schema_discover` for accepted enums and editable fields.
- `project_render` for visual verification.
- `project_resize` for canvas size changes only.
- `state_list`, `state_add`, `state_override_update`, and
  `state_override_clear` for editable state variants.
- `slot_grid_add`, `element_add_many`, and `element_update_many` for bulk edits.
- `group_upsert` for creating or replacing group membership.
- `project_semantic_groups_update` with `member_ids` for explicit semantics.
- `session_report` to record agent-discovered bugs, confusing behavior,
  validation gaps, or production feedback into the app's session log.

## Core Workflow

1. Discover the current surface:
   - Call `gui_template_list`.
   - Prefer an existing template over manually recreating common vanilla layouts.
   - When unsure about enum values or editable fields, call `schema_discover`
     instead of guessing.
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
   - Pass `state_id` to `project_export_preview` or `project_export` when you
     need generated assets/layout JSON for an effective editable state.

7. Verify after each major step:
   - Call `element_list` after adding/moving elements.
   - Call `animation_list` after creating and binding animations.
   - Call `project_render` after major layout changes when visual inspection
     is available; `project_screenshot` remains a backward-compatible deprecated
     alias. Pass `state_id` to render an effective editable state.
   - Call `project_export_preview` before export.
   - Treat preview warnings as actionable. Fix semantic slot-count mismatches by
     aligning `slot_count`, slot roles, and `inventory_group`; fix scrollbar
     warnings by matching `scroll_binding` and scrollbar `target_group`.
   - After export, inspect generated layout JSON and texture files when possible.

8. Report product feedback through the session log:
   - When you find confusing behavior, missing validation, bad generated output,
     a likely bug, or user feedback that should reach maintainers, call
     `session_report` with a concise `summary`, `severity`, and useful
     structured `details`.
   - After calling `session_report`, ask the user to attach the latest session
     log from the app logs directory when filing an issue. The log includes the
     report plus recent warnings, errors, and actions needed for reproduction.

## Visual Authoring

- Use asset-level `nine_slice` metadata for reusable panel atlases shared by
  multiple texture elements.
- Set texture elements to `render_mode: "nine_slice"` only when nine-slice
  guides are defined on the element or on its referenced asset.
- Element-level nine-slice guides override asset metadata; asset metadata is
  the reusable fallback for texture elements that reference the same atlas.
- Prefer `project_render` after guide or render-mode changes, then inspect the
  PNG before exporting.
- For atlas-backed icon buttons, use `icon` plus `icon_uv`. Keep `content` as
  label, accessibility, and fallback metadata.

## Editable State Variants

- Use state variants for alternate authoring layouts such as collapsed and
  expanded side panels. Runtime toggling/codegen behavior is deferred in this
  alpha.
- Call `schema_discover` before state editing. It returns `state_variants`,
  override field allowlists, edit scopes, and tools accepting `state_id`.
- `state_list` is read-only. `state_set_active` changes only session selection
  and edit scope; it does not write project data or undo history.
- Create state metadata with `state_add` and adjust it with `state_update`.
  Remove a state with `state_remove`, which also clears that state's overrides.
- Write state-specific layout changes with `state_override_update` or by
  calling `element_update`/`element_update_many` with explicit `state_id` or
  `edit_scope: "state"`.
- Existing element tools default to base edits. In state scope, only `visible`,
  `x`, `y`, `width`, `height`, `attached_region`, and `layer` are valid element
  override fields; base-only fields such as `content`, `asset`, `slot_role`,
  and semantic metadata must be edited on the base project.
- In state scope, set `attached_region: null` to detach an element for that
  state. Use `state_override_clear` when the element should inherit the base
  attached-region membership again.
- Attached-region state overrides allow `visible`, `x`, `y`, `width`, and
  `height`; group overrides currently allow `visible`.
- Prefer `project_render` with `state_id` for state visual verification.
  `project_screenshot` is a deprecated alias and accepts the same `state_id`.

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
- When creating side panels, module pockets, return-slot pockets, or flair
  outside the main GUI rectangle, create an attached region first. Keep child
  element coordinates absolute relative to the main GUI origin and set each
  child's `attached_region`. Use semantic groups to describe slot/button
  meaning; the region only describes geometry and anchoring. Prefer `state:
  "static"` until toggleable runtime support is implemented.
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
- If you diagnose a product issue or limitation while helping a user, call
  `session_report` before ending the session, then ask the user to attach the
  latest session log to their issue.
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
