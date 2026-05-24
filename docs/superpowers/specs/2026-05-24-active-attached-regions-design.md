# Active Attached Regions Design Spec

## Context

MCGUI Crafter needs to support Minecraft screens where the logical container GUI
has a normal centered main area, but visible and interactive pieces extend
outside that area. Examples include decorative corner flair, side toggles,
upgrade panels, Sophisticated Backpacks-style module panes, and food pouch
return pockets for container items such as bowls, bottles, or mod-specific
return items.

The current `gui_size` model works for vanilla-style screens where everything
lives inside the main rectangle. It keeps that meaning. The new model must
allow active outside elements without breaking existing project files, vanilla
container positioning, or simple exports.

## Goals

- Keep `gui_size` as the main logical Minecraft container bounds.
- Allow elements to use negative coordinates or coordinates beyond `gui_size`.
- Support visible and interactive outside regions such as side panels and tabs.
- Keep project element coordinates absolute, not nested under transformed
  parents.
- Let semantic groups describe the runtime meaning of slots/buttons inside an
  outside region.
- Expand editor preview and exported textures only when outside elements exist.
- Preserve existing behavior for projects whose elements stay inside main
  bounds.

## Non-Goals

- Do not implement full runtime open/closed panel behavior in this cycle.
- Do not introduce nested coordinate transforms for child elements.
- Do not make `gui_size` mean the full visual footprint.
- Do not require users or MCP clients to maintain a manual `visual_bounds`
  override.

## Core Model

`gui_size` remains the main bounds:

- vanilla container area;
- slot/menu coordinate base;
- generated `imageWidth` / `imageHeight`;
- screen centering origin;
- main editor frame.

Elements stay in absolute GUI coordinates relative to the main origin. Negative
`x/y` values and values outside `gui_size` are valid. Existing `Element.x` and
`Element.y` already use signed integers, so this is a behavioral/modeling change
rather than a basic type change.

`visual_bounds` is computed, not manually edited in the first version. It is the
bounding box of:

- the main bounds rectangle `(0, 0, gui_size.width, gui_size.height)`;
- visible/exported elements;
- visible/exported attached regions.

The computed bounds may have negative `x/y`. The main bounds must always be
included so an otherwise empty project still exports exactly like today.

## Attached Regions

Add `attached_regions` to the project model. An attached region is a named
wrapper for an outside or edge-attached area. It is not a transform parent.

Fields:

```json
{
  "id": "returns_pocket",
  "anchor": "right",
  "x": 100,
  "y": 18,
  "width": 54,
  "height": 72,
  "state": "static",
  "kind": "returns_pocket",
  "semantic_group": "food_returns"
}
```

Required fields:

- `id`: stable region id.
- `anchor`: `left`, `right`, `top`, `bottom`, or `free`.
- `x`, `y`, `width`, `height`: absolute coordinates and size relative to main
  origin.
- `state`: `static` or `toggleable`.

Optional fields:

- `kind`: descriptive category such as `flair`, `upgrade_panel`,
  `returns_pocket`, or `side_controls`.
- `semantic_group`: semantic group id most closely associated with the region.
- future metadata for labels, open/closed icon states, or animation hooks.

Elements may reference a region:

```json
{
  "id": "returns_0",
  "type": "slot",
  "x": 108,
  "y": 26,
  "slot_role": "storage",
  "inventory_group": "food_returns",
  "attached_region": "returns_pocket"
}
```

The coordinates remain absolute. Moving a region in the editor may move its
child elements as a group, but the saved child coordinates are updated directly.
Export and runtime code do not need nested transform math.

## Semantic Meaning

Attached regions describe geometry and anchoring. Semantic groups describe
Minecraft/runtime meaning.

This separation allows one attached-region model to cover:

- upgrade slots;
- reusable storage pockets;
- food pouch return slots;
- ghost/filter slots;
- side control buttons;
- decorative flair with no semantic group.

Examples:

```json
{
  "id": "food_returns",
  "kind": "fixed_slots",
  "slot_count": 6,
  "purpose": "container_item_returns"
}
```

The first implementation can use existing semantic group kinds such as
`fixed_slots`, `upgrade_slots`, `virtual_slot_grid`, and `control_buttons`.
Additional purpose metadata can remain descriptive until codegen needs stricter
variants.

