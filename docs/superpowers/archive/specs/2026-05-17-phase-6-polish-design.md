# Phase 6 Polish Design Spec

**Date:** 2026-05-17

## Goal

Phase 6 turns the verified Phase 1-5 baseline into a daily-usable editor: easier to start, easier to configure, easier to understand, and harder to lose context in. This phase is not about deeper Minecraft authoring power; it is about core UX polish with a strong, compact editor design pass.

## Product Direction

MCGUI Crafter should feel like a focused desktop production tool. The UI should stay dense, dark, and technical, but the common workflows must be obvious and low-friction:

- Start or resume work quickly.
- Configure grid/snap behavior without digging through code.
- Discover shortcuts and current MCP/export state.
- Preview export consequences before writing files.
- Edit small and larger textures comfortably.
- See failures and success states in a consistent place.

No landing page, marketing hero, or card-heavy redesign is introduced. Empty states are functional work surfaces, not promotional screens.

## Scope

### 1. Project Start And Recovery

When no project is open, the center editor area becomes a project launcher. It shows:

- New project action.
- Open project action.
- Recent projects from the existing localStorage list.
- A compact MCP endpoint/status display.
- A small list of GUI presets or template shortcuts.

The toolbar and app shell remain visible. Recent project failures remove stale entries only after user confirmation or a clear error state.

### 2. Editing Preferences

Add locally persisted editor preferences:

- Grid visible.
- Snap enabled.
- Major grid size.
- Minor grid size.
- Snap size.
- Default GUI preset.
- Theme setting, limited to the existing dark theme plus one high-contrast dark variant.

Preferences are edited through a compact toolbar popover or modal. They must affect the canvas immediately and persist between app launches.

### 3. GUI Size Presets

Add curated presets for common Minecraft GUI dimensions:

- Furnace / inventory: 176x166.
- Chest 9x3: 176x166 or project-specific template size if already defined.
- Chest 9x6: 176x222.
- Hopper: 176x133.
- Custom dimensions.

Presets appear in the New Project flow and preferences/start surface. Selecting a preset updates width and height without hiding manual input.

### 4. Shortcut And Help Reference

Add a keyboard shortcut reference dialog. It must list only shortcuts that exist or are implemented in the same phase.

Groups:

- Project: new, open, save, save as, export.
- Tools: select, pan, slot, texture, text.
- View: zoom in/out/reset, grid toggle, snap toggle.
- Editing: delete, duplicate, group, ungroup, undo, redo.
- Timeline: play/pause, reset preview.

The dialog is reachable from the toolbar and via keyboard.

### 5. Export Preview

Before export writes files, the export dialog shows a preflight preview:

- Sanitized mod id, package, class name, resource names.
- Target loader.
- Output directory.
- Planned file tree.
- Missing texture errors.
- Existing-file overwrite warnings when detectable.

The preview uses the same backend export planning logic as the write path, so it does not drift from actual export behavior.

### 6. Pixel Editor Zoom

The pixel editor gets zoom controls for 1x, 2x, 4x, 8x, and fit. The canvas remains pixelated and easy to inspect. Large sprites should not overflow the viewport unusably.

### 7. Status And Error UX

Add a compact status message system integrated with the app shell, preferably near the bottom/status bar:

- Success messages for save/export/import.
- Warning messages for export preflight and stale recent projects.
- Error messages for failed open/save/export/MCP status.

Avoid blocking alerts for normal errors. Browser fallback prompts may remain only where a platform dialog cannot be used.

### 8. Visual Design Pass

Apply design polish only where it supports the workflows above:

- Consistent toolbar icon/button sizing.
- Consistent modal/popover spacing.
- Tooltips for icon-only actions.
- Stable widths for status counters and project tabs.
- No new accessibility warnings.

## Architecture

Rust remains authoritative for durable project state, session state, export planning, and MCP state. Svelte stores remain the reactive UI mirror and local editor preferences owner.

New local editor preferences can live in the frontend because they are UI-only and do not affect saved `.mcgui` project data unless explicitly promoted later. Export preview should be backend-based because it must match the real export writer and account for filesystem state.

## Non-Goals

- Expression language for animation bindings.
- New animation engine behavior beyond shortcut/help exposure.
- Custom font import.
- Bedrock/resource-pack export.
- CI/release packaging.
- Full visual redesign or theme marketplace.

These remain candidates for Phase 7 or later Phase 6.x work after the app is polished.

## Spec Self-Review

- Scope is focused on core UX polish, not advanced Minecraft authoring.
- Export preview is backend-based so preview and write behavior cannot drift.
- Preferences are frontend-local because they do not affect `.mcgui` project data.
- Theme scope is limited to one additional high-contrast dark variant.
- Non-goals explicitly exclude expression bindings, custom fonts, Bedrock export, CI/release packaging, and full redesign.

## Acceptance Criteria

- `pnpm verify` passes with 0 Svelte errors and 0 warnings.
- `cargo fmt --all -- --check`, `cargo test`, and `cargo build` pass.
- Opening the app with no project presents useful start actions and recent projects.
- Grid/snap preferences persist and affect canvas behavior.
- Shortcut reference matches actual implemented shortcuts.
- Export preview catches missing textures before writing and shows the planned file tree.
- Pixel editor can comfortably inspect 16x16 and 256x256 textures.
- Save/open/export failures are surfaced through consistent status UI.
