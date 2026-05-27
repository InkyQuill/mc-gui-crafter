# GUI Polish Alpha Toolbar Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move open-project tabs from the crowded primary toolbar into a compact second toolbar row under the primary commands.

**Architecture:** Keep project session behavior unchanged. `Toolbar.svelte` remains the owner of command handlers and project tab callbacks, while `ProjectTabs.svelte` remains a presentational component. The change is a Svelte/CSS layout refactor plus roadmap bookkeeping.

**Tech Stack:** Svelte 5, TypeScript, Vite, existing CSS custom properties, existing project store/API.

---

## File Structure

- Modify `src/lib/components/Toolbar.svelte`
  - Split the top chrome into `.toolbar-shell`, `.toolbar-primary`, and optional `.toolbar-tabs-row`.
  - Keep all existing handlers in this component.
  - Move `ProjectTabs` out of the primary command row.
- Modify `src/lib/components/ProjectTabs.svelte`
  - Make tabs fill the dedicated row.
  - Change overflow behavior so many tabs scroll inside the row instead of shrinking the command toolbar.
  - Preserve existing props and callback contract.
- Modify `docs/roadmap.md`
  - Add a completed roadmap item for the remaining GUI polish project-tab toolbar once the implementation is verified.

Do not modify stores, API contracts, Tauri commands, or persisted layout config for this feature.

---

### Task 1: Split Toolbar Markup Into Primary And Tabs Rows

**Files:**
- Modify: `src/lib/components/Toolbar.svelte`

- [ ] **Step 1: Inspect the current toolbar markup**

Run:

```bash
sed -n '120,235p' src/lib/components/Toolbar.svelte
```

Expected: `ProjectTabs` is rendered inside `<header class="toolbar">` after the preferences button group, followed by `<span class="project-name">`.

- [ ] **Step 2: Replace the single-row toolbar markup**

In `src/lib/components/Toolbar.svelte`, replace the current block from:

```svelte
<header class="toolbar">
```

through:

```svelte
</header>
```

with this structure:

```svelte
<header class="toolbar-shell">
  <div class="toolbar-primary">
    <span class="logo">MCGUI Crafter</span>

    <div class="toolbar-group file-actions">
      <button onclick={() => showNewDialog = true} title="New project">New</button>
      <button onclick={handleOpen} title="Open .mcgui">Open</button>
      <button onclick={handleSave} disabled={!project.isOpen} title="Save project">
        Save{project.isDirty ? " *" : ""}
      </button>
      <button onclick={handleSaveAs} disabled={!project.isOpen} title="Save project as">
        Save As
      </button>
      <button onclick={() => showExportDialog = true} disabled={!project.isOpen} title="Export to mod loader code">
        Export
      </button>
    </div>

    <div class="toolbar-group icon-actions">
      <button class="icon-button" onclick={handleUndo} disabled={!project.canUndo} title="Undo" aria-label="Undo">↩</button>
      <button class="icon-button" onclick={handleRedo} disabled={!project.canRedo} title="Redo" aria-label="Redo">↪</button>
    </div>

    <div class="toolbar-group">
      <button onclick={toggleGrid} class:active={editor.showGrid} title="Toggle grid">
        Grid
      </button>
      <button class="icon-button" onclick={() => editor.zoomOut()} title="Zoom out" aria-label="Zoom out">−</button>
      <span class="zoom-label">{editor.zoom}×</span>
      <button class="icon-button" onclick={() => editor.zoomIn()} title="Zoom in" aria-label="Zoom in">+</button>
      <button class="icon-button" onclick={() => editor.resetView(project.guiSize)} title="Reset view" aria-label="Reset view">⊡</button>
    </div>

    <div class="toolbar-group state-toolbar">
      <select
        aria-label="Active state variant"
        disabled={!project.isOpen}
        value={project.activeStateId ?? ""}
        onchange={handleToolbarStateChange}
        title="Active state variant"
      >
        <option value="">Base</option>
        {#each project.states as state (state.id)}
          <option value={state.id}>{state.label || state.id}</option>
        {/each}
      </select>
      <div class="scope-toggle" aria-label="Edit scope">
        <button
          class:active={project.editScope === "base"}
          disabled={!project.isOpen}
          onclick={() => handleToolbarScope("base")}
          title="Edit base layout"
          aria-label="Edit base layout"
          aria-pressed={project.editScope === "base"}
        >
          <span class="scope-full">Base</span>
          <span class="scope-short">B</span>
        </button>
        <button
          class:active={project.editScope === "state"}
          disabled={!project.isOpen || !project.activeStateId}
          onclick={() => handleToolbarScope("state")}
          title="Edit active state overrides"
          aria-label="Edit active state overrides"
          aria-pressed={project.editScope === "state"}
        >
          <span class="scope-full">State</span>
          <span class="scope-short">S</span>
        </button>
      </div>
    </div>

    <div class="toolbar-group utility-actions">
      <button
        class="icon-button"
        onclick={() => showShortcutsDialog = true}
        title="Keyboard shortcuts (?)"
        aria-label="Open keyboard shortcuts"
      >
        ?
      </button>
      <button
        class="icon-button"
        onclick={() => showPreferencesDialog = true}
        title="Preferences"
        aria-label="Open preferences"
      >
        ⚙
      </button>
    </div>
  </div>

  {#if project.sessions.length > 0}
    <div class="toolbar-tabs-row">
      <ProjectTabs
        sessions={project.sessions}
        activeProjectId={project.activeProjectId}
        onswitch={handleSwitchProject}
        onclose={handleCloseProject}
      />
      <span class="project-name">
        {project.isOpen ? project.name : "No project"}
      </span>
    </div>
  {/if}
</header>
```

