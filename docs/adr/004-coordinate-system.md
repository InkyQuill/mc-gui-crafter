# ADR 004: Coordinate system

**Status:** Accepted; implementation notes refreshed 2026-10-09.

## Decision

Element coordinates use integer pixels relative to the main GUI's top-left
corner. X increases rightward; Y increases downward. Zoom/pan change the editor
view, not stored coordinates. Attached regions may occupy negative coordinates
or extend beyond the main GUI rectangle.

The project has an explicit `main_gui_center` (x, y), defaulting to the midpoint
of `gui_size`. Exported screen placement uses this reference point to center the
main GUI independently of attached-region visual bounds. Inspector controls and
canvas axis overlays expose the same values. Persistence of custom centers is
still tracked in [the backlog](../backlog.md).

Standard Minecraft slot cells have an 18-pixel pitch, not 18 pixels plus another
2-pixel gap. Templates and semantic grid helpers supply their own inventory
origins and dimensions. Editor grid visibility, spacing and snap are configurable.

## Consequences

Export and preview must preserve the same coordinate frame. Visual bounds must
not silently recenter the main GUI. Cover non-default centers, attached regions,
legacy projects and all three loaders when changing placement logic.
