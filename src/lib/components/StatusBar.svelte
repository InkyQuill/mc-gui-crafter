<script lang="ts">
  import { editor } from "../stores/editor.svelte";
  import { project } from "../stores/project.svelte";
  import { preferences } from "../stores/preferences.svelte";

  let coordText = $derived(
    editor.mouseGuiX >= 0 && editor.mouseGuiY >= 0
      ? `${editor.mouseGuiX}, ${editor.mouseGuiY}`
      : "—"
  );

  let selInfo = $derived(
    editor.selectedElementId
      ? `Sel: ${editor.selectedElementId}`
      : ""
  );

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
  <span class="tool">{editor.tool}</span>
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
    background: #16213e;
    border-top: 1px solid #0f3460;
    height: 24px;
    flex-shrink: 0;
    font-size: 11px;
    color: #606080;
    user-select: none;
  }

  .coord {
    color: #e94560;
    font-family: monospace;
    min-width: 60px;
  }

  .tool {
    color: #808090;
    text-transform: capitalize;
    min-width: 50px;
  }

  .zoom {
    font-family: monospace;
  }

  .grid,
  .snap {
    font-family: monospace;
    color: #70708c;
  }

  .dirty {
    color: #e9a23b;
  }

  .sel-info {
    margin-left: auto;
    color: #505060;
    font-family: monospace;
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
