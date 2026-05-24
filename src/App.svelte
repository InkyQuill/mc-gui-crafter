<script lang="ts">
  import Toolbar from "./lib/components/Toolbar.svelte";
  import Canvas from "./lib/components/Canvas.svelte";
  import ElementPalette from "./lib/components/ElementPalette.svelte";
  import InspectorDock from "./lib/components/InspectorDock.svelte";
  import AnimationTimeline from "./lib/components/AnimationTimeline.svelte";
  import NewProjectDialog from "./lib/components/NewProjectDialog.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import StatusMessages from "./lib/components/StatusMessages.svelte";
  import StartPanel from "./lib/components/StartPanel.svelte";
  import { project } from "./lib/stores/project.svelte";
  import { editor } from "./lib/stores/editor.svelte";
  import { layout } from "./lib/stores/layout.svelte";
  import { status, readableError } from "./lib/stores/status.svelte";
  import * as api from "./lib/api";

  let showNewDialog = $state(false);

  async function handleOpenProject() {
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

  // Listen for MCP project changes
  $effect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      await layout.load();
      await project.syncFromBackend();
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const unlistenProjectChanged = await listen<{ tool: string }>("project-changed", () => {
          project.syncFromBackend();
        });
        const unlistenProjectOpenFailed = await listen<{ path: string; error: string }>(
          "project-open-failed",
          (event) => {
            status.error(`Failed to open project ${event.payload.path}: ${event.payload.error}`);
          }
        );
        unlisten = () => {
          unlistenProjectChanged();
          unlistenProjectOpenFailed();
        };
      } catch { /* not in Tauri */ }
    })();
    return () => { unlisten?.(); };
  });

  $effect(() => {
    function handleKeydown(event: KeyboardEvent) {
      const key = event.key.toLowerCase();
      if (key === "r" && event.ctrlKey && event.shiftKey && event.altKey) {
        event.preventDefault();
        void layout.reset();
        status.success("UI layout reset.");
        return;
      }
      if (key === "r" && event.ctrlKey && !event.shiftKey && !event.altKey) {
        event.preventDefault();
        editor.resetView(project.guiSize);
        status.success("Canvas view reset.");
      }
    }
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
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

    <InspectorDock />
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

  :global(:root[data-theme="dark"]) {
    color-scheme: dark;
    --app-bg: #101214;
    --surface: #1f2326;
    --surface-raised: #2d3033;
    --border: #08090a;
    --text: #f2f2f2;
    --muted-text: #b8b8b8;
    --accent: #3aa655;
    --accent-2: #3f76b5;
    --danger: #b83a32;
    --warning: #d7a339;
  }

  :global(:root[data-theme="light"]) {
    color-scheme: light;
    --app-bg: #9f9f9f;
    --surface: #c6c6c6;
    --surface-raised: #d8d8d8;
    --border: #4a4a4a;
    --text: #202020;
    --muted-text: #505050;
    --accent: #2f8f46;
    --accent-2: #3f76b5;
    --danger: #9f3028;
    --warning: #b98525;
  }

  :global(:root[data-theme="high_contrast"]) {
    color-scheme: dark;
    --app-bg: #000000;
    --surface: #000000;
    --surface-raised: #111111;
    --border: #ffffff;
    --text: #ffffff;
    --muted-text: #ffffff;
    --accent: #00ffff;
    --accent-2: #ffff00;
    --danger: #ff5555;
    --warning: #ffff00;
  }

  :global(body) {
    font-family: "Inter", system-ui, -apple-system, sans-serif;
    background: var(--app-bg);
    color: var(--text);
    overflow: hidden;
    height: 100vh;
  }

  :global(select),
  :global(option) {
    background-color: var(--app-bg);
    color: var(--text);
  }

  :global(select) {
    border-color: var(--border);
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

  .sidebar-left {
    width: 220px;
    background: var(--surface);
    overflow-y: auto;
    flex-shrink: 0;
  }

  .sidebar-left {
    border-right: 1px solid var(--border);
  }

  .canvas-area {
    flex: 1;
    position: relative;
    overflow: hidden;
  }
</style>
