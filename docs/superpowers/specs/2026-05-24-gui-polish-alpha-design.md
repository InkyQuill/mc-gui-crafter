# GUI Polish Alpha Design

## Context

The MCP workflow is now usable, but closed alpha also needs the desktop editor
to feel dependable for manual authoring. Recent UX reviews found that the
editor should better expose buttons, progress textures, layers, assets,
properties, generated backgrounds, and layout persistence.

This epic is the GUI/editor polish slice for alpha. It complements the MCP,
visual authoring, and state variant epics without turning into a full workspace
framework.

## Goals

- Make new GUI projects visibly start with their generated background panel.
- Let users add and edit buttons from the UI, including text, tooltip, and icon
  metadata.
- Let users choose progress textures and atlas UVs from Properties.
- Make Properties and Layers usable together on one screen.
- Reduce Layers panel friction for slot-heavy Minecraft layouts.
- Persist editor/app geometry and provide reliable reset shortcuts.
- Keep the closed-alpha UI focused and predictable.

## Non-Goals

- Do not build a full Blender/Photoshop workspace framework in this epic.
- Do not implement movable/pinnable arbitrary panels.
- Do not implement public release settings, installers, or marketplace flows.
- Do not implement full runtime state toggle codegen.

## New GUI Background Behavior

Creating a new GUI should create exactly one visible generated background
texture element by default:

- `id`: stable generated id such as `background`;
- `type`: `texture`;
- `asset`: generated GUI panel texture;
- `x`: `0`;
- `y`: `0`;
- `width`: project GUI width;
- `height`: project GUI height;
- `layer`: `background`;
- z-order: bottom-most element.

The generated background should appear immediately in the editor, in Layers,
through MCP `element_list`, and in exports. Templates must avoid duplicate
backgrounds.

The `empty` template description should be updated. It is no longer a blank
canvas; it is an empty GUI with a generated background panel.

## Progress Editing

Progress elements must expose their visual source in Properties:

- selected asset;
- optional UV rectangle;
- direction;
- width and height.

The alpha UI should launch the shared UV editor from Properties so the user can
choose an atlas region. The progress element remains responsible for fill/mask
behavior. Decorative frames around progress bars should remain normal texture
elements.

Preview/export should warn when a progress element is sized differently from
its selected source or UV region, because stretching pixel-art progress arrows
is often accidental.

## Button Authoring

The UI must allow adding button/toggle button elements, not only editing them
through MCP.

Properties for a button should expose:

- button text or fallback label;
- tooltip;
- icon asset;
- icon UV rectangle;
- layer;
- semantic/control metadata where available.

Buttons should support standalone PNG icons and atlas-backed icons. Icon UV
selection should use the same shared UV editor as progress and texture regions.

For alpha, icon-only and text-only buttons are the priority. Icon plus text may
be supported if it fits the existing renderer without complicating the model.

## Inspector Dock

Replace the cramped single right sidebar with a persisted inspector dock that
keeps Properties and Layers available at the same time.

The right side contains two adjacent columns:

- Properties column: always visible while the dock is open;
- Browser column: tabbed Layers/Assets browser.

The layout should be horizontally resizable with safe min/max clamps. Saved
values must never allow a panel to disappear or take over the entire editor.

This is inspired by professional editor workflows: select from Layers, edit in
Properties immediately. GIMP is not a UX reference for this work.

## Layers And Assets Density

Layers should become compact enough for slot-heavy GUIs:

- use two-line element rows when needed;
- show id/name on the primary line;
- show type, layer, slot role, dimensions, direction, visibility, lock, and
  state markers on the secondary line as space allows;
- support collapsible project groups and semantic groups;
- keep player inventory, hotbar, and large slot grids collapsible;
- selection from Layers must immediately update Properties.

Assets should live beside Layers as a tab in the Browser column and remain
usable at the new dock width.

## Configuration And Window Persistence

Persist editor layout and window geometry in
`~/.config/mc-gui-crafter/config.json`, alongside existing application config.
The app must create `~/.config/mc-gui-crafter` if it does not exist and preserve
unrelated config fields.

Persist:

- total right dock width;
- Properties column width;
- active Browser tab;
- app window position and size;
- editor window position and size where applicable;
- layout version, starting at `1`.

On startup, invalid or out-of-range values should be clamped or reset to safe
defaults.

Shortcuts:

- `Ctrl+R`: center/reset the current canvas view state;
- `Ctrl+Shift+Alt+R`: reset UI layout, dock sizes, app window geometry, and
  editor window geometry.

The full UI reset should not reset project data, theme, MCP port, recent
projects, or unrelated config.

## Roadmap Updates

When this epic is implemented, the roadmap should mark the alpha GUI polish
items as complete and keep a later workspace framework item open.

Future workspace framework scope:

- movable/pinnable panels;
- workspace profiles;
- richer asset/UV panes;
- optional stacked or pinned Layers/Assets behavior;
- more advanced editor window management.

That larger framework is explicitly after closed alpha.

## Testing

Backend/model tests:

- new project creation produces exactly one background texture element;
- `empty` template metadata reflects the generated panel behavior;
- save/load round-trips progress asset/UV and button icon/tooltip metadata;
- config load/save persists dock widths, active tab, and window geometry;
- invalid layout and window config values clamp to safe defaults;
- full UI reset preserves unrelated config fields.

Frontend/manual checks:

- create a new GUI and confirm the generated background is visible immediately;
- add a button from UI, edit its text, tooltip, and icon UV;
- add/select a progress element and change its texture/UV;
- select elements from Layers and edit them in Properties without switching
  screens;
- collapse a large slot group in Layers;
- resize the right dock, restart, and confirm the layout persists;
- move/resize the app and editor windows, restart, and confirm geometry
  persists;
- use `Ctrl+R` to recenter canvas and `Ctrl+Shift+Alt+R` to reset UI/window
  geometry.
