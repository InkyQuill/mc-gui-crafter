<script lang="ts">
  import PropertyPanel from "./PropertyPanel.svelte";
  import LayerPanel from "./LayerPanel.svelte";
  import AssetLibrary from "./AssetLibrary.svelte";
  import { layout } from "../stores/layout.svelte";
  import type { BrowserTab } from "../types";

  let resizing = $state<"dock" | "properties" | null>(null);
  let startX = 0;
  let startRightWidth = 0;
  let startPropertiesWidth = 0;

  function startResize(kind: "dock" | "properties", event: PointerEvent) {
    resizing = kind;
    startX = event.clientX;
    startRightWidth = layout.values.right_dock_width;
    startPropertiesWidth = layout.values.properties_width;
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", stopResize, { once: true });
  }

  function handlePointerMove(event: PointerEvent) {
    if (resizing === "dock") {
      layout.update({ right_dock_width: startRightWidth - (event.clientX - startX) });
    } else if (resizing === "properties") {
      layout.update({ properties_width: startPropertiesWidth + (event.clientX - startX) });
    }
  }

  function stopResize() {
    resizing = null;
    window.removeEventListener("pointermove", handlePointerMove);
  }

  function setTab(tab: BrowserTab) {
    layout.update({ browser_tab: tab });
  }
</script>

<aside
  class="inspector-dock"
  style={`width: ${layout.values.right_dock_width}px; grid-template-columns: ${layout.values.properties_width}px 1fr; --properties-width: ${layout.values.properties_width}px;`}
>
  <button
    type="button"
    class="dock-resizer dock-resizer-outer"
    aria-label="Resize inspector dock"
    onpointerdown={(event) => startResize("dock", event)}
  ></button>
  <section class="dock-pane properties-pane">
    <PropertyPanel />
  </section>
  <button
    type="button"
    class="dock-resizer dock-resizer-inner"
    aria-label="Resize properties pane"
    onpointerdown={(event) => startResize("properties", event)}
  ></button>
  <section class="dock-pane browser-pane">
    <div class="browser-tabs" role="tablist" aria-label="Editor browsers">
      <button class:active={layout.values.browser_tab === "layers"} onclick={() => setTab("layers")}>Layers</button>
      <button class:active={layout.values.browser_tab === "assets"} onclick={() => setTab("assets")}>Assets</button>
    </div>
    <div class="browser-content">
      {#if layout.values.browser_tab === "layers"}
        <LayerPanel />
      {:else}
        <AssetLibrary />
      {/if}
    </div>
  </section>
</aside>

<style>
  .inspector-dock {
    position: relative;
    display: grid;
    flex-shrink: 0;
    min-width: 360px;
    max-width: 900px;
    height: 100%;
    background: var(--surface);
    border-left: 1px solid var(--border);
    overflow: hidden;
  }

  .dock-pane {
    min-width: 0;
    overflow: auto;
  }

  .properties-pane {
    border-right: 1px solid var(--border);
  }

  .browser-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .browser-tabs {
    display: flex;
    border-bottom: 1px solid var(--border);
  }

  .browser-tabs button {
    flex: 1;
    background: transparent;
    border: 0;
    color: var(--muted-text);
    padding: 8px 10px;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .browser-tabs button.active {
    background: var(--surface-raised);
    color: var(--text);
  }

  .browser-content {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .dock-resizer {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 6px;
    padding: 0;
    background: transparent;
    border: 0;
    cursor: col-resize;
    z-index: 5;
  }

  .dock-resizer-outer {
    left: -3px;
  }

  .dock-resizer-inner {
    left: calc(var(--properties-width, 300px) - 3px);
  }
</style>
