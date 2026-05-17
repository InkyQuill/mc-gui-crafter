<script lang="ts">
  let { assetName, dataUrl, onclose, onsaved }: {
    assetName: string;
    dataUrl: string;
    onclose: () => void;
    onsaved: (newDataUrl: string) => void | Promise<void>;
  } = $props();

  type ZoomLevel = 1 | 2 | 4 | 8 | "fit";

  const zoomOptions: { value: ZoomLevel; label: string }[] = [
    { value: 1, label: "1x" },
    { value: 2, label: "2x" },
    { value: 4, label: "4x" },
    { value: 8, label: "8x" },
    { value: "fit", label: "Fit" },
  ];

  let canvasEl: HTMLCanvasElement | undefined = $state();
  let ctx: CanvasRenderingContext2D | null = $state(null);
  let imgWidth = $state(16);
  let imgHeight = $state(16);
  let canvasWrapWidth = $state(0);
  let canvasWrapHeight = $state(0);
  let zoom = $state<ZoomLevel>(4);
  let saveError = $state<string | null>(null);
  let isSaving = $state(false);

  let canvasStyle = $derived.by(() => {
    if (zoom === "fit") {
      const availableWidth = Math.max(1, canvasWrapWidth - 24);
      const availableHeight = Math.max(1, canvasWrapHeight - 24);
      const scale = Math.min(
        8,
        availableWidth / Math.max(1, imgWidth),
        availableHeight / Math.max(1, imgHeight),
      );

      return [
        `width: ${imgWidth * scale}px`,
        `height: ${imgHeight * scale}px`,
      ].join("; ");
    }

    return [
      `width: ${imgWidth * zoom}px`,
      `height: ${imgHeight * zoom}px`,
    ].join("; ");
  });

  // Tools
  let tool = $state<"pencil" | "eraser" | "eyedropper" | "fill">("pencil");
  let brushSize = $state(1);
  let currentColor = $state("#e94560");

  // Minecraft color palette presets
  const mcColors = [
    "#000000", "#0000AA", "#00AA00", "#00AAAA", "#AA0000", "#AA00AA",
    "#FFAA00", "#AAAAAA", "#555555", "#5555FF", "#55FF55", "#55FFFF",
    "#FF5555", "#FF55FF", "#FFFF55", "#FFFFFF",
    "#1a1a2e", "#0f3460", "#e94560", "#e9a23b", "#3b82e9", "#6b4e9b",
  ];

  $effect(() => {
    if (!canvasEl || !dataUrl) return;
    const img = new Image();
    img.onload = () => {
      imgWidth = img.naturalWidth;
      imgHeight = img.naturalHeight;
      canvasEl!.width = imgWidth;
      canvasEl!.height = imgHeight;
      const c = canvasEl!.getContext("2d")!;
      ctx = c;
      c.imageSmoothingEnabled = false;
      c.drawImage(img, 0, 0);
    };
    img.src = dataUrl;
  });

  function getPixel(x: number, y: number): string {
    if (!ctx) return "#000";
    const data = ctx.getImageData(x, y, 1, 1).data;
    return `#${data[0].toString(16).padStart(2, "0")}${data[1].toString(16).padStart(2, "0")}${data[2].toString(16).padStart(2, "0")}`;
  }

  function setPixel(x: number, y: number, color: string) {
    if (!ctx) return;
    const size = brushSize;
    ctx.fillStyle = color;
    ctx.fillRect(x, y, size, size);
  }

  function floodFill(startX: number, startY: number, fillColor: string) {
    if (!ctx) return;
    const targetColor = getPixel(startX, startY);
    if (targetColor === fillColor) return;

    const stack: [number, number][] = [[startX, startY]];
    const visited = new Set<string>();

    while (stack.length > 0) {
      const [x, y] = stack.pop()!;
      const key = `${x},${y}`;
      if (visited.has(key)) continue;
      if (x < 0 || x >= imgWidth || y < 0 || y >= imgHeight) continue;
      if (getPixel(x, y) !== targetColor) continue;

      visited.add(key);
      setPixel(x, y, fillColor);
      stack.push([x + 1, y], [x - 1, y], [x, y + 1], [x, y - 1]);
    }
  }

  function handleCanvasClick(e: MouseEvent) {
    if (!ctx) return;
    const rect = canvasEl!.getBoundingClientRect();
    const scaleX = imgWidth / rect.width;
    const scaleY = imgHeight / rect.height;
    const x = Math.floor((e.clientX - rect.left) * scaleX);
    const y = Math.floor((e.clientY - rect.top) * scaleY);

    if (tool === "pencil") {
      setPixel(x, y, currentColor);
    } else if (tool === "eraser") {
      setPixel(x, y, "#00000000");
      ctx.clearRect(x, y, brushSize, brushSize);
    } else if (tool === "eyedropper") {
      currentColor = getPixel(x, y);
      tool = "pencil";
    } else if (tool === "fill") {
      floodFill(x, y, currentColor);
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (e.buttons !== 1) return;
    handleCanvasClick(e);
  }

  async function handleSave() {
    if (!canvasEl || isSaving) return;
    saveError = null;
    isSaving = true;
    try {
      const newDataUrl = canvasEl.toDataURL("image/png");
      await onsaved(newDataUrl);
      onclose();
    } catch (error) {
      saveError = error instanceof Error ? error.message : String(error || "Failed to save asset");
    } finally {
      isSaving = false;
    }
  }

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      onclose();
    }
  }

  function handleOverlayKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onclose();
    }
  }
