<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import * as api from "../api";
  import NewProjectDialog from "./NewProjectDialog.svelte";
  import ExportDialog from "./ExportDialog.svelte";
  import ProjectTabs from "./ProjectTabs.svelte";
  import PreferencesDialog from "./PreferencesDialog.svelte";

  let showNewDialog = $state(false);
  let showExportDialog = $state(false);
  let showPreferencesDialog = $state(false);

  async function handleOpen() {
    const path = await api.showOpenDialog();
    if (path) {
      await project.openProject(path);
      editor.resetView();
    }
  }

  async function handleSave() {
    if (!project.projectPath) {
      const path = await api.showSaveDialog();
      if (path) {
        await project.saveProjectAs(path);
      } else {
        return;
      }
    } else {
      await project.saveProject();
    }
  }

  async function handleSaveAs() {
    const path = await api.showSaveDialog();
    if (path) {
      await project.saveProjectAs(path);
    }
  }

  async function handleUndo() { await project.undo(); }
  async function handleRedo() { await project.redo(); }
  function toggleGrid() { editor.showGrid = !editor.showGrid; }

  async function handleSwitchProject(projectId: string) {
    await project.switchProject(projectId);
    editor.clearSelection();
    editor.resetView();
  }

  async function handleCloseProject(projectId: string) {
    await project.closeProject(projectId);
    editor.clearSelection();
    editor.resetView();
  }
</script>

<header class="toolbar">
  <span class="logo">MCGUI Crafter</span>

  <div class="toolbar-group">
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

  <div class="toolbar-group">
    <button onclick={handleUndo} disabled={!project.canUndo} title="Undo">↩</button>
    <button onclick={handleRedo} disabled={!project.canRedo} title="Redo">↪</button>
  </div>

  <div class="toolbar-group">
    <button onclick={toggleGrid} class:active={editor.showGrid} title="Toggle grid">
      Grid
    </button>
    <button onclick={() => editor.zoomOut()} title="Zoom out">−</button>
    <span class="zoom-label">{editor.zoom}×</span>
    <button onclick={() => editor.zoomIn()} title="Zoom in">+</button>
    <button onclick={() => editor.resetView()} title="Reset view">⊡</button>
  </div>

  <div class="toolbar-group">
    <button
      class="icon-button"
      onclick={() => showPreferencesDialog = true}
      title="Preferences"
      aria-label="Open preferences"
    >
      ⚙
    </button>
  </div>

  <ProjectTabs
    sessions={project.sessions}
    activeProjectId={project.activeProjectId}
    onswitch={handleSwitchProject}
    onclose={handleCloseProject}
  />

  <span class="project-name">
    {project.isOpen ? project.name : "No project"}
  </span>
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

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 12px;
    background: #16213e;
    border-bottom: 1px solid #0f3460;
    height: 36px;
    flex-shrink: 0;
    user-select: none;
  }

  .logo {
    font-weight: 700;
    color: #e94560;
    font-size: 13px;
    margin-right: 8px;
  }

  .toolbar-group {
    display: flex;
    gap: 2px;
    padding: 0 8px;
    border-right: 1px solid #0f3460;
  }

  .toolbar-group:last-of-type {
    border-right: none;
  }

  button {
    background: transparent;
    border: 1px solid transparent;
    color: #a0a0b0;
    padding: 4px 10px;
    font-size: 12px;
    cursor: pointer;
    border-radius: 3px;
    font-family: inherit;
  }

  button:hover:not(:disabled) {
    background: #0f3460;
    color: #e0e0e0;
  }

  button:disabled {
    opacity: 0.3;
    cursor: default;
  }

  button.active {
    background: #0f3460;
    color: #e94560;
    border-color: #e94560;
  }

  .icon-button {
    width: 28px;
    padding: 4px 0;
    text-align: center;
  }

  .zoom-label {
    font-size: 11px;
    color: #606080;
    min-width: 28px;
    text-align: center;
    font-family: monospace;
  }

  .project-name {
    margin-left: 0;
    color: #a0a0b0;
    font-size: 12px;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
