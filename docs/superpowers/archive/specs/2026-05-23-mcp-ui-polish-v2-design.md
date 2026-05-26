# MCP UI Polish v2 Design

## Goal

Close the remaining MCP workflow gaps found in the 2026-05-23 v2 review:
agents need a screenshot/preview image path, UI users need first-class button
authoring, button elements need icon and tooltip metadata, and MCP/export
responses should be less surprising in common iteration loops.

## Scope

Implement one compatible polish pass over the existing project format:

- add optional button icon and tooltip metadata;
- expose button/toggle creation and editing in the UI;
- render and export button icons from either standalone PNG assets or atlas UVs;
- add a compact MCP screenshot/render-preview tool;
- make immediate MCP element responses include the same effective layer fields as
  `element_list`;
- remove trailing whitespace from generated Java;
- add an overwrite-oriented export option for edit-preview-export loops;
- add low-noise preview warnings for progress stretching and optional
  `control_buttons` mismatch cases.

Do not implement full runtime button interaction, click handlers, or Minecraft
tooltip behavior in generated screens yet. Tooltip metadata is preserved for
layout/codegen consumers and future runtime helpers.

## Data Model

Extend `Element` with optional fields:

- `icon?: string` / `icon: Option<String>`
  - asset path for a button icon, for example
    `textures/gui/icons/settings.png` or `textures/gui/widgets.png`;
- `icon_uv?: UvRect | null`
  - optional atlas region inside `icon`; when absent, the whole PNG is used;
- `tooltip?: string`
  - author-facing tooltip text preserved in `.mcgui` and layout JSON.

`content` remains the button label/accessibility name. When both `icon` and
`content` are present, the first implementation renders the icon visually and
keeps `content` as metadata/runtime label. Text-only buttons keep using
`content`.

All new fields are optional and skipped during serialization when empty, so old
projects remain compatible.

## UI Behavior

`ElementPalette` gains `Button` and `Toggle` tools. Defaults:

- `button`: `52x20`, `asset: "textures/generated/button.png"`,
  `layer: "background"`;
- `toggle_button`: `20x20`, `asset: "textures/generated/button.png"`,
  `layer: "background"`.

`PropertyPanel` should expose button/toggle fields:

- width and height;
- label/content;
- tooltip;
- icon asset select;
- icon UV rectangle controls;
- binding key.

The existing bug where button text cannot be edited in Properties is fixed by
sharing text controls between `text`, `button`, and `toggle_button` where
appropriate.

## Rendering And Export

Pixi rendering order for buttons:

1. draw button chrome/background;
2. if `icon` is set, draw the icon centered inside the button, cropping by
   `icon_uv` when provided;
3. otherwise draw the centered label from `content`.

Export baking follows the same visual rule: the static GUI texture contains the
button chrome and icon pixels. Generated Java may still render text labels for
text-only buttons. For icon buttons, layout JSON preserves `content`,
`tooltip`, `icon`, and `icon_uv` so generated helpers can attach runtime
behavior later.

## MCP Screenshot Tool

Add a read-only MCP tool named `project_screenshot`.

Input:

- optional `project_id`;
- optional `output_path`;
- optional `include_data_url`, default `false`.

Behavior:

- render the current project to a PNG using the same project preview/export
  composition path used for generated GUI textures;
- write it to `output_path` or a temp file;
- return compact metadata:
  - `path`;
  - `width`;
  - `height`;
  - `bytes`;
  - `sha256`.

If `include_data_url` is true, include `data_url` as an explicit large payload.
The default stays compact so normal agent logs do not fill with base64.

## MCP And Export Ergonomics

`element_add_many` should return elements through the same MCP presentation path
as `element_list`, including effective default fields such as
`layer: "background"`.

Export requests accept `overwrite?: boolean`. When true, existing generated
target files are allowed and the existing-file preview warnings are suppressed.
This is safer than `clean_output` because it does not delete extra files from an
output directory.

Generated Java should not contain trailing whitespace. Fix this in the generator
or final formatting step so exported artifacts pass whitespace checks when
copied into a mod repository.

## Semantic And Asset Warnings

Add preview warnings that are helpful but not blocking:

- progress/stretch warning:
  - when a `progress` element references a PNG texture and its element
    `width`/`height` differ from the texture dimensions;
  - wording should make clear that stretching is allowed but may be accidental
    for pixel-art GUIs;
- `control_buttons` warning:
  - only for groups that provide enough metadata to validate, such as
    `data_source`, explicit project group membership, or expected count;
  - warn if no matching `button` or `toggle_button` elements can be found;
  - avoid warnings for intentionally loose metadata.

## Testing

Rust tests:

- `.mcgui` round-trip preserves `icon`, `icon_uv`, and `tooltip`;
- export/layout JSON includes those fields;
- export baking includes icon pixels for standalone PNG and atlas UV icon
  cases;
- `project_screenshot` returns compact metadata and writes a PNG;
- `project_screenshot include_data_url` returns a data URL only when requested;
- `element_add_many` response includes effective `layer`;
- `overwrite: true` suppresses existing-file warnings;
- generated Java has no trailing whitespace;
- progress stretch warning fires for mismatched dimensions;
- `control_buttons` validation warns only when metadata is specific enough.

Frontend checks:

- `pnpm check`;
- `pnpm build`;
- browser smoke: create a button from the palette, edit label/tooltip/icon,
  verify it renders and properties update.

MCP smoke:

- create a project;
- add a button with icon and tooltip via MCP;
- call `project_screenshot`;
- export with `overwrite: true`;
- inspect screenshot/export metadata.

## Open Decisions

No open product decisions remain. Icon support must handle both standalone PNGs
and atlas UVs in the first implementation.
