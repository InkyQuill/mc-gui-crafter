# MCP Workflows

Use these examples as patterns. Tool names refer to the `mc-gui-crafter` MCP
server exposed by the running app. If a tool documented here is missing, reconnect
or restart the MCP client; many clients cache `tools/list` for the session.

## Closed-alpha Compact Workflow

Use this sequence for a small fixed-slot machine GUI. Keep the returned
`project_id` from `project_new` and pass it to later calls when multiple sessions
are open.

```json
{
  "name": "schema_discover",
  "arguments": {}
}
```

```json
{
  "name": "project_new",
  "arguments": {
    "name": "Alpha Machine",
    "mod_target": "neoforge",
    "template": "empty",
    "width": 176,
    "height": 166
  }
}
```

```json
{
  "name": "slot_grid_add",
  "arguments": {
    "id_prefix": "input",
    "x": 44,
    "y": 35,
    "columns": 2,
    "rows": 2,
    "slot_role": "machine",
    "inventory_group": "machine_inputs",
    "slot_index_start": 0,
    "group_id": "machine_inputs",
    "semantic_group_kind": "fixed_slots",
    "slot_count": 4
  }
}
```

```json
{
  "name": "group_upsert",
  "arguments": {
    "group_id": "machine_inputs",
    "element_ids": ["input_0", "input_1", "input_2", "input_3"]
  }
}
```

```json
{
  "name": "project_render",
  "arguments": {
    "output_path": "/tmp/alpha-machine-preview.png",
    "include_data_url": false
  }
}
```

```json
{
  "name": "project_export_preview",
  "arguments": {
    "target": "neoforge",
    "mod_id": "examplemod",
    "package": "net.example.gui",
    "class_name": "AlphaMachineScreen",
    "output_dir": "/tmp/alpha-machine-export",
    "overwrite": true
  }
}
```

```json
{
  "name": "project_save_as",
  "arguments": {
    "path": "/tmp/alpha-machine.mcgui"
  }
}
```

## Practical Machine GUI Workflow

1. List templates.

```json
{
  "name": "gui_template_list",
  "arguments": {}
}
```

2. Create the project. Most fixed machine screens fit 176x166. Machine-style
templates already include `player_inventory` 9x3 at `(8,84)` and `hotbar` 9x1 at
`(8,142)` with semantic groups; use `empty` when you want to build every slot
grid yourself.

```json
{
  "name": "project_new",
  "arguments": {
    "name": "Alloy Smelter",
    "mod_target": "neoforge",
    "template": "advanced_machine",
    "width": 176,
    "height": 166
  }
}
```

3. Add repeated machine slots with `slot_grid_add`. Vanilla slot origins are 18
pixels apart with no extra gap; the slot border creates the visible separation.

```json
{
  "name": "slot_grid_add",
  "arguments": {
    "id_prefix": "input",
    "x": 44,
    "y": 35,
    "columns": 2,
    "rows": 2,
    "slot_role": "machine",
    "inventory_group": "machine_inputs",
    "slot_index_start": 0,
    "group_id": "machine_inputs",
    "semantic_group_kind": "fixed_slots",
    "slot_count": 4
  }
}
```

