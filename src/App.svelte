<script lang="ts">
  import Toolbar from "./lib/components/Toolbar.svelte";
  import Canvas from "./lib/components/Canvas.svelte";
  import ElementPalette from "./lib/components/ElementPalette.svelte";
  import PropertyPanel from "./lib/components/PropertyPanel.svelte";
  import LayerPanel from "./lib/components/LayerPanel.svelte";
  import AssetLibrary from "./lib/components/AssetLibrary.svelte";
  import AnimationTimeline from "./lib/components/AnimationTimeline.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import { project } from "./lib/stores/project.svelte";

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

  <div class="workspace">
    <nav class="sidebar-left">
      <ElementPalette />
    </nav>

    <main class="canvas-area">
      <Canvas />
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
