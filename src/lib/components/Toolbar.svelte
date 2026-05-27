<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import { status, readableError } from "../stores/status.svelte";
  import * as api from "../api";
  import NewProjectDialog from "./NewProjectDialog.svelte";
  import ExportDialog from "./ExportDialog.svelte";
  import ProjectTabs from "./ProjectTabs.svelte";
  import PreferencesDialog from "./PreferencesDialog.svelte";
  import ShortcutsDialog from "./ShortcutsDialog.svelte";

  let showNewDialog = $state(false);
  let showExportDialog = $state(false);
  let showPreferencesDialog = $state(false);
  let showShortcutsDialog = $state(false);

  async function handleOpen() {
    const path = await api.showOpenDialog();
    if (path) {
      try {
        await project.openProject(path);
        editor.resetView(project.guiSize);
        status.success("Project opened.");
      } catch (error) {
        status.error(`Failed to open project: ${readableError(error)}`);
      }
    }
  }

  async function handleSave() {
    if (!project.projectPath) {
      const path = await api.showSaveDialog();
      if (path) {
        try {
          await project.saveProjectAs(path);
          status.success("Project saved.");
        } catch (error) {
          status.error(`Failed to save project: ${readableError(error)}`);
        }
      } else {
        return;
      }
    } else {
      try {
        await project.saveProject();
        status.success("Project saved.");
      } catch (error) {
        status.error(`Failed to save project: ${readableError(error)}`);
      }
    }
  }

  async function handleSaveAs() {
    const path = await api.showSaveDialog();
    if (path) {
      try {
        await project.saveProjectAs(path);
        status.success("Project saved as.");
      } catch (error) {
        status.error(`Failed to save project as: ${readableError(error)}`);
      }
    }
  }

  async function handleUndo() { await project.undo(); }
  async function handleRedo() { await project.redo(); }
  function toggleGrid() { editor.showGrid = !editor.showGrid; }

  async function handleSwitchProject(projectId: string) {
    await project.switchProject(projectId);
    editor.clearSelection();
    editor.resetView(project.guiSize);
  }

  async function handleCloseProject(projectId: string) {
    await project.closeProject(projectId);
    editor.clearSelection();
    editor.resetView(project.guiSize);
  }

  async function handleToolbarStateChange(event: Event) {
    const stateId = (event.currentTarget as HTMLSelectElement).value || null;
    await project.setActiveState(stateId, stateId ? project.editScope : "base");
  }

  async function handleToolbarScope(scope: "base" | "state") {
    await project.setEditScope(scope);
  }

  function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;

    return target instanceof HTMLInputElement
      || target instanceof HTMLSelectElement
      || target instanceof HTMLTextAreaElement
      || target.isContentEditable
      || target.closest("[contenteditable='true']") !== null;
  }

  function isDialogTarget(target: EventTarget | null): boolean {
    return target instanceof HTMLElement && target.closest('[role="dialog"]') !== null;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (
      event.key !== "?"
      || event.ctrlKey
      || event.metaKey
      || event.altKey
      || showShortcutsDialog
      || showNewDialog
      || showExportDialog
      || showPreferencesDialog
      || isEditableTarget(event.target)
      || isDialogTarget(event.target)
    ) {
      return;
    }

    event.preventDefault();
    showShortcutsDialog = true;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

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

{#if showNewDialog}
  <NewProjectDialog onclose={() => showNewDialog = false} />
{/if}

{#if showExportDialog}
  <ExportDialog onclose={() => showExportDialog = false} />
{/if}

{#if showPreferencesDialog}
  <PreferencesDialog onclose={() => showPreferencesDialog = false} />
{/if}

{#if showShortcutsDialog}
  <ShortcutsDialog onclose={() => showShortcutsDialog = false} />
{/if}

<style>
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

  .logo {
    font-weight: 700;
    color: var(--accent);
    font-size: 13px;
    margin-right: 4px;
    flex: 0 0 auto;
    white-space: nowrap;
  }

  .toolbar-group {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 6px;
    border-right: 1px solid var(--border);
    flex: 0 0 auto;
    min-width: 0;
  }

  .utility-actions {
    border-right: none;
    margin-left: auto;
  }

  .file-actions {
    max-width: clamp(220px, 34vw, 390px);
    overflow-x: auto;
    scrollbar-width: none;
  }

  .file-actions::-webkit-scrollbar {
    display: none;
  }

  .icon-actions {
    gap: 3px;
  }

  button {
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted-text);
    padding: 3px 10px;
    font-size: 12px;
    cursor: pointer;
    border-radius: 3px;
    font-family: inherit;
    line-height: 1;
    white-space: nowrap;
    flex: 0 0 auto;
  }

  button:hover:not(:disabled) {
    background: var(--surface-raised);
    color: var(--text);
  }

  button:disabled {
    opacity: 0.3;
    cursor: default;
  }

  button.active {
    background: var(--surface-raised);
    color: var(--accent);
    border-color: var(--accent);
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .icon-button {
    width: 28px;
    height: 24px;
    padding: 0;
    text-align: center;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .zoom-label {
    font-size: 11px;
    color: var(--muted-text);
    min-width: 28px;
    text-align: center;
    font-family: monospace;
    flex: 0 0 28px;
  }

  .state-toolbar {
    gap: 5px;
    max-width: clamp(190px, 22vw, 300px);
    overflow: hidden;
  }

  .state-toolbar select {
    flex: 1 1 82px;
    min-width: 82px;
    max-width: 140px;
    height: 24px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--surface-raised);
    color: var(--text);
    font: inherit;
    font-size: 12px;
    padding: 2px 22px 2px 7px;
  }

  .state-toolbar select:disabled {
    opacity: 0.35;
  }

  .scope-toggle {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
  }

  .scope-toggle button {
    border: 0;
    border-radius: 0;
    padding-inline: 7px;
    height: 22px;
  }

  .scope-short {
    display: none;
  }

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

  @media (max-width: 1120px) {
    .project-name {
      display: none;
    }
  }

  @media (max-width: 940px) {
    .toolbar-primary {
      gap: 6px;
      padding-inline: 8px;
    }

    .toolbar-tabs-row {
      padding-inline: 8px;
    }

    .logo {
      max-width: 112px;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .toolbar-group {
      padding-inline: 4px;
    }

    .file-actions {
      max-width: 260px;
    }

    .state-toolbar {
      gap: 3px;
      max-width: 132px;
    }

    .state-toolbar select {
      flex-basis: 72px;
      min-width: 64px;
      max-width: 76px;
      padding-inline: 5px 16px;
    }

    .scope-toggle button {
      width: 24px;
      padding-inline: 0;
    }

    .scope-full {
      display: none;
    }

    .scope-short {
      display: inline;
    }
  }
</style>
