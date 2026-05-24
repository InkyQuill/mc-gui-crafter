# Editor UX Polish Design

## Goal

Polish the GUI editor workflow after the MCP-focused work: new projects should
open with a visible generated background, progress elements should use real
texture sources, the right side of the editor should support fast
select-and-edit workflows, and atlas-backed button/progress/texture regions
should be selectable visually.

## Scope

Implement one editor UX polish cycle:

- create generated GUI backgrounds as real texture elements by default;
- expose progress texture and UV editing in Properties;
- add a reusable UV Editor dialog for texture, progress, and button icon UVs;
- replace the cramped single right sidebar with a persisted inspector dock;
- make Layers compact enough for slot-heavy Minecraft screens through grouped
  rows;
- add layout reset and canvas view reset shortcuts;
- persist and reset the app window size and position;
- document a future workspace/dock framework upgrade in the roadmap.

Do not implement a full movable/pinnable workspace framework in this cycle.
That remains a roadmap item.

## New Project Backgrounds

Creating a new GUI should insert a real background element:

- `id`: stable generated ID such as `background`;
- `type`: `texture`;
- `x`: `0`;
- `y`: `0`;
- `width`: project GUI width;
- `height`: project GUI height;
- `asset`: `textures/generated/gui_panel.png`;
- `layer`: `background`;
- z-order: bottom-most element.

The project should still include the generated default assets. The difference is
that the generated panel asset is now applied automatically as an element, so the
editor canvas, Layers, save/load, MCP, and export paths all see the same visual
state.

Templates should follow the same convention and avoid duplicate generated
backgrounds. If a template already creates a background texture element, it
should keep exactly one. The `empty` template is not a special no-background
mode for this cycle; creating a GUI from the app should give the user a visible
editable generated panel.

## Progress Texture Editing

Progress elements use one source texture:

- `asset`: PNG source, either a standalone progress texture or atlas image;
- `uv`: optional source rectangle inside `asset`;
- `direction`: existing fill direction;
- `width` / `height`: on-GUI rendered size.

Properties must expose progress asset selection and UV editing. Runtime/export
behavior continues to mask/fill the selected source region according to
`direction`. A separate background frame for a progress bar is modeled as a
normal `texture` element placed behind or beside the progress element, not as a
second progress-specific field.

Export preview warnings should compare the element size against the selected
source dimensions. If `uv` is present, compare against the clamped UV region. If
`uv` is absent, compare against the full PNG. A mismatch is allowed but warned as
possible accidental pixel-art stretching.

## Button Icon Editing

Buttons and toggle buttons keep the current metadata model:

- `asset`: button chrome/background texture;
- `content`: label, accessibility/fallback metadata, and runtime text for
  text-only buttons;
- `tooltip`: tooltip metadata;
- `icon`: standalone icon PNG or atlas PNG;
- `icon_uv`: optional rectangle inside `icon`.

Properties should launch the reusable UV Editor for `icon_uv`. If no `icon`
asset is selected, the user should select an icon asset before editing the
region. Icon-only buttons still keep `content` so exports and future runtime
helpers have a stable label.

## Reusable UV Editor

Add a reusable UV Editor dialog opened from Properties. It must support:

- texture element `uv`;
- progress element `uv`;
- button/toggle `icon_uv`.

The dialog should:

- choose from existing project PNG assets;
- preview the selected image at pixel-art-friendly scaling;
- support pan/zoom when the image is larger than the dialog;
- let the user draw and adjust a rectangular region;
- expose numeric `x`, `y`, `width`, and `height` fields for precise editing;
- provide `Apply`, `Clear`, and `Cancel`.

`Apply` writes the selected region back to the relevant element field. `Clear`
removes the UV and uses the full selected asset. The component should be
reusable so Asset Library can later open the same dialog without duplicating UV
selection logic.

## Inspector Dock

