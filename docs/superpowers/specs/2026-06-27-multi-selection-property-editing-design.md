# Multi-Selection Property Editing Design

Date: 2026-06-27

## Context

MCGUI Crafter currently has partial multi-selection state in the editor store and
canvas drag handling, but the object list and property panel do not present a
normal GUI-editor workflow. The current "Apply to all slots" action is
misleading because the target set is implicit. Batch editing must be driven by
the user's explicit selection, the same way layer-based editors such as
Photoshop define edit targets.

## Goals

- Let users select multiple objects from the object list with `Ctrl`/`Cmd+click`
  and `Shift+click`.
- Make the properties panel edit all selected objects when a supported field is
  changed.
- Keep the object-list selection and editor selection frames mirrored so the
  user always sees the same selected objects in both places.
- Show mixed values clearly instead of choosing an arbitrary object's value.
- Remove implicit bulk actions such as "Apply to all slots".
- Prevent ambiguous geometry edits from the properties panel during
  multi-selection.

## Non-Goals

- Multi-selection does not add bulk numeric position or size editing.
- Mixed-type selection does not expose type-specific fields.
- This design does not change backend project structure or export semantics.
- This design does not add lasso selection or marquee selection.

## Selection Behavior

The object list becomes the primary precise selection surface.

- Plain click selects one object and clears the previous object selection.
- `Ctrl+click` on Linux/Windows and `Cmd+click` on macOS toggles one object in
  the current selection.
- `Shift+click` selects a contiguous range in the currently visible object list
  order. The range is anchored to the most recent non-shift object-list
  selection.
- Selected rows show a visible selected state for every selected object, not only
  the primary selection.
- The primary selected object remains the last clicked selected object. It is
  used for focus, scroll anchoring, and any single-object-only affordance.
- Editor selection frames mirror the full object-list selection. If an object is
  selected in the list, its canvas frame is drawn; if a canvas frame is selected,
  the corresponding list row is selected.

Canvas selection should follow the same modifier model where practical:
`Ctrl`/`Cmd+click` toggles an object, `Shift+click` may continue to add objects
for compatibility, and dragging an already-selected object moves the whole
selected set.

Multi-selection frames are for selection identity and group dragging only. They
must not draw resize handles or any other resize nodes. Resize handles are a
single-object affordance only.

## Property Panel Model

The properties panel derives an editable field set from the current selection.

For a single selected object, behavior remains equivalent to the current
single-object inspector.

For a multi-selection of one object type, the panel shows fields common and
meaningful for that type. For slots, this includes slot texture/background,
inventory metadata where applicable, layer, visibility, and attached region. It
does not show editable position, size, or coordinates.

For a mixed-type multi-selection, the panel shows only compositing fields:

- Layer
- Visibility
- Attached region

The underlying project field remains `attached_region`; the UI label should be
`Attached region`. An attached region is a named geometry and anchoring
container for elements that live outside or alongside the main GUI rectangle,
such as side panels, upgrade pockets, return pockets, floating toggles, or
decorative flair. Child elements keep normal absolute coordinates relative to
the main GUI origin and store the region id in `attached_region`. Moving the
region moves its assigned children together; semantic groups still describe
runtime meaning separately.

## Mixed Values

When selected objects have different values for a visible field, that field
shows a clear mixed state.

- Select inputs show `Mixed` as a disabled placeholder.
- Text inputs show an empty value with a `Mixed` placeholder.
- Checkboxes use an indeterminate visual state.
- Asset fields show `Mixed` until the user picks a specific asset.

Changing a mixed field applies the new value to every selected object that
supports that field. Unsupported fields are not shown, so the user is never
asked to reason about partial application for a visible control.

## Batch Update Flow

The property panel should call explicit multi-update helpers rather than looping
ad hoc in component event handlers. The multi-update helper should:

- Accept selected object IDs and a patch.
- Validate selected IDs and the patch up front.
- Reject missing IDs, duplicate IDs, and unsupported fields before any history or
  change is recorded.
- Apply updates only if the full batch is valid.
- Preserve undo/history behavior consistently with existing single-object
  updates.
- Produce one user-visible action for one property-panel edit.
- Log the action through the existing session logging path.

This keeps the property panel focused on UI state and keeps batch semantics
centralized. If the UI needs skip/continue behavior, it should pre-filter the
selection before invoking the helper so the helper remains atomic.

## Error Handling

If a selected object disappears during an edit, the frontend should either
pre-filter it before calling the helper or surface the helper's rejection. If no
selected object supports the field being changed, the edit is ignored and a
warning is written to the session log.

If a backend update fails, the status bar should report the failure using the
existing status mechanism, which also records the error in the session log.

## Testing

Verification should cover:

- Object-list `Ctrl`/`Cmd+click` toggles selection.
- Object-list `Shift+click` selects a contiguous visible range.
- Multi-selected rows are all visibly selected.
- Canvas selection frames mirror the selected rows in the object list.
- Multi-selection canvas frames do not draw resize handles.
- Same-type slot selection exposes slot texture/background fields and applies
  changes to all selected slots.
- Same-type selection hides or disables position and size fields.
- Mixed-type selection exposes only layer, visibility, and attached region.
- Mixed values render as mixed placeholders or indeterminate controls.
- Removing the old "Apply to all slots" action leaves no implicit bulk-edit path.
- Existing canvas dragging still moves selected objects as a set.

Manual verification should include the current food pouch project with
`background`, `food_8`, and `food_17`, because it exercises mixed texture/slot
selection and same-type slot batch editing.