## Editor Behavior

The editor shows two frames:

- main bounds: the existing `gui_size` rectangle;
- visual bounds: the computed full visible footprint.

Pixi rendering must not clip elements to main bounds. Selection, handles,
hit-testing, layer visibility, and panning must work for negative/outside
coordinates.

Layers show attached regions as collapsible grouping rows when elements
reference them. Selecting a region edits region properties. Dragging a region
can move its child elements together while preserving absolute coordinates.

Properties for an element expose `attached_region` as an optional region
membership field. Properties for a region expose anchor, absolute
position, size, state, kind, and associated semantic group.

## Export Behavior

Background and overlay atlases are built against computed `visual_bounds`, not
only against `gui_size`, when outside elements exist.

The layout JSON includes:

```json
{
  "gui_size": { "width": 100, "height": 200 },
  "visual_bounds": { "x": -16, "y": -16, "width": 170, "height": 216 },
  "textures": {
    "background": "textures/gui/example_gui.png",
    "visual_offset_x": -16,
    "visual_offset_y": -16
  },
  "attached_regions": []
}
```

Runtime contract:

- generated `imageWidth` / `imageHeight` remain `gui_size`;
- `leftPos` / `topPos` remain the main origin;
- background and overlay textures render at
  `leftPos + visual_offset_x`, `topPos + visual_offset_y`;
- elements render and hit-test at `leftPos + element.x`,
  `topPos + element.y`;
- static outside buttons and slots are active if the generated runtime supports
  those controls.

Projects with no outside elements continue to export textures at
`gui_size` with `visual_offset_x = 0` and `visual_offset_y = 0`, or omit offset
fields if omission is already the project convention.

## Toggleable Regions

`state: "toggleable"` is part of the data model, but full runtime support is
deferred.

Current-cycle behavior:

- save/load preserves `toggleable`;
- MCP and UI can set it;
- export/layout JSON preserves it;
- generated simple runtime treats it as static or metadata;
- modular codegen may emit comments or helper placeholders.

Roadmap behavior:

- open/closed state;
- toggle button binding;
- animated panel transitions;
- conditional slot visibility;
- server/client routing for dynamic module panels.

## MCP Behavior

MCP exposes attached region operations:

- `attached_region_add`;
- `attached_region_update`;
- `attached_region_remove`;
- `attached_region_list`;
- `attached_region_move_with_elements`.

`element_add` and `element_add_many` accept `attached_region`.

An agent workflow for a Sophisticated-style or food-pouch UI:

1. Create main GUI using normal `gui_size`.
2. Add a static attached region on the right.
3. Add slots/buttons with absolute coordinates.
4. Set each child element's `attached_region`.
5. Add semantic groups describing slot/button meaning.
6. Preview/export and verify visual bounds include the outside area.

## Migration

Existing project files load with `attached_regions: []`.

Existing elements without `attached_region` behave exactly as before.

Existing exports remain byte-for-byte equivalent where no element or region
extends outside main bounds. If exact layout JSON byte equality is not practical
because of serialization ordering, semantic JSON equality and unchanged PNG
dimensions are the compatibility requirement.

## Testing

Unit tests:

- project save/load round-trip preserves `attached_regions` and element
  `attached_region`;
- computed visual bounds include main bounds, negative elements, and regions;
- computed visual bounds ignore hidden elements and hidden regions;
- background/overlay export expands to visual bounds for outside elements;
- export remains main-size when all elements are inside main bounds;
- layout JSON includes visual bounds and offsets when needed;
- generated runtime keeps `imageWidth` / `imageHeight` equal to `gui_size`;
- hidden outside missing textures do not block export.

Frontend checks:

- editor renders outside elements and handles;
- selection and drag work for negative coordinates;
- dragging a region moves child elements while saving absolute coordinates;
- Layers groups elements by attached region;
- Properties edits region anchor/position/state/kind.

MCP checks:

- create region;
- add elements into region;
- update region;
- move region with elements;
- export preview reports expected visual bounds.

## Roadmap Entry

Add a future roadmap item for full toggleable attached regions:

- runtime open/closed state;
- click bindings;
- panel animation;
- conditional slot activation;
- generated modular helpers for Sophisticated-style modules.