Replace the cramped right sidebar with a professional editor-style inspector
dock inspired by Photoshop, Krita, Blender, and similar large editors. Do not use
GIMP as a UX reference.

The right dock contains two adjacent areas:

- **Properties area**
  - always visible when the dock is open;
  - edits the selected element or project;
  - horizontally resizable within safe min/max limits.
- **Browser area**
  - tabbed `Layers` / `Assets`;
  - visible beside Properties so the user can select an element in Layers and
    immediately edit it in Properties;
  - keeps its active tab in layout state.

Persist layout values in `~/.config/mc-gui-crafter/config.json`:

- total right dock width;
- Properties area width;
- active browser tab (`layers` or `assets`);
- app window size and position at close;
- `editor_layout_version`, starting at `1`, so future layout migrations can
  distinguish old saved values from new workspace formats.

On load and during resize, clamp widths so neither panel can disappear or take
over the whole app. Broken or out-of-range config values should fall back to
safe defaults.

Shortcuts:

- `Ctrl+Shift+Alt+R`: reset editor layout/docks and app window geometry;
- `Ctrl+R`: reset and center the canvas view only.

## Layers Panel Density

Layers should use a hybrid compact model suitable for Minecraft GUIs with many
slots:

- project groups and semantic groups can appear as collapsible group rows;
- slot grids such as player inventory and hotbar should be collapsible so they
  do not dominate the list;
- important individual elements can use a two-line row with ID plus useful
  metadata such as type, layer, dimensions, slot role, or direction;
- selected rows must be visibly highlighted;
- clicking a layer selects the element and immediately updates Properties.

The first implementation may use existing project groups and semantic groups as
the source for grouping. It should not rely only on ID naming conventions when
real group metadata is available.

Assets remains a compact browser tab next to Layers. It should continue to show
project assets clearly at the new dock width and provide entry points that are
compatible with the reusable UV Editor later.

## Configuration And Reset

Editor layout persistence belongs in the app config under
`~/.config/mc-gui-crafter`, alongside the existing MCP port. The app should
create the config directory when missing and preserve unrelated config fields.
The same config stores the last app window size and position so the editor
reopens where the user left it.

Reset behavior is intentionally narrow:

- `Ctrl+Shift+Alt+R` resets dock/layout values and app window geometry to safe
  defaults;
- it does not reset theme, MCP port, recent projects, project data, grid
  visibility, or canvas view;
- `Ctrl+R` handles canvas recenter/reset separately.

## Roadmap

Add a checked item for this polish cycle when implemented. Also add a future
roadmap item for a larger workspace/dock framework:

- movable and pinnable editor panels;
- workspace profiles;
- richer asset/UV workspace panes;
- optional stacked/pinned Layers and Assets behavior.

That future upgrade is out of scope for this cycle.

## Testing

Rust/backend tests:

- new project/template creation produces exactly one generated background
  texture element with the expected asset, size, layer, and bottom z-order;
- save/load round-trip preserves the generated background, progress asset/UV,
  and button icon UV metadata;
- config load/save persists editor layout fields and clamps invalid values;
- config load/save persists app window size and position;
- layout reset restores safe dock and window defaults without changing unrelated
  config fields;
- export preview progress stretch warnings use the selected UV region when
  present.

Frontend checks:

- `pnpm check`;
- `pnpm build`;
- create a new GUI and verify the generated background is visible immediately;
- create/select progress and change its asset/UV through Properties;
- create/select button and choose an atlas icon region through the UV Editor;
- resize the right dock, switch Layers/Assets tab, restart or reload state, and
  verify persisted layout;
- move/resize the app window, close/reopen, and verify geometry is restored;
- trigger `Ctrl+Shift+Alt+R` and verify dock and window defaults return;
- trigger `Ctrl+R` and verify the canvas recenters without changing dock layout.

## Open Decisions

No product decisions remain open. The implementation should use the recommended
editor dock plus reusable UV Editor approach and leave the full movable
workspace framework for the roadmap.