- [ ] **Step 3: Run Svelte autofixer on Toolbar**

Run:

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/Toolbar.svelte --svelte-version 5
```

Expected: no syntax issues. If it reports a concrete issue, fix the exact file and rerun the same command.

- [ ] **Step 4: Run type check**

Run:

```bash
pnpm check
```

Expected: `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add src/lib/components/Toolbar.svelte
git commit -m "feat: split project tabs into toolbar row"
```

Expected: commit succeeds and includes only `src/lib/components/Toolbar.svelte`.

---

### Task 2: Update Toolbar And ProjectTabs Styling

**Files:**
- Modify: `src/lib/components/Toolbar.svelte`
- Modify: `src/lib/components/ProjectTabs.svelte`

- [ ] **Step 1: Replace Toolbar top-chrome CSS**

In `src/lib/components/Toolbar.svelte`, replace the `.toolbar { ... }` rule with:

```css
.toolbar-shell {
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  min-width: 0;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  user-select: none;
}

.toolbar-primary {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 12px;
  height: 36px;
  min-width: 0;
  overflow: hidden;
}

.toolbar-tabs-row {
  display: flex;
  align-items: stretch;
  gap: 8px;
  min-width: 0;
  height: 30px;
  padding: 0 12px;
  border-top: 1px solid var(--border);
  background: var(--surface);
}
```

- [ ] **Step 2: Update toolbar group border rule**

In `src/lib/components/Toolbar.svelte`, replace:

```css
.toolbar-group:last-of-type {
  border-right: none;
}
```

with:

```css
.utility-actions {
  border-right: none;
  margin-left: auto;
}
```

This keeps utility buttons right-aligned in the primary row after tabs move out.

- [ ] **Step 3: Update project name CSS**

In `src/lib/components/Toolbar.svelte`, replace the `.project-name` rule with:

```css
.project-name {
  align-self: center;
  color: var(--muted-text);
  font-size: 12px;
  max-width: min(240px, 28vw);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 0 1 auto;
  min-width: 0;
}
```

Keep the existing `@media (max-width: 1120px)` rule that hides `.project-name`.

- [ ] **Step 4: Update responsive toolbar selector**

In `src/lib/components/Toolbar.svelte`, inside the `@media (max-width: 940px)` block, replace:

```css
.toolbar {
  gap: 6px;
  padding-inline: 8px;
}
```

with:

```css
.toolbar-primary {
  gap: 6px;
  padding-inline: 8px;
}

