# Visual Authoring Alpha Design

## Context

MCGUI Crafter can generate usable Minecraft-style GUI textures, but closed
alpha needs stronger visual authoring primitives. Users should be able to build
background panels from atlas pieces, reuse pixel-art GUI parts, and choose
button/progress regions visually instead of hard-coding coordinates.

This epic focuses on the visual asset model and renderer. It builds on the MCP
reliability contract, especially schema discovery and render verification.

## Goals

- Support Minecraft-like resizable GUI backgrounds through a 3x3/nine-slice
  model.
- Let users define default nine-slice guides on assets and override them per
  element.
- Keep ordinary texture elements simple when they do not need nine-slice
  rendering.
- Reuse one visual UV/guide editing experience for atlas-backed elements.
- Support button icons from standalone PNGs or atlas regions.
- Make MCP able to author and verify the same visual metadata as the UI.

## Non-Goals

- Do not build a full pixel editor.
- Do not implement arbitrary procedural texture effects.
- Do not implement editable state variants in this epic.
- Do not replace all current generated textures with nine-slice assets in one
  step.

## Asset Nine-Slice Metadata

Project asset metadata may define default nine-slice guides:

```json
{
  "name": "textures/gui/panel_atlas.png",
  "width": 64,
  "height": 64,
  "nine_slice": {
    "left": 4,
    "right": 4,
    "top": 4,
    "bottom": 4,
    "edge_mode": "tile",
    "center_mode": "tile"
  }
}
```

Guide values are measured in source pixels:

- `left`: width of the left column;
- `right`: width of the right column;
- `top`: height of the top row;
- `bottom`: height of the bottom row.

The first alpha implementation should support `tile`. `stretch` may be included
if it is cheap and well-tested, but pixel-art GUI backgrounds should prefer
tiling.

## Texture Element Rendering Mode

Extend the existing texture element instead of introducing a separate
background element type.

Fields:

```json
{
  "id": "background",
  "type": "texture",
  "asset": "textures/gui/panel_atlas.png",
  "x": 0,
  "y": 0,
  "width": 176,
  "height": 166,
  "render_mode": "nine_slice",
  "nine_slice": {
    "left": 4,
    "right": 4,
    "top": 4,
    "bottom": 4,
    "edge_mode": "tile",
    "center_mode": "tile"
  }
}
```

`render_mode` values:

- `plain`: draw the texture or selected UV region as today;
- `nine_slice`: draw corners, edges, and center using nine-slice metadata.

If `render_mode` is `nine_slice`, the renderer resolves guides in this order:

1. element-level `nine_slice`;
2. asset metadata `nine_slice`;
3. validation error or preview warning if neither exists.

Element-level fields are overrides. They may partially override asset defaults
when the implementation can resolve missing fields unambiguously.

## Guide Editor

Add a visual guide editor for nine-slice metadata. It should feel like a UV
editor rather than a form-only feature.

The editor shows the selected source image at pixel-art-friendly scaling and
four draggable guides:

- left vertical guide;
- right vertical guide;
- top horizontal guide;
- bottom horizontal guide.

It also exposes numeric fields for precise editing. Values should be clamped so
the corners cannot overlap or produce zero/negative center regions.

The same component family should support:

- selecting a rectangular UV region for texture/progress/button icons;
- editing nine-slice guides for an asset or element;
- previewing how a chosen target size tiles the edges and center.

## Renderer And Export

Pixi editor rendering, MCP render output, and exported texture compositing must
use the same nine-slice layout rules.

Rendering behavior:

- corners are copied without scaling;
- top/bottom edges tile or stretch horizontally into the target width;
- left/right edges tile or stretch vertically into the target height;
- center tiles or stretches into the remaining region;
- source rectangles are integer pixel regions;
- output should remain crisp for Minecraft pixel art.

Exported GUI textures should include the fully composed nine-slice background,
so resource packs can override the complete generated texture the same way they
do for normal Minecraft GUI textures.

## Button Icons

Button and toggle button elements should support icons from either:

- standalone PNG assets;
- an atlas PNG plus `icon_uv`.

Button properties and MCP tools should allow:

- text-only button;
- icon-only button with tooltip/fallback label;
- icon plus text if the renderer supports it cleanly.

The alpha priority is icon-only and text-only, because compact Minecraft GUI
controls commonly use icons.

## MCP Support

Expose MCP fields and tools for:

- setting asset-level nine-slice metadata;
- setting texture element `render_mode`;
- setting element-level nine-slice overrides;
- setting texture/progress/button UV rectangles;
- setting button icon asset and `icon_uv`;
- discovering accepted render modes and nine-slice fields through schema
  discovery;
- rendering a project after nine-slice changes through `project_render`.

Responses should remain compact and should not inline binary images unless
explicitly requested.

## Validation

Preview/export should warn when:

- a texture element uses `nine_slice` without resolvable guides;
- guides overlap or leave no center area;
- the target element is smaller than the fixed corner sizes;
- progress or icon UV rectangles exceed the selected asset bounds;
- a button references an icon asset but has an invalid or empty UV region.

Warnings should not block export unless the data is impossible to render.

## Testing

Backend/renderer tests:

- nine-slice guide resolution prefers element overrides over asset defaults;
- invalid guides produce warnings or validation errors;
- generated nine-slice output has expected dimensions;
- export composes nine-slice backgrounds into the final GUI texture;
- standalone icon and atlas `icon_uv` metadata round-trip through save/load.

Frontend/manual checks:

- define nine-slice guides on an imported asset;
- apply the asset to a background texture element at several sizes;
- override guides for one element without changing the asset default;
- choose a button icon from an atlas region;
- render/export and inspect the generated PNG.
