# Editable State Variants Alpha Design

## Context

Some Minecraft GUIs need multiple visual states: collapsed and expanded drawers,
tab-like side panels, module panels, and controls that only appear in a specific
mode. Current MCGUI Crafter projects can model an expanded static layout, but
they cannot represent alternate visible states in one source project.

Closed alpha should introduce editable state variants as project metadata and
layout overrides. Runtime toggle behavior can remain a later code generation
feature.

## Goals

- Let one `.mcgui` project contain base, collapsed, expanded, and similar
  layout states.
- Avoid duplicating entire projects for each state.
- Keep the base layout canonical and store state-specific overrides only.
- Limit alpha overrides to geometry, visibility, attached region, and layer
  membership.
- Make state variants renderable and exportable for visual review.
- Preserve simple projects that have no states.

## Non-Goals

- Do not generate full runtime open/closed behavior in this epic.
- Do not support per-state semantic group definitions beyond visibility/layout
  effects.
- Do not support arbitrary property overrides such as text, slot role, or
  texture changes in alpha.
- Do not implement animated transitions between states in alpha.

## Project Model

Add a `states` collection to the project:

```json
{
  "states": [
    {
      "id": "collapsed",
      "label": "Collapsed",
      "description": "Main pouch layout without the settings drawer.",
      "initial": true,
      "export_role": "collapsed"
    },
    {
      "id": "expanded",
      "label": "Expanded",
      "description": "Settings drawer visible.",
      "export_role": "expanded"
    }
  ]
}
```

The base project remains the canonical layout. A state stores overrides:

```json
{
  "state_overrides": {
    "expanded": {
      "elements": {
        "settings_drawer_panel": {
          "visible": true,
          "x": 176,
          "y": 0,
          "width": 88,
          "height": 166,
          "attached_region": "settings_drawer",
          "layer": "overlay"
        }
      },
      "groups": {},
      "attached_regions": {
        "settings_drawer": {
          "visible": true,
          "x": 176,
          "y": 0,
          "width": 88,
          "height": 166
        }
      }
    }
  }
}
```

Allowed element override fields in alpha:

- `visible`;
- `x`;
- `y`;
- `width`;
- `height`;
- `attached_region`;
- `layer`.

Allowed attached-region override fields in alpha:

- `visible`;
- `x`;
- `y`;
- `width`;
- `height`.

Group overrides should be limited to visibility/layout-style behavior where the
existing group model can support it cleanly. If group geometry is ambiguous,
the implementation should use element and attached-region overrides first.

## State-Owned Regions And Groups

Support lightweight ownership metadata for groups and attached regions:

```json
{
  "id": "settings_drawer",
  "state_owned": ["expanded"]
}
```

Alpha meaning:

- the editor can hide or visually mark the region when another state is active;
- MCP/list responses can show that the region belongs to a state;
- export/layout JSON preserves the relationship.

This is metadata and editor behavior, not runtime logic. Runtime toggleable
attached regions remain a roadmap item.

## Effective Layout

The editor, MCP render tool, and export preview calculate an effective layout:

1. start from the base project;
2. apply state overrides for the active state;
3. render/export the resulting element geometry and visibility.

If no state is active, the base layout is used. If a project has states, one
state may be marked `initial`. If no state is marked initial, the first state in
project order may be treated as the initial state for UI convenience.

## Editor Behavior

Add a state selector to the editor toolbar or inspector. The editor should make
the current editing scope clear:

- **Edit base**: changes write to base element/group/region fields.
- **Edit state override**: geometry, visibility, attached region, and layer
  changes write to the selected state override.

In a non-base state, alpha behavior should be conservative:

- moving/resizing/visibility changes create or update overrides;
- content, texture, semantic, and slot metadata edits remain base edits;
- Properties should show whether a field is inherited or overridden;
- overridden/state-owned rows in Layers should have visible markers;
- clearing an override returns the field to its base value.

This prevents accidental creation of many near-duplicate element definitions.

## MCP Support

Add MCP tools:

- `state_list`;
- `state_add`;
- `state_update`;
- `state_remove`;
- `state_set_active`;
- `state_override_update`;
- `state_override_clear`.

Batch override update may be added if useful for drawer workflows.

Existing element tools should default to base edits unless the caller provides
`state_id` or `edit_scope: "state"`. This avoids surprising existing agents.

`project_screenshot` should accept `state_id` so agents can visually inspect
collapsed and expanded layouts.

Schema discovery should list state override fields and explain which existing
tools accept `state_id`.

## Export

Exported layout JSON should include:

- base project layout;
- state definitions;
- state override metadata;
- effective render metadata where needed for generated assets.

For alpha, generated Java may treat states as reference metadata. It may emit
comments or non-runtime helper scaffolding, but it is not required to implement
drawer opening/closing.

Exported screenshots/rendered references should support per-state rendering so
mod authors can inspect collapsed and expanded assets separately.

## Validation

Preview/export should warn when:

- a state override references a missing element, group, or attached region;
- a state has duplicate ids or no valid label;
- multiple states are marked `initial`;
- an override attempts to set a field outside the alpha allowlist;
- a state-owned attached region is not visible in any owning state.

Validation should not require every project to have states.

## Testing

Backend/model tests:

- save/load round-trips state definitions and overrides;
- effective layout applies overrides without mutating the base layout;
- clearing an override restores inherited base values;
- invalid override fields are rejected or warned consistently;
- render/export can target a specific state.

Frontend/manual checks:

- create collapsed and expanded states for a drawer-style GUI;
- mark the drawer attached region as expanded-state-owned;
- move/resize drawer elements in the expanded state without changing base;
- render both states through MCP;
- save, reopen, and verify the same effective layouts.
