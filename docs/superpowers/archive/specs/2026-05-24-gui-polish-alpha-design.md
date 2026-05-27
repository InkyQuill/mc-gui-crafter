# GUI Polish Alpha Design

## Status

This spec is now the remaining GUI polish slice for closed alpha. The original
editor UX polish scope has mostly been implemented and archived at
`docs/superpowers/archive/specs/2026-05-24-editor-ux-polish-design.md`.

Implemented behavior such as generated background elements, the inspector dock,
UV editing, layer/browser density, state variants, config persistence, and
reset shortcuts should not regress. This document focuses on the remaining
top-chrome polish needed before alpha.

## Context

The editor now has enough manual authoring surface for closed alpha, but the top
toolbar has become crowded. Project tabs currently live in the primary toolbar
beside file actions, view controls, state controls, preferences, and the project
name. On smaller windows this leaves too little room for open project tabs and
forces unrelated controls to compete for horizontal space.

Project tabs are navigation, not a primary command group. They should move into
a compact secondary toolbar directly under the main toolbar.

## Goals

- Move open-project tabs out of the primary toolbar into a dedicated second row.
- Give project tabs substantially more horizontal space without hiding core
  commands.
- Keep the primary toolbar focused on commands and mode controls.
- Preserve existing project switching, closing, dirty markers, and hydration
  behavior.
- Keep the no-project start screen from losing vertical space to an empty tab
  row.
- Improve top-chrome accessibility and overflow behavior as part of the move.
- Update this spec so it describes current remaining work rather than completed
  editor polish.

## Non-Goals

- Do not implement draggable/reorderable project tabs.
- Do not add pinned projects, tab groups, split editors, or workspace profiles.
- Do not build a movable/pinnable workspace framework.
- Do not redesign the inspector dock, element palette, canvas, timeline, or
  status bar in this slice.
- Do not revisit generated background, UV editor, or config persistence behavior
  except to prevent regressions.

## Two-Row Toolbar

The top chrome should be structured as two rows within the existing toolbar
component boundary:

- **Primary toolbar**
  - app logo;
  - New/Open/Save/Save As/Export;
  - Undo/Redo;
  - Grid and zoom controls;
  - active state selector and Base/State scope toggle;
  - shortcuts and preferences.
- **Project tabs toolbar**
  - open project tabs;
  - dirty markers;
  - close buttons;
  - optional active-project context on the right.

`ProjectTabs` should be removed from the primary toolbar row. The second row
should span the available window width so tabs no longer compete with command
groups. The row should remain compact, roughly the height of the current tab
buttons, and should use the same restrained editor styling as the rest of the
app.

When there are no open sessions, the project tabs toolbar should not reserve
visible height. The start screen should keep its current vertical space.

## Project Tabs Behavior

Existing tab behavior remains the source of truth:

- each open session renders one tab;
- the active tab is visually distinct and exposes `aria-current`;
- dirty projects keep a dirty marker;
- each tab keeps a close button with an accessible name;
- switching a tab uses the existing project switch flow;
- closing a tab uses the existing project close flow;
- switching or closing still clears selection and resets the canvas view as it
  does today.

Long project names should use ellipsis. The active tab or optional right-side
context may show the full current project title only if it does not reduce tab
capacity. If that context would crowd the row, the active tab title is enough.

## Overflow And Responsive Rules

The primary toolbar should continue to use compact controls at narrower widths,
but project tabs must no longer cause primary toolbar clipping.

The second toolbar owns tab overflow:

- tabs should have a readable minimum width;
- tabs may flex up to a reasonable maximum width;
- overflow should stay inside the tab row through horizontal scrolling or a
  clipped/faded tab strip;
- overflow must not push command controls offscreen;
- close buttons should remain reachable for visible tabs.

The responsive target is practical desktop use, not a mobile-first redesign.
The layout should be checked at wide desktop, the existing 940px breakpoint, and
a narrow desktop/mobile-ish viewport.

## Accessibility

The tab row should keep clear navigation semantics:

- the container remains labelled as open projects;
- active tab state is exposed with `aria-current="page"` or an equivalent
  pattern already used by the component;
- close buttons include project-specific `aria-label` text;
- focus outlines remain visible on tab buttons and close buttons;
- keyboard tab order follows visual order: primary commands first, project tabs
  second, workspace content after.

The primary toolbar's state scope buttons and icon buttons should keep their
current accessible names and pressed/disabled state behavior.

## Component Boundaries

`Toolbar.svelte` remains responsible for command handlers and project-tab
callbacks:

- `handleSwitchProject`;
- `handleCloseProject`;
- file actions;
- undo/redo;
- view controls;
- state selector and edit-scope changes.

`ProjectTabs.svelte` remains a presentation component. It should continue to
receive:

- `sessions`;
- `activeProjectId`;
- `onswitch`;
- `onclose`.

The implementation should not introduce new store state for tab placement. The
move is layout-only unless a small derived display helper is needed.

## Remaining GUI Polish Items

After the tab-row move, the remaining polish pass should cover:

- primary toolbar density after `ProjectTabs` is removed;
- second-row spacing, borders, active state, and empty state;
- tab overflow behavior with many open projects;
- no-project layout behavior;
- high-contrast and light/dark theme compatibility;
- basic keyboard and screen-reader affordances for the top chrome.

Future workspace framework scope remains separate:

- movable and pinnable panels;
- workspace profiles;
- richer asset/UV workspace panes;
- optional stacked or pinned Layers/Assets behavior;
- advanced project-tab management.

## Testing

Automated checks:

- `pnpm check`;
- `pnpm run build`;
- Svelte autofixer on changed `.svelte` files;
- existing Rust tests should not need changes for this layout-only slice.

Manual checks:

- open no project and confirm the second toolbar is hidden;
- create or open one project and confirm one tab appears in the second toolbar;
- open multiple projects and confirm tabs have more room than before;
- switch between projects and confirm selection clears and canvas view resets;
- close the active project and confirm the next active session behaves as
  before;
- confirm dirty markers remain visible;
- confirm tab close buttons remain accessible and clickable;
- verify no primary toolbar controls are clipped at wide desktop, around 940px,
  and a narrow desktop/mobile-ish viewport;
- verify dark, light, and high-contrast themes keep readable borders, focus
  rings, and active tab state.

## Roadmap

When implemented, mark the remaining GUI polish/project-tab-toolbar item as
complete. Keep the larger workspace framework as a future roadmap item.
