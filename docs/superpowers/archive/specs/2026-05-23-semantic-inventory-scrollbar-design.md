# Semantic Inventory Slots and Scrollbar Template - Design Spec

## Overview

MCGUI Crafter currently represents all slots as the same visual `slot` element.
That is enough for drawing and atlas export, but not enough for generated code
to understand whether a slot belongs to the machine, the player inventory, the
hotbar, or a scrollable inventory viewport.

This spec adds semantic slot metadata and a scrollable inventory template. The
goal is to keep Minecraft-style GUI texture export intact: every visible slot is
baked into the generated GUI texture, while exported layout metadata tells
runtime code how those visible slots map to container/menu indices.

## Reference Findings

### Toms Storage

Toms Storage terminals are not just fixed container slots. The storage area is a
virtual item grid:

- the visible terminal grid is recreated as `rowCount x 9` display cells;
- `scrollTo(float)` maps sorted item rows into those visible cells;
- search, sort, direction, ghost mode, control mode, and tall mode all affect the
  visible data window;
- tall mode changes the GUI height from the current screen size and adds more
  visible rows;
- terminal cells are rendered manually, while player inventory/hotbar/offhand are
  normal Minecraft `Slot`s;
- the scrollbar thumb is a dynamic control, but the base terminal texture still
  represents the complete visible GUI background.

This means MCGUI Crafter needs a semantic "virtual slot grid" concept in
addition to ordinary slot elements. A visible cell can represent an entry in a
filtered/sorted item list rather than a stable server-side slot index.

### Sophisticated Backpacks / SophisticatedCore

Sophisticated Backpacks gets most of its GUI structure from SophisticatedCore:

- storage screens calculate visible storage rows from available height and total
  storage rows;
- 9-wide and 12-wide storage backgrounds exist, plus wider variants when a
  scrollbar is present;
- storage slots, player inventory, hotbar, upgrade slots, extra slots, and
  upgrade-setting slots have distinct meanings;
- upgrade tabs open as docked side panels and move their own slots into/out of
  view;
- upgrade inventory parts occupy columns next to the storage grid;
- filter/crafting/tank/battery upgrades combine slot grids, buttons, progress
  bars, item buttons, toggles, and dynamic overlays.

This means the data model should not stop at "scrollbar plus slot role." It
needs reusable semantic groups, docked panels, tabs, buttons, text fields, and
virtual grids so complex screens can be described without hardcoding one mod's
implementation.

## Goals

- Make inventory slots visible in templates and exported GUI textures.
- Distinguish machine slots, player inventory slots, hotbar slots, and
  scrollable inventory viewport slots in saved project data and export layout
  JSON.
- Add a template for a machine with more inventory rows than fit in the GUI,
  using a vertical scrollbar and a visible slot viewport.
- Add semantics that can represent terminal-style virtual item grids and
  backpack-style upgrade panels/tabs, even if the first implementation only
  exposes one focused scrollbar template.
- Let users choose whether code generation is simple/monolithic or
  modular/component-based, and expose that choice to MCP so AI clients can set it
  intentionally.
- Keep existing `.mcgui` projects compatible. Old slot elements behave as
  generic machine slots unless updated.
- Avoid implementing a full generated menu/container system in this pass. The
  export should provide clear metadata and compile-ready hooks for users to wire
  into their own menu state.

## Non-Goals

- Do not generate a complete `AbstractContainerMenu` or `ScreenHandler`
  implementation yet.
- Do not implement a fully generic virtualized list editor in this pass.
- Do not generate search/sort/ghost-mode business logic for storage terminals.
- Do not recreate SophisticatedCore's upgrade system. The goal is a compatible
  layout/metadata model, not a dependency-specific clone.
- Do not force modular generated code on simple projects. Simple GUIs should keep
  the current compact output path unless the user opts into modular generation.
- Do not add Bedrock JSON UI export.
- Do not replace individual slot elements with an inventory block-only model.

## Data Model

### Slot Role

Add optional semantic fields to `Element`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SlotRole {
    Machine,
    PlayerInventory,
    Hotbar,
    ScrollableInventory,
    VirtualStorage,
    Upgrade,
    UpgradeSettings,
    Filter,
    Ghost,
    Offhand,
}

