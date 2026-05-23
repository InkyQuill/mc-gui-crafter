<script lang="ts">
  import { editor } from "../stores/editor.svelte";
  import { project } from "../stores/project.svelte";
  import { preferences } from "../stores/preferences.svelte";

  let coordText = $derived(
    editor.mouseGuiX >= 0 && editor.mouseGuiY >= 0
      ? `${editor.mouseGuiX}, ${editor.mouseGuiY}`
      : "—"
  );

  let activeTool = $derived.by(() => {
    void editor.toolRevision;
    return editor.tool;
  });

  let selInfo = $derived.by(() => {
    void editor.selectionRevision;
    return editor.selectedElementId
      ? `Sel: ${editor.selectedElementId}`
      : ""
  });

  let gridInfo = $derived(
    preferences.values.showGrid
      ? `Grid ${Math.max(1, preferences.values.minorGridSize)}/${Math.max(1, preferences.values.majorGridSize)}`
      : "Grid off"
  );

  let snapInfo = $derived(
    preferences.values.snapToGrid
      ? `Snap ${Math.max(1, preferences.values.snapSize)}`
      : "Snap off"
  );
</script>

<footer class="statusbar">
  <span class="tool">{activeTool}</span>
  <span class="coord">{coordText}</span>
  <span class="zoom">Zoom: {editor.zoom}×</span>
  <span class="grid">{gridInfo}</span>
  <span class="snap">{snapInfo}</span>
  <span class="dirty">{project.isDirty ? "● modified" : ""}</span>
  <span class="sel-info">{selInfo}</span>
</footer>

<style>
  .statusbar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 3px 12px;
    background: var(--surface);
    border-top: 1px solid var(--border);
    height: 24px;
    flex-shrink: 0;
    font-size: 11px;
    color: var(--muted-text);
    user-select: none;
  }

  .coord {
    color: var(--accent);
    font-family: monospace;
    min-width: 60px;
  }

  .tool {
    color: var(--muted-text);
    text-transform: capitalize;
    min-width: 50px;
  }

  .zoom {
    font-family: monospace;
  }

  .grid,
  .snap {
    font-family: monospace;
    color: var(--muted-text);
  }

  .dirty {
    color: var(--warning);
  }

  .sel-info {
    margin-left: auto;
    color: var(--muted-text);
    font-family: monospace;
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
