# Backlog

Outstanding work as of 2026-10-09. This replaces the old phased roadmap,
completed implementation plans, and point-in-time AI review reports. Completed
work remains in Git history. Items below are not claims that old review findings
still reproduce; confirmed defects and future capabilities are separated.

## Correctness and release qualification

- [ ] Add real editor interaction coverage for multi-selection (Ctrl/Cmd toggle,
  Shift range, mixed values, one undo per batch), tab switching, and center axes.
  The implementation exists; historical plan checkboxes and browser API mocks
  do not establish native end-to-end coverage.
- [ ] Compile and run representative generated Forge, Fabric and NeoForge mods
  against explicitly supported Minecraft versions. Export unit tests alone do
  not qualify game integration, slot interaction or runtime rendering.
- [ ] Bound per-session undo/redo memory. `ProjectSessionManager` currently
  retains full snapshots without a history limit. Cover large texture projects
  and preserve expected undo behavior when enforcing a budget.
- [ ] Profile long export/render operations and session-lock contention before
  changing locking or adding caches. Old reports about unbounded text caches
  and full-image `asset_list` responses are obsolete in the current code.

## Editor and Minecraft runtime

- [ ] Expression support for animation bindings, such as
  `cook_time / total_cook_time`.
- [ ] Movable/pinnable panels, workspace profiles, richer Asset/UV panes, and
  stacked Layers/Assets. Current inspector docking and layout reset already exist.
- [ ] Full runtime container/menu generation for semantic inventories and
  virtual storage grids; generated screen/rendering scaffolds are not gameplay
  inventory implementations.
- [ ] Toggleable attached regions: open/closed state, click bindings, transitions,
  conditional slot activation, and modular runtime helpers. Static attached
  regions and authorable state variants already exist.
- [ ] Custom font support in exported Minecraft runtime. Font import and editor
  preview exist; generated screens use the platform text renderer.
- [ ] Parameterized custom-grid UI. MCP `slot_grid_add` already supports grid
  parameters; the New Project template is a fixed 3×3 starter.
- [ ] Research Bedrock JSON UI/resource-pack export; no implementation commitment.

## Distribution and community

- [ ] Qualify Linux package startup on supported Debian/Ubuntu, Fedora and Arch
  versions in isolated displays, including desktop launch and project opening.
  Package inspection and pacman installation do not prove GUI startup.
- [ ] Windows/macOS packaging and signing; AppImage and Linux ARM64 only when
  explicitly added to the supported release matrix. Current scope is Linux x86_64
  deb/rpm/pacman.
- [ ] Documentation website, MCP marketplace listing, community template sharing,
  integration guides for Create/Thermal/Mekanism, and video tutorials.