pub struct Element {
    // existing fields...
    pub slot_role: Option<SlotRole>,
    pub slot_index: Option<u32>,
    pub inventory_group: Option<String>,
    pub scroll_binding: Option<String>,
}
```

Defaults:

- Missing `slot_role` means `machine` for backwards compatibility.
- `slot_index` is the logical index in the owning inventory group.
- `inventory_group` identifies the logical slot source, for example
  `machine`, `player_inventory`, `player_hotbar`, `machine_buffer`,
  `terminal_items`, or `upgrade_0_filter`.
- `scroll_binding` links visible slots to a scrollbar element when the visible
  slot index changes based on scroll state.

### Semantic Groups

Add an optional project-level `semantic_groups` collection. Groups describe a
logical inventory or widget region once, and individual elements can reference
the group by id:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticGroupKind {
    FixedSlots,
    VirtualSlotGrid,
    PlayerInventory,
    Hotbar,
    UpgradeSlots,
    UpgradePanel,
    SearchField,
    ControlButtons,
}

pub struct SemanticGroup {
    pub id: String,
    pub kind: SemanticGroupKind,
    pub columns: Option<u32>,
    pub visible_rows: Option<u32>,
    pub total_rows: Option<u32>,
    pub slot_count: Option<u32>,
    pub data_source: Option<String>,
    pub scroll_binding: Option<String>,
    pub dynamic_height: Option<bool>,
}
```

Use cases:

- `fixed_slots`: normal machine/container slots with stable slot indices.
- `virtual_slot_grid`: Toms-style visible cells backed by a filtered/sorted data
  source. Visible cell index and logical data index are different concepts.
- `player_inventory` and `hotbar`: vanilla 27 + 9 player slots.
- `upgrade_panel`: Sophisticated-style side tab/panel that can own slots and
  controls.

The first implementation may serialize this as optional metadata and use it for
template/export layout JSON before all editor controls exist.

### Generation Mode

Add project-level export/codegen configuration:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CodegenMode {
    Simple,
    Modular,
}

