# ADR 005: Export system

**Status:** Accepted; implementation notes refreshed 2026-10-09.

## Decision

Rust builds an export plan containing output files, warnings and errors before
writing. Preview and export share this planning path, so callers can inspect
missing assets, unsafe identifiers and overwrite conflicts before committing.

Targets are Forge, Fabric and NeoForge. Export supports full-mod scaffolding or
textures-only scope; project settings select simple or modular code generation.
Full output includes Java sources/loader setup, Gradle metadata, layout JSON and
textures under a Minecraft resource tree. Generated integration instructions
accompany the output. Actual filenames depend on the class, mod ID and settings.

The implementation in `src-tauri/src/export/mod.rs` owns current loader APIs,
file planning and compositing rules. Project state variants can be resolved for
export. Main GUI center controls screen placement; attached-region bounds do not
replace that reference point.

## Limits

Generated screens and runtime helpers are scaffolds, not complete gameplay
container/menu implementations. Semantic inventory behavior, toggleable attached
regions and custom runtime fonts remain [backlog items](../backlog.md).
Compiling and running exported projects against supported Minecraft/loader
versions is a separate qualification step from testing generated strings.