4. Add non-grid elements in one batch with `element_add_many`.

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
        "shadow": true,
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
        "animation": "processing_progress",
        "layer": "animatable"
      },
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
        "asset": "textures/generated/button.png",
        "layer": "background"
      }
    ]
  }
}
```

Generated button textures are visible in the editor and baked into exported
textures. Use `icon` plus `icon_uv` for atlas-backed icons, or `icon` alone for a
standalone PNG. Keep `content` even for icon buttons as label, accessibility, and
fallback metadata.

Use `group_upsert` when revising group membership after elements already exist.
It creates missing groups or replaces existing membership in one history entry;
avoid ungrouping and recreating groups just to edit their members.

5. Create and bind the progress animation.

```json
{
  "name": "animation_create",
  "arguments": {
    "id": "processing_progress",
    "type": "fill",
    "data_key": "processing_progress",
    "duration": 200
  }
}
```

```json
{
  "name": "animation_bind",
  "arguments": {
    "element_id": "progress_arrow",
    "animation_id": "processing_progress"
  }
}
```

6. Verify the live state.

```json
{
  "name": "element_list",
  "arguments": {}
}
```

```json
{
  "name": "animation_list",
  "arguments": {}
}
```

When visual inspection is available, capture a screenshot after major layout
changes:

```json
{
  "name": "project_render",
  "arguments": {
    "output_path": "/tmp/alloy-smelter-preview.png",
    "include_data_url": false
  }
}
```

Use project-specific output paths when saving visual artifacts:

```json
{
  "name": "project_render",
  "arguments": {
    "project_id": "11111111-2222-3333-4444-555555555555",
    "output_path": "docs/mcgui/screenshots/example.png",
    "include_data_url": false
  }
}
```

`project_render` returns compact metadata by default: `project_id`, `path`,
`width`, `height`, `bytes`, and `sha256`. Set `include_data_url` only if the
client cannot open local files. `project_screenshot` remains available as a
deprecated alias.

7. Save the project if the user asked for a `.mcgui` artifact. New MCP-created
projects need `project_save_as` because they do not have a path yet.

```json
{
  "name": "project_save_as",
  "arguments": {
    "path": "/tmp/alloy-smelter.mcgui"
  }
}
```

## Visual Authoring Alpha

Use asset metadata for reusable nine-slice panel atlases, then switch texture
elements to `render_mode: "nine_slice"` only after element or asset guides
exist. Render the project after guide changes and inspect the PNG before export.

```json
{
  "name": "asset_metadata_update",
  "arguments": {
    "name": "textures/gui/panel_atlas.png",
    "metadata": {
      "nine_slice": {
        "left": 4,
        "right": 4,
        "top": 4,
        "bottom": 4,
        "edge_mode": "tile",
        "center_mode": "tile"
      }
    }
  }
}
```

```json
{
  "name": "element_update_many",
  "arguments": {
    "updates": [
      {
        "id": "background",
        "changes": {
          "asset": "textures/gui/panel_atlas.png",
          "render_mode": "nine_slice",
          "width": 176,
          "height": 166
        }
      }
    ]
  }
}
```

```json
{
  "name": "project_render",
  "arguments": {
    "output_path": "/tmp/panel-atlas-nine-slice-check.png",
    "include_data_url": false
  }
}
```

## Player Inventory And Hotbar Grids

If you start from `empty`, add vanilla player grids explicitly:

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

## Scrollable Inventory GUI

Prefer the built-in template for scrollable storage.

```json
{
  "name": "project_new",
  "arguments": {
    "name": "Scrollable Machine",
    "mod_target": "neoforge",
    "template": "scrollable_inventory_machine"
  }
}
```

When building manually, use `slot_grid_add` for visible slot cells, then add a
scrollbar and set the semantic group to the full logical inventory.

```json
{
  "name": "slot_grid_add",
  "arguments": {
    "id_prefix": "buffer_slot",
    "x": 34,
    "y": 58,
    "columns": 5,
    "rows": 3,
    "slot_role": "scrollable_inventory",
    "inventory_group": "machine_buffer",
    "slot_index_start": 0,
    "group_id": "machine_buffer_visible",
    "semantic_group_kind": "virtual_slot_grid",
    "slot_count": 30,
    "scroll_binding": "buffer_scroll"
  }
}
```

```json
{
  "name": "element_add",
  "arguments": {
    "id": "buffer_scroll",
    "type": "scrollbar",
    "x": 130,
    "y": 58,
    "width": 12,
    "height": 54,
    "target_group": "machine_buffer",
    "columns": 5,
    "visible_rows": 3,
    "total_rows": 6,
    "scroll_min": 0,
    "scroll_max": 3,
    "layer": "background"
  }
}
```

`slot_grid_add` creates a semantic group with `total_rows` equal to the visible
rows. For larger logical inventories, replace semantic groups explicitly:

```json
{
  "name": "project_semantic_groups_update",
  "arguments": {
    "semantic_groups": [
      {
        "id": "machine_buffer",
        "kind": "virtual_slot_grid",
        "columns": 5,
        "visible_rows": 3,
        "total_rows": 6,
        "slot_count": 30,
        "data_source": "machine_buffer",
        "scroll_binding": "buffer_scroll"
      },
      {
        "id": "player_inventory",
        "kind": "player_inventory",
        "columns": 9,
        "visible_rows": 3,
        "total_rows": 3,
        "slot_count": 27,
        "data_source": "player_inventory"
      },
      {
        "id": "hotbar",
        "kind": "hotbar",
        "columns": 9,
        "visible_rows": 1,
        "total_rows": 1,
        "slot_count": 9,
        "data_source": "hotbar"
      }
    ]
  }
}
```

## Attached Region Workflow

Use attached regions for GUI parts outside the main `gui_size` bounds: side
module panels, return pockets, upgrade panes, floating toggles, and flair.
Coordinates stay absolute relative to the main GUI origin.

1. Call `attached_region_add` with `id`, `anchor`, `x`, `y`, `width`,
   `height`, `state: "static"`, and optional `kind` / `semantic_group`.
2. Add child elements with normal absolute `x` / `y` coordinates and set
   `attached_region` to the region id.
3. Use semantic groups to describe the meaning of slots/buttons inside the
   region. The region itself only describes geometry and anchoring.
4. Prefer `state: "static"` for generated exports. `toggleable` is preserved as
   metadata until runtime open/closed behavior is implemented.
5. Use `attached_region_move_with_elements` when repositioning the region after
   adding children.

```json
{
  "name": "attached_region_add",
  "arguments": {
    "id": "returns_pocket",
    "anchor": "right",
    "x": 176,
    "y": 18,
    "width": 54,
    "height": 72,
    "state": "static",
    "kind": "returns_pocket",
    "semantic_group": "food_returns"
  }
}
```

```json
{
  "name": "element_add",
  "arguments": {
    "id": "returns_0",
    "type": "slot",
    "x": 184,
    "y": 26,
    "size": 18,
    "slot_role": "machine",
    "inventory_group": "food_returns",
    "attached_region": "returns_pocket"
  }
}
```

## Enums

Slot roles:

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

Semantic group kinds:

- `fixed_slots`
- `virtual_slot_grid`
- `player_inventory`
- `hotbar`
- `upgrade_slots`
- `upgrade_panel`
- `search_field`
- `control_buttons`

## Assets

MCP `asset_import` and `asset_list` return compact metadata:

```json
{
  "name": "textures/generated/button.png",
  "width": 16,
  "height": 16,
  "bytes": 128,
  "sha256": "..."
}
```

Fetch the full base64 payload only for explicit binary inspection:

```json
{
  "name": "asset_get_data_url",
  "arguments": {
    "name": "textures/generated/button.png"
  }
}
```

## Export Preview And Export

Switch to modular mode for semantic registry output.

```json
{
  "name": "project_export_settings_update",
  "arguments": {
    "codegen_mode": "modular",
    "generate_runtime_helpers": true,
    "generate_semantic_registry": true
  }
}
```

Preview before writing files:

```json
{
  "name": "project_export_preview",
  "arguments": {
    "target": "neoforge",
    "mod_id": "examplemod",
    "package": "net.example.gui",
    "class_name": "AlloySmelterScreen",
    "output_dir": "/tmp/mcgui-export",
    "codegen_mode": "modular",
    "generate_runtime_helpers": true,
    "generate_semantic_registry": true,
    "overwrite": true
  }
}
```

`class_name` is sanitized. The generated screen class appends `Screen` only when
the sanitized value does not already end with `Screen`, so `AlloySmelter` exports
`AlloySmelterScreen.java` and `AlloySmelterScreen` does not become
`AlloySmelterScreenScreen.java`.

Fix preview warnings before exporting:

- `declares N slots but ... matching elements were found`: update `slot_count`,
  `slot_role`, or `inventory_group` so the semantic group matches real slots.
- `has no scroll binding`: add `scroll_binding` to scrollable semantic groups
  whose `total_rows` exceed `visible_rows`.
- `declares scroll binding ... but no matching scrollbar`: add or fix a
  scrollbar whose `target_group` and binding metadata target that group.

Then export:

```json
{
  "name": "project_export",
  "arguments": {
    "target": "neoforge",
    "mod_id": "examplemod",
    "package": "net.example.gui",
    "class_name": "AlloySmelterScreen",
    "output_dir": "/tmp/mcgui-export",
    "codegen_mode": "modular",
    "generate_runtime_helpers": true,
    "generate_semantic_registry": true,
    "overwrite": true
  }
}
```

Use `overwrite: true` during iteration when re-exporting to the same generated
directory. It suppresses existing-file warnings only; semantic, progress, and
control validation warnings still need fixes.

After export, check that layout JSON includes `semantic_groups` and
`export_settings`, modular exports include `GuiSemanticRegistry.java`, baked slot
and button pixels appear in the GUI PNG, and animatable progress textures are
separate files.
