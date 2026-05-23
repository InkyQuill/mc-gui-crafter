# Phase 6.x: Minecraft Visual Fidelity - Design Spec

## Overview

This spec defines a focused visual fidelity pass for MCGUI Crafter. The goal is to make new projects look useful by default, fix template sizing against vanilla-style GUI metrics, and introduce Minecraft-like light and dark editor themes without copying Mojang assets.

The approved direction is a hybrid:

1. Use vanilla Java GUI dimensions and slot metrics where they are known and stable.
2. Generate original Minecraft-like base textures for default project backgrounds and controls.
3. Keep user-imported textures authoritative: imported assets override generated defaults.
4. Keep high contrast as an accessibility theme, separate from the light/dark visual themes.

## Problems

### Template slot overflow

Some starter templates place slot grids using a step of `18 + 2` pixels. A standard Minecraft slot is already an 18x18 visual unit. Adjacent inventory slots are normally placed on an 18px cadence; the apparent separation comes from the slot border and shading, not a 2px empty gap.

This currently causes chest-style templates to exceed a 176px GUI panel. For example, a 9-column grid starting at x=8 with a 20px step spans beyond the panel width.

### Missing default textures

Templates include background texture elements such as `textures/background.png`, but newly generated projects may not contain actual image bytes for that path. The renderer then shows a generic checkerboard placeholder, which is useful for broken assets but poor as the default GUI experience.

### Theme mismatch

The app currently has a dark/high-contrast theme preference and many hardcoded interface colors. The default dark palette is more generic editor UI than Minecraft-like GUI tool. There is no light Minecraft-like theme.

## Goals

- New template projects show generated base GUI textures immediately.
- Empty or old projects without texture bytes do not display broken-looking placeholders for built-in default backgrounds.
- Light and dark themes feel Minecraft-like while staying original.
- High contrast remains available and legible.
- Vanilla-style templates use correct contiguous 18px slot cadence.
- Template dimensions remain compatible with existing exporters and coordinate conventions.
- Generated textures are deterministic, saved inside `.mcgui`, and exportable.

## Non-Goals

- Do not bundle or copy official Minecraft GUI textures.
- Do not attempt full parity with every vanilla screen in this pass.
- Do not replace the pixel editor or asset import flow.
- Do not redesign the whole application shell beyond theme tokenization needed for this pass.
- Do not introduce Bedrock JSON UI export here.

## Theme Design

### Theme options

`EditorPreferences.theme` should become:

```ts
type Theme = "light" | "dark" | "high_contrast";
```

Suggested defaults:

- Existing users with no stored preference: `dark`
- Invalid stored values: `dark`
- Existing `high_contrast`: preserved

### Light theme

The light theme should evoke vanilla stone and parchment UI:

- App background: muted stone gray
- Work surface: light warm gray
- Panels: pale stone
- Borders: dark bevel gray
- Primary accent: emerald/experience green
- Warning accent: gold
- Destructive accent: redstone red

### Dark theme

The dark theme should evoke deepslate, obsidian, and dark oak:

- App background: near-black deepslate
- Work surface: dark stone
- Panels: cool dark gray
- Borders: blackened bevel
- Primary accent: emerald/experience green
- Secondary accent: lapis blue
- Destructive accent: redstone red

### High contrast

High contrast should remain a functional accessibility mode:

- Black background
- White text
- Strong focus outlines
- Minimal subtle shading
- Accent colors chosen for contrast, not Minecraft flavor

### Tokenization

Introduce app-level CSS variables for common UI colors instead of continuing to spread hardcoded values through components:

```css
:root[data-theme="dark"] {
  --app-bg: #101214;
  --surface: #1f2326;
  --surface-raised: #2d3033;
  --border: #08090a;
  --text: #f2f2f2;
  --muted-text: #b8b8b8;
  --accent: #3aa655;
  --accent-2: #3f76b5;
  --danger: #b83a32;
  --warning: #d7a339;
}
```

The exact values can be tuned during implementation, but the implementation should centralize the palette before component-by-component polishing.

## Generated Texture Design

### Asset paths

Generated defaults should live under a clearly reserved namespace:

- `textures/generated/gui_panel.png`
- `textures/generated/slot.png`
- `textures/generated/progress_arrow.png`
- `textures/generated/fluid_tank.png`
- `textures/generated/energy_bar.png`

