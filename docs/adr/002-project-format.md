# ADR 002: Single-file project format

**Status:** Accepted; implementation notes refreshed 2026-10-09.

## Decision

A `.mcgui` file is a ZIP archive so project data and textures travel together.
The reader/writer in `src-tauri/src/format/mod.rs` is the format authority.

- `manifest.json`: version, name, GUI size and mod target.
- `layout.json`: elements, groups, attached regions, states/overrides, semantic
  groups, asset metadata and export settings.
- `animations.json`: optional animation definitions.
- `fonts.json`: optional font metadata/data as written by the format module.
- `textures/**/*.png`: embedded texture bytes.

Saving writes a sibling `.tmp` archive and renames it over the destination.
The runtime project path, dirty flag, selection and undo history are session
state rather than archive data. Missing optional layout collections default to
empty values for older projects.

## Limitations

The archive manifest currently omits `main_gui_center`, even though runtime
project state and exports support it. The required save/reopen correction and
legacy fallback regression are tracked in [the backlog](../backlog.md).

ZIP input must be treated as untrusted project data. A format change should
include round-trip and legacy-file tests, not just serde model tests.
