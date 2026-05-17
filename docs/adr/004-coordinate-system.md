# ADR 004: Coordinate System

**Date:** 2026-05-17  
**Status:** Accepted

## Context

Minecraft GUI rendering uses a specific coordinate system. The editor must follow this convention to ensure exported coordinates work correctly in generated mod code.

## Decision

**Minecraft-native convention: top-left origin, Y increases downward, pixel units.**

### Reference Frame

Minecraft renders GUI screens using two reference frames:

1. **Screen space** — (0, 0) is the top-left of the game window
2. **GUI space** — (0, 0) is the top-left of the GUI background texture, positioned at `(guiLeft, guiTop)` in screen space

MCGUI Crafter works exclusively in **GUI space**. All element coordinates are relative to the GUI's top-left corner.

```
(0,0) ──────────────── (width, 0)
 │                         │
 │    [slot]   [arrow]     │
 │    (56,17)  (79,35)     │
 │                         │
 │    Player Inventory     │
 │    (8, 84)              │
 │                         │
(0, height) ─────────── (width, height)
```

### Common GUI Sizes

| GUI Type | Width × Height |
|----------|---------------|
| Player inventory | 176 × 166 |
| Chest (9×3) | 176 × 166 |
| Chest (9×6) | 176 × 222 |
| Furnace | 176 × 166 |
| Brewing stand | 176 × 166 |
| Hopper | 176 × 133 |
| Custom (free) | Any > 0 |

### Slot Grid Offsets

Standard 18×18 slots with 2px gap:

```
Row 0: x = 8,   y = 84   (player inventory)
       x = 8+18+2, y = 84
       x = 8+2*(18+2), y = 84
       ...

Row 1: x = 8,   y = 84+18+2
       ...
```

### Text Positioning

Text is positioned by its top-left pixel. Minecraft renders text with a shadow offset of (1, 1) by default. Font glyph width is 6–7 pixels depending on character. Standard font height is 8 pixels.

### Coordinate Precision

All coordinates are stored as **integers** (pixels). Sub-pixel precision is not needed for Minecraft GUI rendering which works at a fixed integer grid.

## Exported Code Convention

When generating mod code, coordinates are exported as named constants:

```java
// Forge Screen class
public class FurnaceScreen extends AbstractContainerScreen<FurnaceMenu> {
    private static final int GUI_WIDTH = 176;
    private static final int GUI_HEIGHT = 166;

    private static final int INPUT_SLOT_X = 56;
    private static final int INPUT_SLOT_Y = 17;
    private static final int FUEL_SLOT_X = 56;
    private static final int FUEL_SLOT_Y = 53;
    private static final int OUTPUT_SLOT_X = 116;
    private static final int OUTPUT_SLOT_Y = 35;

    private static final int PROGRESS_ARROW_X = 79;
    private static final int PROGRESS_ARROW_Y = 35;
    private static final int PROGRESS_ARROW_W = 22;
    private static final int PROGRESS_ARROW_H = 15;
    // ...
}
```

The `guiLeft` and `guiTop` values are computed by the Screen's `init()` method as `(width - GUI_WIDTH) / 2` and `(height - GUI_HEIGHT) / 2` respectively.

## Editor Canvas

In the editor, the canvas renders the GUI at 1×, 2×, or 3× zoom. The coordinate grid overlay shows major grid lines at every 18 pixels (the standard slot size) and minor lines every 2 pixels (slot gap). A crosshair follows the cursor showing exact coordinates.

## Consequences

- Export code directly uses the stored coordinates — no transformation needed
- Templates use standardized offsets (e.g., player inventory always at `y = height - 82`)
- Any GUI size is supported; templates adapt their inventory offset accordingly
- The pixel grid editor tool snaps to even pixel coordinates by default (configurable)
