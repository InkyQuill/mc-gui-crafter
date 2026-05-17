<script lang="ts">
  import Toolbar from "./lib/components/Toolbar.svelte";
  import Canvas from "./lib/components/Canvas.svelte";
  import ElementPalette from "./lib/components/ElementPalette.svelte";
  import PropertyPanel from "./lib/components/PropertyPanel.svelte";
  import LayerPanel from "./lib/components/LayerPanel.svelte";
  import AssetLibrary from "./lib/components/AssetLibrary.svelte";
  import AnimationTimeline from "./lib/components/AnimationTimeline.svelte";
  import NewProjectDialog from "./lib/components/NewProjectDialog.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import StatusMessages from "./lib/components/StatusMessages.svelte";
  import StartPanel from "./lib/components/StartPanel.svelte";
  import { project } from "./lib/stores/project.svelte";
  import { editor } from "./lib/stores/editor.svelte";
  import { status, readableError } from "./lib/stores/status.svelte";
  import * as api from "./lib/api";

  let showNewDialog = $state(false);

  async function handleOpenProject() {
    const path = await api.showOpenDialog();
    if (path) {
      try {
        await project.openProject(path);
        editor.resetView();
        status.success("Project opened.");
      } catch (error) {
        status.error(`Failed to open project: ${readableError(error)}`);
      }
    }
  }

  // Listen for MCP project changes
  $effect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      await project.syncFromBackend();
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlisten = await listen<{ tool: string }>("project-changed", () => {
          project.syncFromBackend();
        });
      } catch { /* not in Tauri */ }
    })();
    return () => { unlisten?.(); };
  });
</script>

<div class="app">
  <Toolbar />
  <StatusMessages />

  <div class="workspace">
    <nav class="sidebar-left">
      <ElementPalette />
    </nav>

    <main class="canvas-area">
      {#if project.isOpen}
        <Canvas />
      {:else}
        <StartPanel onnew={() => showNewDialog = true} onopen={handleOpenProject} />
      {/if}
    </main>

    <aside class="sidebar-right">
      <PropertyPanel />
      <LayerPanel />
      <AssetLibrary />
    </aside>
  </div>

  <AnimationTimeline />
  <StatusBar />
</div>

{#if showNewDialog}
  <NewProjectDialog onclose={() => showNewDialog = false} />
{/if}

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  :global(body) {
    font-family: "Inter", system-ui, -apple-system, sans-serif;
    background: #1a1a2e;
    color: #e0e0e0;
    overflow: hidden;
    height: 100vh;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .workspace {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .sidebar-left,
  .sidebar-right {
    width: 220px;
    background: #1a1a2e;
    overflow-y: auto;
    flex-shrink: 0;
  }

  .sidebar-left {
    border-right: 1px solid #0f3460;
  }

  .sidebar-right {
    border-left: 1px solid #0f3460;
  }

  .canvas-area {
    flex: 1;
    position: relative;
    overflow: hidden;
  }
</style>
