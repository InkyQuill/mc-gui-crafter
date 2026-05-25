<script lang="ts">
  import { project } from "../stores/project.svelte";
  import type { NineSlice, NineSliceMode, Size, UvRect } from "../types";

  type EditorMode = "uv" | "nine_slice";
  type GuideHandle = "left" | "right" | "top" | "bottom";

  const DEFAULT_NINE_SLICE: NineSlice = {
    left: 4,
    right: 4,
    top: 4,
    bottom: 4,
    edge_mode: "tile",
    center_mode: "tile",
  };

  let {
    title,
    mode = "uv",
    assets,
    asset,
    uv = null,
    nineSlice = null,
    targetSize = null,
    onapply,
    onclear,
    onclose,
  }: {
    title: string;
    mode?: EditorMode;
    assets: string[];
    asset: string | null;
    uv?: UvRect | null;
    nineSlice?: NineSlice | null;
    targetSize?: Size | null;
    onapply: (asset: string, value: UvRect | NineSlice | null) => void;
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
  let guide = $state<NineSlice>({ ...DEFAULT_NINE_SLICE });
  let dragStart: { x: number; y: number } | null = null;
  let guideDrag = $state<GuideHandle | null>(null);
  let imageWrapEl: HTMLDivElement | undefined = $state();
  let imageNaturalWidth = $state(1);
  let imageNaturalHeight = $state(1);
  let hasImageDimensions = $state(false);
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
    guide = clampNineSlice(nineSlice ?? DEFAULT_NINE_SLICE);
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

  function clampNumber(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, Math.round(value)));
  }

  function clampNineSlice(next: NineSlice): NineSlice {
    if (!hasImageDimensions) return { ...next };
    const width = Math.max(1, imageNaturalWidth);
    const height = Math.max(1, imageNaturalHeight);
    const left = clampNumber(next.left, 0, Math.max(0, width - 1));
    const right = clampNumber(next.right, 0, Math.max(0, width - left - 1));
    const top = clampNumber(next.top, 0, Math.max(0, height - 1));
    const bottom = clampNumber(next.bottom, 0, Math.max(0, height - top - 1));
    return {
      left,
      right,
      top,
      bottom,
      edge_mode: next.edge_mode,
      center_mode: next.center_mode,
    };
  }

  function updateGuide(changes: Partial<NineSlice>) {
    guide = clampNineSlice({ ...guide, ...changes });
  }

  function updateGuideNumber(key: GuideHandle, value: string) {
    updateGuide({ [key]: Number(value) } as Partial<NineSlice>);
  }

  function updateGuideMode(key: "edge_mode" | "center_mode", value: string) {
    updateGuide({ [key]: value as NineSliceMode } as Partial<NineSlice>);
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

  function startGuideDrag(handle: GuideHandle, event: PointerEvent) {
    event.preventDefault();
    event.stopPropagation();
    guideDrag = handle;
    dragGuide(event);
    window.addEventListener("pointermove", dragGuide);
    window.addEventListener("pointerup", stopGuideDrag, { once: true });
  }

  function dragGuide(event: PointerEvent) {
    if (!guideDrag) return;
    const point = imagePoint(event);
    if (guideDrag === "left") updateGuide({ left: point.x });
    if (guideDrag === "right") updateGuide({ right: imageNaturalWidth - point.x });
    if (guideDrag === "top") updateGuide({ top: point.y });
    if (guideDrag === "bottom") updateGuide({ bottom: imageNaturalHeight - point.y });
  }

  function stopGuideDrag() {
    guideDrag = null;
    window.removeEventListener("pointermove", dragGuide);
  }

  function apply() {
    if (!selectedAsset) return;
    onapply(selectedAsset, mode === "nine_slice" ? clampNineSlice(guide) : clampRect(rect));
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
            aria-label={mode === "nine_slice" ? "Nine-slice guide canvas" : "UV selection canvas"}
            style={`width:${imageNaturalWidth * zoom}px;height:${imageNaturalHeight * zoom}px`}
            onpointerdown={mode === "uv" ? startDrag : undefined}
          >
            <img
              src={dataUrl}
              alt={selectedAsset}
              style={`width:${imageNaturalWidth * zoom}px;height:${imageNaturalHeight * zoom}px`}
              onload={(event) => {
                const image = event.currentTarget as HTMLImageElement;
                imageNaturalWidth = image.naturalWidth;
                imageNaturalHeight = image.naturalHeight;
                hasImageDimensions = true;
                rect = clampRect(rect);
                guide = clampNineSlice(guide);
              }}
            />
            {#if mode === "uv"}
              <div
                class="selection"
                style={`left:${rect.x * zoom}px;top:${rect.y * zoom}px;width:${rect.width * zoom}px;height:${rect.height * zoom}px`}
              ></div>
            {:else}
              <div class="nine-slice-guides">
                <div
                  class="slice-center"
                  style={`left:${guide.left * zoom}px;top:${guide.top * zoom}px;width:${Math.max(1, imageNaturalWidth - guide.left - guide.right) * zoom}px;height:${Math.max(1, imageNaturalHeight - guide.top - guide.bottom) * zoom}px`}
                ></div>
                <button
                  type="button"
                  class="guide-handle vertical"
                  style={`left:${guide.left * zoom - 3}px;top:0;height:${imageNaturalHeight * zoom}px`}
                  aria-label="Move left guide"
                  onpointerdown={(event) => startGuideDrag("left", event)}
                ></button>
                <button
                  type="button"
                  class="guide-handle vertical"
                  style={`left:${(imageNaturalWidth - guide.right) * zoom - 3}px;top:0;height:${imageNaturalHeight * zoom}px`}
                  aria-label="Move right guide"
                  onpointerdown={(event) => startGuideDrag("right", event)}
                ></button>
                <button
                  type="button"
                  class="guide-handle horizontal"
                  style={`left:0;top:${guide.top * zoom - 3}px;width:${imageNaturalWidth * zoom}px`}
                  aria-label="Move top guide"
                  onpointerdown={(event) => startGuideDrag("top", event)}
                ></button>
                <button
                  type="button"
                  class="guide-handle horizontal"
                  style={`left:0;top:${(imageNaturalHeight - guide.bottom) * zoom - 3}px;width:${imageNaturalWidth * zoom}px`}
                  aria-label="Move bottom guide"
                  onpointerdown={(event) => startGuideDrag("bottom", event)}
                ></button>
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <div class="missing-preview">No preview data for selected asset.</div>
      {/if}

      {#if mode === "uv"}
        <div class="numeric-grid">
          <label>X <input type="number" min="0" value={rect.x} oninput={(event) => updateRect({ x: Number(event.currentTarget.value) })} /></label>
          <label>Y <input type="number" min="0" value={rect.y} oninput={(event) => updateRect({ y: Number(event.currentTarget.value) })} /></label>
          <label>W <input type="number" min="1" value={rect.width} oninput={(event) => updateRect({ width: Number(event.currentTarget.value) })} /></label>
          <label>H <input type="number" min="1" value={rect.height} oninput={(event) => updateRect({ height: Number(event.currentTarget.value) })} /></label>
        </div>
      {:else}
        <div class="numeric-grid">
          <label>Left <input type="number" min="0" value={guide.left} oninput={(event) => updateGuideNumber("left", event.currentTarget.value)} /></label>
          <label>Right <input type="number" min="0" value={guide.right} oninput={(event) => updateGuideNumber("right", event.currentTarget.value)} /></label>
          <label>Top <input type="number" min="0" value={guide.top} oninput={(event) => updateGuideNumber("top", event.currentTarget.value)} /></label>
          <label>Bottom <input type="number" min="0" value={guide.bottom} oninput={(event) => updateGuideNumber("bottom", event.currentTarget.value)} /></label>
          <label>
            Edge
            <select value={guide.edge_mode} onchange={(event) => updateGuideMode("edge_mode", event.currentTarget.value)}>
              <option value="tile">Tile</option>
              <option value="stretch">Stretch</option>
            </select>
          </label>
          <label>
            Center
            <select value={guide.center_mode} onchange={(event) => updateGuideMode("center_mode", event.currentTarget.value)}>
              <option value="tile">Tile</option>
              <option value="stretch">Stretch</option>
            </select>
          </label>
          {#if targetSize}
            <div class="target-size">Target {targetSize.width}x{targetSize.height}</div>
          {/if}
        </div>
      {/if}
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

  .nine-slice-guides {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .slice-center {
    position: absolute;
    border: 1px dashed var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    pointer-events: none;
  }

  .guide-handle {
    position: absolute;
    z-index: 1;
    border: 0;
    border-radius: 0;
    background: var(--accent);
    padding: 0;
    opacity: 0.85;
    pointer-events: auto;
  }

  .guide-handle.vertical {
    width: 7px;
    cursor: ew-resize;
    background: linear-gradient(to right, transparent 0 3px, var(--accent) 3px 4px, transparent 4px 7px);
  }

  .guide-handle.horizontal {
    height: 7px;
    cursor: ns-resize;
    background: linear-gradient(to bottom, transparent 0 3px, var(--accent) 3px 4px, transparent 4px 7px);
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

  .target-size {
    color: var(--muted-text);
    font-size: 11px;
    font-family: monospace;
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