pub struct ProjectExportSettings {
    pub codegen_mode: CodegenMode,
    pub generate_runtime_helpers: bool,
    pub generate_semantic_registry: bool,
}
```

Defaults:

- `codegen_mode = simple` for existing projects and new small templates.
- `generate_runtime_helpers = true` so scroll/progress hooks remain usable.
- `generate_semantic_registry = false` in simple mode, `true` in modular mode.

Simple mode:

- Generates the current compact screen/layout output.
- Bakes all visible static GUI elements into the atlas.
- Exports semantic metadata in layout JSON, but keeps generated code mostly
  monolithic.
- Best for machine screens, fixed slot layouts, and first-time users.

Modular mode:

- Generates named logical parts from semantic groups: storage grid, player
  inventory, hotbar, scrollbar, search field, side buttons, tab panels, upgrade
  slot groups, and virtual grids.
- Emits a semantic registry or equivalent mapping file so runtime code can look
  up groups by id instead of hardcoded local variables.
- Keeps docked panels/tabs and virtual slot grids as separate generated helpers.
- Best for Sophisticated-style storage screens, Toms-style terminals, and any GUI
  where users expect reusable panels or switchable controls.

The setting belongs to the project, but export dialogs may temporarily override
it for one export run.

### Scrollbar Element

Add `Scrollbar` to `ElementType`.

Scrollbar elements are visible GUI controls and semantic bindings:

```rust
pub struct Element {
    // existing fields...
    pub scroll_min: Option<u32>,
    pub scroll_max: Option<u32>,
    pub visible_rows: Option<u32>,
    pub total_rows: Option<u32>,
    pub columns: Option<u32>,
    pub target_group: Option<String>,
}
```

The first implementation only needs vertical scrollbars. Horizontal support can
be added later without changing the semantic model.

Scrollbar rendering:

- The scrollbar track/thumb is baked into the GUI texture if it is static.
- The movable thumb can be exported as an animatable sprite if the template uses
  scroll progress at runtime.
- The layout JSON records the scrollbar element, target group, visible rows,
  total rows, and columns.

### Widget Elements

Add semantic element types needed by the reference GUIs:

```rust
pub enum ElementType {
    // existing variants...
    Scrollbar,
    Button,
    ToggleButton,
    TextInput,
    Tab,
    Panel,
    VirtualSlotCell,
}
```

The first implementation does not need full interactivity for every widget. It
must be able to place, label, bake, and export them with role metadata:

- `Button` and `ToggleButton` export state names and icon/texture references.
- `TextInput` exports a binding such as `search`.
- `Tab` exports dock side, open/closed dimensions, and target panel/group id.
- `Panel` groups child elements and can be marked as docked left/right of the
  main GUI.
- `VirtualSlotCell` is visually slot-like but exports as a virtual display cell,
  not as a stable Minecraft `Slot`.

## Template Design

Add a new template named `scrollable_inventory_machine`.

Default size: `176x166`.

Layout:

- Full generated GUI panel background.
- Title overlay: `Scrollable Machine`.
- Small machine area near the top with:
  - two machine input slots
  - one output slot
  - one progress arrow
- Scrollable inventory viewport:
  - visible grid: `5 columns x 3 rows`
  - total rows: `6`
  - vertical scrollbar on the right
- No player hotbar row in the first version.

First version:

- Use `5x3` scrollable machine-buffer slots in the main panel.
- Use `total_rows = 6`, `columns = 5`, `visible_rows = 3`.
- Add a vertical scrollbar beside the grid.
- Do not include the player hotbar in this template. A second template can cover
  player inventory plus machine slots once the semantic model is proven.

Reasoning: a 9-column scroll area plus scrollbar is very tight in a 176px GUI.
A 5-column buffer clearly demonstrates scroll semantics and leaves room for the
machine controls.

### Future Templates From References

These should be designed after the first scrollbar implementation lands:

- `storage_terminal_9x5`: inspired by Toms Storage. It should include a 9-column
  virtual storage grid, search field, sort/filter/tall-mode side controls,
  scrollbar, player inventory, hotbar, and optional offhand slot metadata.
- `backpack_storage_9`: inspired by Sophisticated Backpacks. It should include a
  storage slot grid, player inventory/hotbar, upgrade slots on the left, a
  docked settings tab on the right, and optional upgrade inventory-part columns.
- `upgrade_filter_tab`: a reusable docked panel with filter slots, allow/deny
  toggle, match toggles, and tag controls. This covers the common Sophisticated
  filter/voiding/pickup/magnet patterns.

These templates should use our generated textures by default, not imported mod
assets. Users can import mod-inspired or resource-pack textures when they need a
closer match.

## Export Behavior

### Texture Export

Visible slot elements continue to be baked into the atlas:

- `machine`, `player_inventory`, `hotbar`, and `scrollable_inventory` slots all
  use the slot texture visually.
- Only visible viewport slots are baked. Offscreen logical rows are represented
  by metadata, not hidden slot pixels.
- `virtual_storage` cells are baked visually, but exported as virtual cells so
  generated code does not treat them as server-side `Slot`s.

Scrollbar track and a preview thumb are baked into the atlas. The movable thumb
is also exported as an animatable sprite and rendered using the scroll value
when the generated runtime hook is wired.

### Layout JSON

Export slot metadata in each slot element:

```json
{
  "id": "buffer_slot_0_0",
  "type": "slot",
  "slot_role": "scrollable_inventory",
  "inventory_group": "machine_buffer",
  "slot_index": 0,
  "scroll_binding": "buffer_scroll",
  "x": 34,
  "y": 54,
  "size": 18
}
```

Export scrollbar metadata:

```json
{
  "id": "buffer_scroll",
  "type": "scrollbar",
  "target_group": "machine_buffer",
  "columns": 5,
  "visible_rows": 3,
  "total_rows": 6,
  "x": 130,
  "y": 54,
  "width": 12,
  "height": 54
}
```

Generated runtime should keep compile-ready hooks similar to progress
animations:

- For Forge/NeoForge: named stub methods for reading scroll state from the menu.
- For Fabric: equivalent `ScreenHandler` stub methods.

The layout runtime should not attempt to create or move server-side slots. It
should expose metadata clearly so users can wire their own menu/container.

For virtual grids, exported runtime hooks should describe the mapping:

- `scroll_value` from `0.0` to `1.0`;
- `first_visible_row = round(scroll_value * max(total_rows - visible_rows, 0))`;
- `visible_cell_index = local_column + local_row * columns`;
- `logical_index = local_column + (first_visible_row + local_row) * columns`.

For dynamic-height/tall-mode layouts, export min/max rows and row height. The
generated comments should show where a user would recompute `imageHeight`,
`inventoryLabelY`, and slot positions from the current screen height.

### Code Generation Modes

The same project can export through either codegen mode:

Simple output shape:

- one screen class/file per target loader;
- one layout JSON;
- one atlas texture plus animatable sprites;
- helper methods for progress/scroll values only where needed.

Modular output shape:

- one screen class/file that composes generated parts;
- one generated part per semantic group where practical;
- one semantic registry that lists group ids, element ids, roles, bindings, and
  dynamic row/scroll metadata;
- separate helper methods for tabs, virtual grids, search controls, side
  buttons, and upgrade panels;
- the same atlas texture contract as simple mode so resource packs can still
  override the complete visual background.

Modular output must still be usable when a user deletes or rewrites parts of the
generated code. Generated names should be stable and based on element/group ids.

## Editor Behavior

Layer panel:

- Show slot role and group in slot labels where space allows.
- Keep existing slot selection and movement behavior.

Properties panel:

- For slot elements, add controls for:
  - role
  - group
  - slot index
  - scroll binding
- For scrollbar elements, add controls for:
  - target group
  - columns
  - visible rows
  - total rows
  - orientation
- For semantic groups, add controls for:
  - group kind
  - columns
  - visible rows
  - total rows/slot count
  - data source key
  - dynamic height
- For tabs/panels/buttons/text inputs, add enough controls to set role, group,
  binding key, and visible label/icon reference.

Project/export settings:

- Add a code generation mode control:
  - `simple`: compact generated code.
  - `modular`: generated semantic parts and registry.
- Show a short description in the UI, but do not block export if semantic groups
  are sparse. In simple mode, missing semantic fields are acceptable. In modular
  mode, validation should warn when a panel/grid/control lacks a stable group id.

MCP:

- `element_add` and `element_update` should accept the new optional fields.
- `gui_template_list` should expose the new template.
- `project_get` or equivalent project read responses should include export
  settings.
- `project_update` or a dedicated `project_export_settings_update` tool should
  allow MCP clients to set:
  - `codegen_mode`
  - `generate_runtime_helpers`
  - `generate_semantic_registry`
- `project_export` should accept optional per-run overrides for these settings.
- MCP validation should return clear errors for unknown codegen modes and clear
  warnings when modular mode is selected without semantic group ids.

## Compatibility

Existing projects without slot metadata continue to load. During export:

- Missing `slot_role` is treated as `machine`.
- Missing `inventory_group` is omitted from layout JSON.
- Missing `slot_index` is omitted from layout JSON.
- Missing scrollbar fields are omitted from layout JSON.
- Missing `semantic_groups` means the project uses only per-element metadata.
- Missing export settings means `codegen_mode = simple` with runtime helpers
  enabled.
- Unknown future semantic fields are ignored on import if the current app version
  does not understand them, but preserved when possible.

This keeps old `.mcgui` files valid and avoids rewriting saved projects unless
the user edits them.

## Testing

Rust tests:

- Project serialization/deserialization accepts new slot and scrollbar fields.
- Project serialization/deserialization accepts export/codegen settings and
  defaults missing settings to simple mode.
- Old slot JSON without semantic fields still deserializes.
- `scrollable_inventory_machine` appears in `gui_template_list`.
- Template slot positions stay inside bounds.
- Template includes the expected count of visible scrollable slots.
- Exported background atlas contains baked slot pixels for visible scrollable
  slots.
- Exported layout JSON includes `slot_role`, `inventory_group`, `slot_index`,
  and `scroll_binding` for scrollable slots.
- Exported layout JSON includes scrollbar metadata.
- Virtual grid metadata round-trips through project serialization.
- Virtual cells bake slot-like pixels but export as virtual cells, not container
  slots.
- Dynamic row metadata appears in layout JSON without changing static texture
  dimensions unexpectedly.
- Simple export still produces the current compact output for projects without
  semantic groups.
- Modular export emits semantic registry/part metadata for groups, tabs, virtual
  grids, and scrollbars.

Frontend checks:

- Properties panel can edit slot role/group/index/binding.
- Properties panel can edit scrollbar metadata.
- Properties panel can edit the first set of semantic group fields used by the
  new template.
- Project/export settings can switch between simple and modular codegen modes.
- Layers list remains selectable for slot and scrollbar elements.

MCP E2E:

- Create a project from `scrollable_inventory_machine`.
- Verify element count and scrollbar metadata.
- Save and export.
- Inspect layout JSON and atlas PNG.
- Create a terminal-like project with a virtual `9x5` grid and verify that
  export distinguishes virtual cells from player inventory slots.
- Update export settings through MCP, then verify `project_get` and
  `project_export` reflect simple vs modular mode.

## Decisions

- The first scrollbar template uses `5x3` visible slots because it fits the
  standard 176px frame without crowding machine controls.
- The first scrollbar implementation exports a static track and preview thumb in
  the atlas, plus an animatable thumb sprite for runtime scroll state.
- Toms Storage and SophisticatedCore are treated as reference patterns for
  semantics and layout shapes. We will not copy their assets or generate their
  full business logic.
- The implementation should start with the simple scrollbar template, but the
  data model must include the group/widget fields needed for terminal and
  backpack-style GUIs so exported projects do not need a breaking migration
  immediately afterward.
- Code generation mode is configurable per project. Simple mode preserves the
  current compact workflow, while modular mode targets complex Sophisticated- and
  Toms-style GUIs with generated semantic parts.