For backward compatibility, templates may continue to reference `textures/background.png` if we generate that alias too. New templates should prefer `textures/generated/gui_panel.png` unless existing exporter assumptions require the older path.

### Ownership

Generated texture bytes should be created by the Rust backend when creating a new project from a template. They should be inserted into:

- `Project.assets`
- `Project.texture_data`

This makes the textures visible to the asset library, saved inside `.mcgui`, and available to export without relying on frontend-only rendering.

The frontend renderer should also have a procedural fallback for built-in generated paths. This covers old projects and partially migrated files, but the saved project should be the source of truth after creation or migration.

### Style

Generated textures should be original pixel art primitives:

- GUI panel: beveled stone panel with a subtle noisy fill
- Slot: 18x18 beveled square with dark inner well and bright top-left highlight
- Progress arrow: gray beveled arrow background plus optional colored fill sprite
- Fluid tank: bordered glass-like well with dark interior
- Energy bar: bordered vertical well with redstone-colored fill

The renderer should avoid showing the generic missing-texture checkerboard for these reserved generated paths. The checkerboard remains correct for arbitrary missing user assets.

### Determinism

Texture generation should be deterministic. If noise is used, it should be derived from stable coordinates and a fixed seed, not a runtime RNG. This keeps tests and exports reproducible.

## Template Metric Design

### Slot cadence

Inventory-style slots should use:

```text
slot_size = 18
slot_step = 18
```

No additional gap should be added between adjacent inventory slots. Borders and bevels provide visual separation.

### Standard panel dimensions

Keep common template panels at vanilla-like sizes:

- Furnace: 176x166
- Crafting table style: 176x166
- Chest 9x3: 176x166
- Chest 9x6: 176x222

### Known immediate fixes

The following existing patterns should be audited and corrected:

- `crafting_3x3`: grid loop currently uses a 20px step.
- `chest_9x3`: grid loop currently uses a 20px step and can overflow.
- `chest_9x6`: grid loop currently uses a 20px step and can overflow.
- Any later templates copied from those patterns should be checked for the same issue.

### Template tests

Add or update tests to assert:

- All slot elements are inside the project width/height.
- Standard inventory grids use an 18px cadence.
- 9-column grids span exactly 162px from first slot x to final slot right edge.
- Generated default background asset exists for template projects.

## Migration and Compatibility

Existing `.mcgui` files should continue to open unchanged.

On open, the app may add generated default texture bytes only if all of these are true:

1. The project references a reserved generated texture path or legacy `textures/background.png`.
2. The asset is missing from `texture_data`.
3. The user has not imported an asset with the same path.

If migration is implemented, it should mark the project dirty only when it actually inserts missing bytes.

## UI Changes

### Preferences

Update the theme selector to show:

- Dark
- Light
- High contrast

### New project/template flow

New projects created from templates should show generated default textures immediately. The user should not need to import a background texture before the canvas looks like a Minecraft GUI.

### Asset library

Generated assets should appear as normal project assets, but the UI can label them as generated if that metadata becomes available later. No metadata field is required for this pass.

## Implementation Areas

Likely files/modules:

- `src-tauri/src/templates/mod.rs`: template coordinates and default asset references
- `src-tauri/src/project/mod.rs`: project creation or migration hooks
- `src-tauri/src/texture/mod.rs`: deterministic generated PNG creation
- `src/lib/engine/renderer.ts`: reserved-path fallback rendering and canvas background colors
- `src/lib/stores/preferences.svelte.ts`: theme type and default handling
- `src/lib/components/PreferencesDialog.svelte`: theme selector
- Svelte component styles: replace hardcoded palette values with theme tokens where needed
- `docs/roadmap.md`: roadmap entry and status tracking

## Acceptance Criteria

- Creating a furnace, crafting, or chest template shows a Minecraft-like base GUI background by default.
- User-imported textures still replace generated defaults.
- Chest and crafting slots no longer protrude outside their GUI backgrounds.
- Light, dark, and high-contrast themes are selectable and persist through preferences.
- Existing projects without generated assets still render intelligibly.
- Tests cover template bounds, slot cadence, and generated texture insertion.
- `pnpm check`, `pnpm build`, `cargo test --all-features --locked`, and `pnpm tauri build` pass.

## Open Questions for Implementation

- Whether to generate `textures/background.png` as the primary default path or only as a compatibility alias.
- Whether generated assets need explicit metadata in the project model later.
- Whether template migration should happen eagerly on open or lazily when the project is saved.
