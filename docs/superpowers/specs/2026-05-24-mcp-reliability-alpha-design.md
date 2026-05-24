# MCP Reliability Alpha Design

## Context

MCGUI Crafter is now good enough for agents to create real Minecraft GUI
references through MCP, but repeated agent reviews found several remaining
rough edges that make the workflow harder than it needs to be. Closed alpha
should make MCP authoring reliable, discoverable, compact, and visually
verifiable.

This is the first alpha epic. It defines the MCP behavior expected by the
visual authoring, state variant, and GUI polish epics, but it does not implement
those larger features.

## Goals

- Let an LLM author complete GUI projects through MCP without relying on manual
  UI workarounds.
- Provide a discoverable render/screenshot tool for visual verification.
- Add small reliability tools that remove known slow or awkward edit loops.
- Make schemas and validation explicit enough that agents can self-correct.
- Keep MCP responses compact and predictable for normal agent review.
- Preserve project compatibility with existing `.mcgui` files.

## Non-Goals

- Do not implement editable state variants in this epic.
- Do not implement nine-slice rendering in this epic.
- Do not implement full public release packaging or installer workflows.
- Do not redesign the desktop UI in this epic.

## Shared Alpha MCP Contract

Alpha MCP tools should follow a consistent contract:

- tool names use `snake_case`;
- mutating responses include `project_id` and enough compact metadata to verify
  what changed;
- binary assets and rendered images return metadata such as `path`, `width`,
  `height`, `bytes`, and `sha256`, not inline payloads by default;
- no-op mutations should not create history entries or misleading change
  events;
- visible state mutations must bump the project revision or otherwise trigger
  UI/editor synchronization;
- schema discovery should expose accepted enum values, default behavior, and
  field descriptions for element types, semantic groups, attached regions, and
  export settings.

Later alpha epics should reuse this contract for state and nine-slice tools.

## Render Tool

Add a dedicated MCP render tool, exposed in discovery as either
`project_render` or a stabilized `project_screenshot`. The preferred name is
`project_render` because it can represent deterministic project rendering
without depending on an OS window screenshot.

The tool renders the current project to a PNG and returns compact metadata:

```json
{
  "project_id": "uuid",
  "path": "docs/mcgui/screenshots/example.png",
  "width": 264,
  "height": 166,
  "bytes": 12345,
  "sha256": "..."
}
```

Inputs should include:

- optional output path;
- optional state id, reserved for the state variants epic;
- optional flags for overlay/background/composite if the renderer already has
  those concepts.

The tool should produce an image suitable for a multimodal agent to inspect.
For alpha, it is acceptable if runtime text rendering differs from the final
Minecraft screen as long as the limitation is documented in the response or
tool docs.

## Project Resize

Add `project_resize` for changing `gui_size`.

Behavior:

- updates only the project canvas/main GUI size;
- does not move, scale, or clamp existing elements;
- preserves elements outside the new bounds;
- returns old and new dimensions;
- triggers normal project synchronization and history behavior.

Agents are responsible for moving elements after resize. This keeps the tool
predictable and avoids hidden layout mutations.

## Group Updates

Add `group_upsert` or `group_update` so agents can update group membership
without calling `group_ungroup` followed by `group_create`.

Behavior:

- creating a missing group and updating an existing group are both supported;
- existing group metadata is preserved unless explicitly overwritten;
- membership replacement is explicit;
- the mutation creates one history entry;
- the response includes compact group metadata and member count.

This addresses the current `Group already exists` edit loop.

## Batch Editing

Add `element_update_many` as the minimum batch-editing improvement.

Inputs:

- list of element ids and patch objects;
- optional strict mode for whether one failed patch aborts the whole batch.

Behavior:

- returns compact per-element success/failure records;
- applies one project revision/history entry when possible;
- validates element type-specific fields the same way single-element updates do.

If attached region batch editing exists after the attached-region work,
`attached_region_update_many` should follow the same response shape.

## Semantic Authoring

Semantic groups should support explicit member element ids where the semantics
benefit from validation or code generation.

For alpha, prioritize:

- `control_buttons`: explicit button/toggle member ids;
- fixed slot groups: explicit slot member ids when a group is non-rectangular;
- player inventory and hotbar groups: grid metadata plus optional member ids.

Preview/export validation should warn when:

- a semantic group references missing elements;
- a `control_buttons` group references non-button elements;
- a fixed/player/hotbar group declares a slot count that does not match member
  ids or matching slot elements;
- a grid shape declares more visible cells than available matching slots.

The existing split-machine workflow remains valid: agents may create visual
slot grids first and then replace/update semantic metadata explicitly.

## Schema Discovery

Add or stabilize a schema-discovery tool that returns the accepted variants
agents need to author projects without guessing.

It should include:

- element types and their editable fields;
- semantic group kinds;
- accepted slot roles;
- export settings and codegen modes;
- attached region anchors/states/kinds if present;
- field defaults and whether default values are omitted on save/list output.

This should explicitly document cases like `dynamic_height: false`: either the
field is preserved, or default-value omission is documented as expected.

## Validation And Response Polish

Preview/export warnings should be stable and actionable. Existing output-file
warnings are useful, but common edit-preview-export loops should be able to opt
into a clean overwrite flow later. For this alpha epic, the minimum requirement
is that warnings are precise and not confused with schema or semantic errors.

Asset import/list responses must stay compact. Binary payloads should be
returned only behind an explicit opt-in field.

Element creation and batch responses should include enough fields to trust the
immediate result. If `element_list` later shows fields such as `layer`, creation
responses should either include them or document that defaulted fields are
omitted from mutation responses.

## Testing

Backend/MCP tests:

- `project_render` creates a PNG and returns compact metadata;
- `project_resize` changes only `gui_size` and preserves element coordinates;
- `group_upsert` creates and updates groups without the ungroup workaround;
- `element_update_many` applies valid batches and reports invalid patches;
- schema discovery lists semantic group kinds, slot roles, and element fields;
- semantic validation warns for missing/incorrect control button members;
- semantic validation warns for slot count mismatches;
- binary asset responses remain compact by default.

Manual alpha validation:

- open an existing `.mcgui`, resize it, update groups, render it, save it, and
  export it through MCP only;
- repeat the Auto Cutter and Food Pouch agent workflows without undocumented
  workarounds except the accepted split-machine semantic pass.