.toolbar-tabs-row {
  padding-inline: 8px;
}
```

- [ ] **Step 5: Update ProjectTabs row CSS**

In `src/lib/components/ProjectTabs.svelte`, replace the `.project-tabs` rule with:

```css
.project-tabs {
  display: flex;
  align-items: stretch;
  flex: 1 1 auto;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: thin;
  border-left: 1px solid var(--border);
  border-right: 1px solid var(--border);
}
```

- [ ] **Step 6: Update tab sizing CSS**

In `src/lib/components/ProjectTabs.svelte`, replace the `.tab` rule with:

```css
.tab {
  display: flex;
  align-items: center;
  min-width: 112px;
  max-width: 220px;
  flex: 1 0 148px;
  border-right: 1px solid var(--border);
  background: var(--surface);
}
```

- [ ] **Step 7: Add a narrow-width ProjectTabs media rule**

At the end of `src/lib/components/ProjectTabs.svelte` style block, before `</style>`, add:

```css
@media (max-width: 940px) {
  .tab {
    min-width: 96px;
    flex-basis: 120px;
  }
}
```

- [ ] **Step 8: Run Svelte autofixer on both components**

Run:

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/Toolbar.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/ProjectTabs.svelte --svelte-version 5
```

Expected: no syntax issues. If either command reports a concrete issue, fix that file and rerun both commands.

- [ ] **Step 9: Run checks**

Run:

```bash
pnpm check
pnpm run build
git diff --check
```

Expected:

- `svelte-check found 0 errors and 0 warnings`;
- Vite build exits 0; the existing large chunk warning is acceptable;
- `git diff --check` prints no output.

- [ ] **Step 10: Commit Task 2**

Run:

```bash
git add src/lib/components/Toolbar.svelte src/lib/components/ProjectTabs.svelte
git commit -m "fix: improve project tab toolbar overflow"
```

Expected: commit succeeds and includes only the two Svelte components.

---

### Task 3: Verify Behavior And Update Roadmap

**Files:**
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Start the dev server for visual checks**

Run:

```bash
pnpm run dev -- --host 127.0.0.1
```

Expected: Vite prints a local URL such as `http://127.0.0.1:5173/`. Keep this command running while performing the manual checks.

- [ ] **Step 2: Perform manual checks**

Use the dev server URL and check these behaviors:

- with no project open, there is no visible project-tabs row;
- after creating or opening one project, a compact second toolbar row appears;
- with multiple open projects, tabs occupy the second row and primary command controls remain visible;
- switching a tab clears selection and resets the canvas view;
- closing the active tab selects the next available session as before;
- dirty markers and close buttons remain visible on tabs;
- focus outlines are visible on tab buttons and close buttons;
- at wide desktop, around 940px, and a narrow desktop/mobile-ish viewport, primary toolbar controls do not overlap or disappear because of tabs;
- dark, light, and high-contrast themes keep readable tab borders and active tab state.

- [ ] **Step 3: Stop the dev server**

Stop the running Vite process with `Ctrl+C`.

Expected: no dev server remains running.

- [ ] **Step 4: Update roadmap**

In `docs/roadmap.md`, near the existing checked `Editor UX polish` item, add this checked item:

```markdown
- [x] GUI polish alpha top chrome: project tabs moved to a dedicated second toolbar row with preserved switching, closing, dirty markers, and overflow behavior
```

Keep the existing future `Workspace/dock framework` item open.

- [ ] **Step 5: Final verification**

Run:

```bash
pnpm check
pnpm run build
git diff --check
```

Expected:

- `svelte-check found 0 errors and 0 warnings`;
- Vite build exits 0; the existing large chunk warning is acceptable;
- `git diff --check` prints no output.

- [ ] **Step 6: Commit Task 3**

Run:

```bash
git add docs/roadmap.md
git commit -m "docs: mark gui polish toolbar complete"
```

Expected: commit succeeds and includes only `docs/roadmap.md`.

---

## Final Review Checklist

- [ ] `src/lib/components/Toolbar.svelte` has two top chrome rows: primary commands and project tabs.
- [ ] `ProjectTabs` is not rendered inside the primary command row.
- [ ] `ProjectTabs` props and callback names are unchanged.
- [ ] The second toolbar row is hidden when `project.sessions.length === 0`.
- [ ] Tab overflow is contained inside the second row.
- [ ] Primary command controls stay visible at the existing responsive breakpoint.
- [ ] Accessibility labels, `aria-current`, close-button labels, and focus outlines are preserved.
- [ ] `docs/roadmap.md` marks only the top-chrome GUI polish item complete and leaves the workspace framework item open.
- [ ] Svelte autofixer was run on both changed `.svelte` files.
- [ ] `pnpm check`, `pnpm run build`, and `git diff --check` pass.