</script>

<div class="pixel-editor-overlay" role="presentation" onclick={handleOverlayClick} onkeydown={handleOverlayKeydown}>
  <div class="pixel-editor" role="dialog" aria-modal="true" aria-labelledby="pixel-editor-title">
    <div class="pe-header">
      <span id="pixel-editor-title" class="pe-title">Edit: {assetName.replace("textures/", "")}</span>
      <span class="pe-size">{imgWidth}×{imgHeight}</span>
      <div class="pe-zoom" role="group" aria-label="Zoom level">
        {#each zoomOptions as option}
          <button
            type="button"
            class:active={zoom === option.value}
            aria-pressed={zoom === option.value}
            aria-label={`Set zoom to ${option.label}`}
            title={`Set zoom to ${option.label}`}
            onclick={() => zoom = option.value}
          >
            {option.label}
          </button>
        {/each}
      </div>
      <button class="pe-close" onclick={onclose} title="Close" aria-label="Close pixel editor">×</button>
    </div>

    <div class="pe-body">
      <div class="pe-tools">
        <button class:active={tool === "pencil"} onclick={() => tool = "pencil"} title="Pencil" aria-label="Pencil">✎</button>
        <button class:active={tool === "eraser"} onclick={() => tool = "eraser"} title="Eraser" aria-label="Eraser">⌫</button>
        <button class:active={tool === "eyedropper"} onclick={() => tool = "eyedropper"} title="Color picker" aria-label="Color picker">◉</button>
        <button class:active={tool === "fill"} onclick={() => tool = "fill"} title="Fill" aria-label="Fill">▨</button>
        <span class="sep"></span>
        <label for="pe-brush-size" class="pe-size-label">Size</label>
        <input id="pe-brush-size" type="number" bind:value={brushSize} min="1" max="8" class="pe-size-input" />
      </div>

      <div
        class:fit={zoom === "fit"}
        class="pe-canvas-wrap"
        bind:clientWidth={canvasWrapWidth}
        bind:clientHeight={canvasWrapHeight}
      >
        <canvas
          bind:this={canvasEl}
          class="pe-canvas"
          style={canvasStyle}
          onclick={handleCanvasClick}
          onmousemove={handleMouseMove}
        ></canvas>
      </div>

      <div class="pe-palette">
        <input type="color" bind:value={currentColor} class="pe-color-picker" />
        <span class="pe-hex">{currentColor}</span>
        <div class="pe-mc-palette">
          {#each mcColors as c}
            <button
              class="pe-swatch"
              class:selected={currentColor === c}
              style="background: {c}"
              onclick={() => currentColor = c}
              aria-label="Use color {c}"
              title="Use color {c}"
            ></button>
          {/each}
        </div>
      </div>
    </div>

    <div class="pe-footer">
      {#if saveError}
        <span class="pe-error">{saveError}</span>
      {/if}
      <button class="pe-cancel" onclick={onclose}>Cancel</button>
      <button class="pe-save" onclick={handleSave} disabled={isSaving}>
        {isSaving ? "Saving..." : "Save"}
      </button>
    </div>
  </div>
</div>

<style>
  .pixel-editor-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
    padding: 12px;
  }

  .pixel-editor {
    background: #1a1a2e;
    border: 1px solid #0f3460;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    min-width: 400px;
    max-width: calc(100vw - 24px);
    max-height: calc(100vh - 24px);
    display: flex;
    flex-direction: column;
  }

  .pe-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid #0f3460;
    flex-wrap: wrap;
  }

  .pe-title {
    font-size: 13px;
    color: #e0e0e0;
    font-family: monospace;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pe-size {
    font-size: 11px;
    color: #606080;
    font-family: monospace;
    flex: 0 0 auto;
  }

  .pe-zoom {
    display: inline-flex;
    flex: 0 0 auto;
    overflow: hidden;
    border: 1px solid #0f3460;
    border-radius: 4px;
  }

  .pe-zoom button {
    min-width: 32px;
    height: 24px;
    padding: 0 7px;
    background: #12121f;
    border: 0;
    border-right: 1px solid #0f3460;
    color: #808090;
    font-size: 10px;
    font-family: inherit;
    cursor: pointer;
  }

  .pe-zoom button:last-child {
    border-right: 0;
  }

  .pe-zoom button:hover,
  .pe-zoom button.active {
    background: #1a0f1f;
    color: #e94560;
  }

  .pe-close {
    margin-left: auto;
    background: transparent;
    border: 1px solid transparent;
    color: #a0a0b0;
    width: 28px;
    height: 28px;
    padding: 0;
    border-radius: 4px;
    font-size: 18px;
    cursor: pointer;
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .pe-close:hover { background: #0f3460; color: #e0e0e0; }

  .pe-body {
    display: flex;
    gap: 12px;
    padding: 12px;
    min-height: 0;
    overflow: hidden;
  }

  .pe-tools {
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: center;
  }

  .pe-tools button {
    background: transparent;
    border: 1px solid #0f3460;
    color: #808090;
    width: 32px;
    height: 32px;
    font-size: 14px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .pe-tools button:hover, .pe-tools button.active {
    border-color: #e94560;
    color: #e94560;
    background: #1a0f1f;
  }

  .sep {
    border-top: 1px solid #0f3460;
    width: 100%;
    margin: 4px 0;
  }

  .pe-size-label {
    font-size: 9px;
    color: #505060;
  }

  .pe-size-input {
    width: 40px;
    background: #12121f;
    border: 1px solid #0f3460;
    color: #e0e0e0;
    font-size: 11px;
    text-align: center;
    border-radius: 2px;
    padding: 2px;
  }

  .pe-canvas-wrap {
    flex: 1;
    min-width: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    background-image:
      linear-gradient(45deg, #222 25%, transparent 25%),
      linear-gradient(-45deg, #222 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #222 75%),
      linear-gradient(-45deg, transparent 75%, #222 75%);
    background-size: 16px 16px;
    background-position: 0 0, 0 8px, 8px -8px, -8px 0px;
    border: 1px solid #0f3460;
    border-radius: 4px;
    min-height: 200px;
    max-width: calc(100vw - 144px);
    max-height: calc(100vh - 172px);
    overflow: auto;
    padding: 12px;
  }

  .pe-canvas {
    image-rendering: crisp-edges;
    image-rendering: pixelated;
    flex: 0 0 auto;
    cursor: crosshair;
  }

  .pe-canvas-wrap.fit {
    width: calc(100vw - 144px);
    height: calc(100vh - 172px);
  }

  .pe-canvas-wrap.fit .pe-canvas {
    max-width: 100%;
    max-height: 100%;
  }

  .pe-palette {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: center;
    flex: 0 0 auto;
  }

  .pe-color-picker {
    width: 40px;
    height: 40px;
    border: 1px solid #0f3460;
    border-radius: 4px;
    cursor: pointer;
    padding: 0;
  }

  .pe-hex {
    font-size: 10px;
    color: #808090;
    font-family: monospace;
  }

  .pe-mc-palette {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
  }

  .pe-swatch {
    width: 20px;
    height: 20px;
    border: 1px solid #0f3460;
    border-radius: 2px;
    cursor: pointer;
    padding: 0;
  }

  .pe-swatch.selected {
    border-color: #fff;
    box-shadow: 0 0 0 1px #fff;
  }

  .pe-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    padding: 8px 12px;
    border-top: 1px solid #0f3460;
    flex-wrap: wrap;
  }

  .pe-error {
    margin-right: auto;
    color: #ff7a90;
    font-size: 11px;
  }

  .pe-cancel, .pe-save {
    padding: 6px 16px;
    font-size: 12px;
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
  }

  .pe-cancel {
    background: transparent;
    border: 1px solid #0f3460;
    color: #808090;
  }

  .pe-save {
    background: #e94560;
    border: 1px solid #e94560;
    color: #12121f;
    font-weight: 600;
  }

  .pe-save:disabled {
    opacity: 0.7;
    cursor: wait;
  }

  button:focus-visible,
  input:focus-visible {
    outline: 2px solid #e94560;
    outline-offset: 2px;
  }

  @media (max-width: 560px) {
    .pixel-editor {
      min-width: 0;
      width: calc(100vw - 24px);
    }

    .pe-body {
      flex-wrap: wrap;
      overflow: auto;
    }

    .pe-tools,
    .pe-palette {
      flex-direction: row;
      align-items: center;
    }

    .pe-canvas-wrap {
      flex-basis: 100%;
      order: 2;
      max-width: 100%;
      max-height: calc(100vh - 236px);
    }

    .pe-canvas-wrap.fit {
      width: 100%;
      height: calc(100vh - 236px);
    }
  }
</style>
