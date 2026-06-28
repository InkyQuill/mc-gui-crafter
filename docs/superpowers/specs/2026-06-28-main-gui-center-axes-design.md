# Main GUI Center Axes Design

## Goal

Support projects where the visible/exported GUI area is not symmetric around the Minecraft screen center. Users must be able to define the point in the authored GUI that should align to the in-game screen center.

This is needed for two project styles:

- Basic/baked GUIs, where the background and side panels can be baked into one larger texture.
- Modular GUIs, where the main GUI remains stable while transient side panels or attached regions animate in and out.

This design covers only the project-level anchor for the main GUI. Region-level anchors for attached panels are explicitly out of scope for this pass.

## Data Model

Add project-level metadata:

```json
"main_gui_center": { "x": 88, "y": 83 }
```

`x` and `y` are canvas coordinates in the same coordinate space as elements. They represent the point that should land at the Minecraft screen center.

Defaults:

- If `main_gui_center` is missing, use `gui_size.width / 2` and `gui_size.height / 2`.
- New projects initialize the field to that default.
- Loading old projects does not require migration to keep rendering/export working, but saving after changes can persist the explicit field.

The field is independent from visual bounds. Moving side panels or adding off-bounds elements must not automatically move the center axes.

## Editor UX

Show two project-level guide lines:

- A vertical line at `main_gui_center.x`.
- A horizontal line at `main_gui_center.y`.

The guides must be visible over the canvas and easy to distinguish from selection frames, grid lines, and element bounds. They are editor controls, not exported art.

Users must be able to adjust the axes with numeric fields in the project/property panel. Dragging guide lines in the canvas is out of scope for the first implementation and can be added later without changing the saved data model.

Selection behavior:

- Center axes are project settings, not elements.
- They do not appear as layers.
- They do not participate in multi-select, grouping, or resizing.

## Export Semantics

The runtime must place the GUI so that `main_gui_center` aligns to the Minecraft screen center.

For basic/baked export:

- The exported background atlas may expand to the project visual bounds.
- The atlas is drawn with the existing visual offset behavior.
- `leftPos` and `topPos` must be derived from the project `main_gui_center`, not from `gui_size / 2`.
- Off-bounds baked pixels can extend left/right/up/down without recentering the whole visual atlas.

For modular export:

- The same project-level center axes place the base/main GUI.
- Transient side panels and attached regions must not change the main GUI center.
- Future region-level anchors will define their own animation and attachment behavior separately.

## Layout JSON

Exported layout JSON must include the center metadata so generated runtime helpers can use it:

```json
"main_gui_center": { "x": 88, "y": 83 }
```

For compatibility, generated runtime code must default to `WIDTH / 2` and `HEIGHT / 2` if the field is absent.

## Validation And Warnings

Visual size mismatch warnings must remain useful, but they must not treat every off-bounds side panel as an error. The warning copy must make clear that users can either resize/shrink the project or adjust center axes when off-bounds content is intentional.

Add warnings for suspicious center axes:

- The center point is outside the current visual bounds.
- The center point is outside the declared `gui_size` rectangle.

These warnings should not block export.

## Testing

Backend tests:

- Missing `main_gui_center` defaults to `gui_size / 2`.
- The field round-trips through `.mcgui` project serialization.
- Layout JSON includes explicit center axes.
- Generated Forge/Fabric runtime code uses center axes for placement.
- Baked visual bounds export keeps the expanded atlas while positioning it from the custom center axes.

Frontend tests/checks:

- Project state hydrates and saves `main_gui_center`.
- Numeric edits update the project and rerender guides.
- Old projects without the field show default axes.

Manual verification:

- A GUI with a right side panel and an off-center vertical axis exports with the red-line-style axis centered in-game.
- A modular project keeps the base GUI centered while side panels extend outside the base bounds.
