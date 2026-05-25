<script lang="ts">
  import { project } from "../stores/project.svelte";
  import type { UvRect } from "../types";

  let {
    title,
    assets,
    asset,
    uv = null,
    onapply,
    onclear,
    onclose,
  }: {
    title: string;
    assets: string[];
    asset: string | null;
    uv?: UvRect | null;
    onapply: (asset: string, uv: UvRect | null) => void;
    onclear: () => void;
    onclose: () => void;
  } = $props();

  let initialized = $state(false);
  let selectedAsset = $state("");
  let rect = $state<UvRect>({
    x: 0,
    y: 0,
    width: 16,
    height: 16,
  });
  let dragStart: { x: number; y: number } | null = null;
  let imageWrapEl: HTMLDivElement | undefined = $state();
  let imageNaturalWidth = $state(1);
  let imageNaturalHeight = $state(1);
  let zoom = $state(4);

  let dataUrl = $derived(selectedAsset ? project.getAssetDataUrl(selectedAsset) : undefined);

  $effect(() => {
    if (initialized) return;
    selectedAsset = asset ?? assets[0] ?? "";
    rect = {
      x: uv?.x ?? 0,
      y: uv?.y ?? 0,
      width: uv?.width ?? 16,
      height: uv?.height ?? 16,
    };
    initialized = true;
  });

  function clampRect(next: UvRect): UvRect {
    const maxX = Math.max(0, imageNaturalWidth - 1);
    const maxY = Math.max(0, imageNaturalHeight - 1);
    const x = Math.max(0, Math.min(maxX, Math.round(next.x)));
    const y = Math.max(0, Math.min(maxY, Math.round(next.y)));
    const width = Math.max(1, Math.min(imageNaturalWidth - x, Math.round(next.width)));
    const height = Math.max(1, Math.min(imageNaturalHeight - y, Math.round(next.height)));
    return { x, y, width, height };
  }

  function updateRect(changes: Partial<UvRect>) {
    rect = clampRect({ ...rect, ...changes });
  }

  function imagePoint(event: PointerEvent): { x: number; y: number } {
    if (!imageWrapEl) return { x: 0, y: 0 };
    const bounds = imageWrapEl.getBoundingClientRect();
    return {
      x: Math.floor((event.clientX - bounds.left) / zoom),
      y: Math.floor((event.clientY - bounds.top) / zoom),
    };
  }

  function startDrag(event: PointerEvent) {
    event.preventDefault();
    dragStart = imagePoint(event);
    updateRect({ x: dragStart.x, y: dragStart.y, width: 1, height: 1 });
    window.addEventListener("pointermove", drag);
    window.addEventListener("pointerup", stopDrag, { once: true });
  }

  function drag(event: PointerEvent) {
    if (!dragStart) return;
    const current = imagePoint(event);
    updateRect({
      x: Math.min(dragStart.x, current.x),
      y: Math.min(dragStart.y, current.y),
      width: Math.abs(current.x - dragStart.x) + 1,
      height: Math.abs(current.y - dragStart.y) + 1,
    });
  }

  function stopDrag() {
    dragStart = null;
    window.removeEventListener("pointermove", drag);
  }

  function apply() {
    if (!selectedAsset) return;
    onapply(selectedAsset, clampRect(rect));
  }
</script>

<div class="uv-overlay" role="presentation" onclick={(event) => event.target === event.currentTarget && onclose()}>
  <div class="uv-dialog" role="dialog" aria-modal="true" aria-labelledby="uv-editor-title">
    <header>
      <h2 id="uv-editor-title">{title}</h2>
      <button type="button" onclick={onclose} aria-label="Close">x</button>
    </header>

    <div class="uv-controls">
      <label>
        Asset
        <select bind:value={selectedAsset}>
          {#each assets as name (name)}
            <option value={name}>{name}</option>
          {/each}
        </select>
      </label>
      <label>
        Zoom
        <input type="range" min="1" max="12" bind:value={zoom} />
      </label>
    </div>

    <div class="uv-body">
      {#if dataUrl}
        <div class="image-stage">
          <div
            bind:this={imageWrapEl}
            class="image-wrap"
            role="application"
            aria-label="UV selection canvas"
            style={`width:${imageNaturalWidth * zoom}px;height:${imageNaturalHeight * zoom}px`}
            onpointerdown={startDrag}
          >
            <img
              src={dataUrl}
              alt={selectedAsset}
              style={`width:${imageNaturalWidth * zoom}px;height:${imageNaturalHeight * zoom}px`}
              onload={(event) => {
                const image = event.currentTarget as HTMLImageElement;
                imageNaturalWidth = image.naturalWidth;
                imageNaturalHeight = image.naturalHeight;
                rect = clampRect(rect);
              }}
            />
            <div
              class="selection"
              style={`left:${rect.x * zoom}px;top:${rect.y * zoom}px;width:${rect.width * zoom}px;height:${rect.height * zoom}px`}
            ></div>
          </div>
        </div>
      {:else}
        <div class="missing-preview">No preview data for selected asset.</div>
      {/if}

      <div class="numeric-grid">
        <label>X <input type="number" min="0" value={rect.x} oninput={(event) => updateRect({ x: Number(event.currentTarget.value) })} /></label>
        <label>Y <input type="number" min="0" value={rect.y} oninput={(event) => updateRect({ y: Number(event.currentTarget.value) })} /></label>
        <label>W <input type="number" min="1" value={rect.width} oninput={(event) => updateRect({ width: Number(event.currentTarget.value) })} /></label>
        <label>H <input type="number" min="1" value={rect.height} oninput={(event) => updateRect({ height: Number(event.currentTarget.value) })} /></label>
      </div>
    </div>

    <footer>
      <button type="button" onclick={onclear}>Clear</button>
      <button type="button" onclick={onclose}>Cancel</button>
      <button type="button" class="primary" onclick={apply} disabled={!selectedAsset}>Apply</button>
    </footer>
  </div>
</div>

<style>
  .uv-overlay {
    position: fixed;
    inset: 0;
    z-index: 1200;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
  }

  .uv-dialog {
    width: min(860px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 10px;
    border-bottom: 1px solid var(--border);
  }

  footer {
    border-top: 1px solid var(--border);
    border-bottom: 0;
    justify-content: flex-end;
  }

  h2 {
    font-size: 13px;
    margin: 0;
  }

  .uv-controls {
    display: grid;
    grid-template-columns: 1fr 160px;
    gap: 10px;
    padding: 10px;
    border-bottom: 1px solid var(--border);
  }

  .uv-body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 170px;
    gap: 10px;
    padding: 10px;
    min-height: 0;
    overflow: hidden;
  }

  .image-stage {
    overflow: auto;
    background: var(--app-bg);
    border: 1px solid var(--border);
    min-height: 340px;
  }

  .image-wrap {
    position: relative;
    image-rendering: pixelated;
  }

  img {
    display: block;
    image-rendering: pixelated;
    user-select: none;
    pointer-events: none;
  }

  .selection {
    position: absolute;
    border: 1px solid var(--accent);
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    pointer-events: none;
  }

  .numeric-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 8px;
    align-content: start;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--muted-text);
    font-size: 11px;
  }

  input,
  select,
  button {
    font: inherit;
  }

  input,
  select {
    background: var(--app-bg);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 4px 6px;
  }

  button {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text);
    padding: 5px 9px;
    cursor: pointer;
  }

  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .missing-preview {
    color: var(--muted-text);
    padding: 16px;
  }
</style>
