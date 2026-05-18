<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";

  let selectedEl = $derived(editor.selectedElementId ? project.elementById(editor.selectedElementId) : null);
  let selectedGroup = $derived(editor.selectedElementId ? project.groupForElement(editor.selectedElementId) : null);

  function updateProp(key: string, value: unknown) {
    if (!editor.selectedElementId) return;
    project.updateElement(editor.selectedElementId, { [key]: value });
  }

  function numberValue(value: string, fallback = 0): number {
    const parsed = Number.parseInt(value, 10);
    return Number.isFinite(parsed) ? parsed : fallback;
  }

  function updateUv(key: "x" | "y" | "width" | "height", value: string) {
    if (!selectedEl) return;
    const next = {
      x: selectedEl.uv?.x ?? 0,
      y: selectedEl.uv?.y ?? 0,
      width: selectedEl.uv?.width ?? selectedEl.width ?? 16,
      height: selectedEl.uv?.height ?? selectedEl.height ?? 16,
      [key]: Math.max(key === "width" || key === "height" ? 1 : 0, numberValue(value)),
    };
    updateProp("uv", next);
  }
</script>

<aside class="properties">
  <h3>Properties</h3>

  {#if selectedEl}
    <div class="props-form">
      <div class="prop-row">
        <span class="prop-label">ID</span>
        <span class="prop-value mono">{selectedEl.id}</span>
      </div>
      <div class="prop-row">
        <span class="prop-label">Type</span>
        <span class="prop-value">{selectedEl.type}</span>
      </div>
      <div class="prop-row">
        <span class="prop-label">Group</span>
        {#if selectedGroup}
          <span class="prop-value mono">{selectedGroup.id}</span>
        {:else}
          <span class="prop-value">None</span>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-x">X</label>
        <input
          id="prop-x"
          type="number"
          value={selectedEl.x}
          oninput={(e) => updateProp("x", parseInt(e.currentTarget.value) || 0)}
        />
      </div>
      <div class="prop-row">
        <label for="prop-y">Y</label>
        <input
          id="prop-y"
          type="number"
          value={selectedEl.y}
          oninput={(e) => updateProp("y", parseInt(e.currentTarget.value) || 0)}
        />
      </div>

      <div class="prop-row">
        <label for="prop-layer">Layer</label>
        <select
          id="prop-layer"
          value={selectedEl.layer ?? "background"}
          onchange={(e) => updateProp("layer", e.currentTarget.value)}
        >
          <option value="background">Background</option>
          <option value="overlay">Overlay</option>
          <option value="animatable">Animatable</option>
        </select>
      </div>

      {#if selectedEl.type === "slot"}
        <div class="prop-row">
          <label for="prop-size">Size</label>
          <input
            id="prop-size"
            type="number"
            value={selectedEl.size ?? 18}
            oninput={(e) => updateProp("size", parseInt(e.currentTarget.value) || 18)}
          />
        </div>
      {:else if selectedEl.type === "texture" || selectedEl.type === "progress" || selectedEl.type === "fluid_tank" || selectedEl.type === "energy_bar"}
        <div class="prop-row">
          <label for="prop-width">Width</label>
          <input
            id="prop-width"
            type="number"
            value={selectedEl.width ?? ""}
            oninput={(e) => updateProp("width", parseInt(e.currentTarget.value) || undefined)}
          />
        </div>
        <div class="prop-row">
          <label for="prop-height">Height</label>
          <input
            id="prop-height"
            type="number"
            value={selectedEl.height ?? ""}
            oninput={(e) => updateProp("height", parseInt(e.currentTarget.value) || undefined)}
          />
        </div>
      {/if}

      {#if selectedEl.type === "texture"}
        <div class="prop-row">
          <label for="prop-asset">Texture</label>
          <select
            id="prop-asset"
            value={selectedEl.asset ?? ""}
            onchange={(e) => updateProp("asset", e.currentTarget.value || undefined)}
          >
            <option value="">(none)</option>
            {#each project.assets as a}
              <option value={a}>{a.replace("textures/", "").replace(".png", "")}</option>
            {/each}
          </select>
        </div>
        <div class="prop-section">
          <div class="section-title">UV Rect</div>
          <div class="uv-grid">
            <label for="prop-uv-x">X</label>
            <input
              id="prop-uv-x"
              type="number"
              min="0"
              value={selectedEl.uv?.x ?? 0}
              oninput={(e) => updateUv("x", e.currentTarget.value)}
            />
            <label for="prop-uv-y">Y</label>
            <input
              id="prop-uv-y"
              type="number"
              min="0"
              value={selectedEl.uv?.y ?? 0}
              oninput={(e) => updateUv("y", e.currentTarget.value)}
            />
            <label for="prop-uv-width">W</label>
            <input
              id="prop-uv-width"
              type="number"
              min="1"
              value={selectedEl.uv?.width ?? selectedEl.width ?? 16}
              oninput={(e) => updateUv("width", e.currentTarget.value)}
            />
            <label for="prop-uv-height">H</label>
            <input
              id="prop-uv-height"
              type="number"
              min="1"
              value={selectedEl.uv?.height ?? selectedEl.height ?? 16}
              oninput={(e) => updateUv("height", e.currentTarget.value)}
            />
          </div>
          <button class="secondary-btn" onclick={() => updateProp("uv", null)}>
            Clear UV
          </button>
        </div>
      {/if}

      {#if selectedEl.type === "progress"}
        <div class="prop-row">
          <label for="prop-direction">Direction</label>
          <select
            id="prop-direction"
            value={selectedEl.direction ?? "left_to_right"}
            onchange={(e) => updateProp("direction", e.currentTarget.value)}
          >
            <option value="left_to_right">Left to Right</option>
            <option value="right_to_left">Right to Left</option>
            <option value="bottom_to_top">Bottom to Top</option>
            <option value="top_to_bottom">Top to Bottom</option>
          </select>
        </div>
      {/if}

      {#if selectedEl.type === "text"}
        <div class="prop-row">
          <label for="prop-content">Content</label>
          <input
            id="prop-content"
            type="text"
            value={selectedEl.content ?? ""}
            oninput={(e) => updateProp("content", e.currentTarget.value)}
          />
        </div>
        <div class="prop-row">
          <label for="prop-font">Font</label>
          <select
            id="prop-font"
            value={selectedEl.font ?? "minecraft:default"}
            onchange={(e) => updateProp("font", e.currentTarget.value)}
          >
            {#each project.fonts as font}
              <option value={font.id}>{font.id}</option>
            {/each}
            {#if project.fonts.length === 0}
              <option value="minecraft:default">minecraft:default</option>
            {/if}
          </select>
        </div>
        <div class="prop-row">
          <label for="prop-color">Color</label>
          <input
            id="prop-color"
            type="text"
            value={selectedEl.color?.toString(16) ?? "404040"}
            oninput={(e) => updateProp("color", parseInt(e.currentTarget.value, 16) || 0x404040)}
          />
        </div>
        <div class="prop-row">
          <label for="prop-shadow">Shadow</label>
          <input
            id="prop-shadow"
            type="checkbox"
            checked={selectedEl.shadow ?? false}
            onchange={(e) => updateProp("shadow", e.currentTarget.checked)}
          />
        </div>
      {/if}

      <hr class="divider" />

      <button class="delete-btn" onclick={() => {
        if (editor.selectedElementId) {
          project.removeElement(editor.selectedElementId);
          editor.clearSelection();
        }
      }}>
        Delete Element
      </button>

      <button class="delete-btn" onclick={() => {
        if (editor.selectedElementId) {
          const el = project.elementById(editor.selectedElementId);
          if (el) {
            project.addElement(el.type, el.x + 20, el.y + 20, { ...el, id: undefined });
          }
        }
      }}>
        Duplicate
      </button>

      {#if selectedGroup}
        <button class="secondary-btn" onclick={() => project.ungroup(selectedGroup.id)}>
          Ungroup
        </button>
      {/if}
    </div>
  {:else}
    <p class="muted">
      {#if project.isOpen}
        Select an element to edit
      {:else}
        Create or open a project
      {/if}
    </p>
  {/if}
</aside>

<style>
  .properties {
    padding: 10px;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #606080;
    margin-bottom: 8px;
  }

  .muted {
    color: #505060;
    font-size: 12px;
  }

  .props-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .prop-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .prop-label,
  .prop-row label {
    font-size: 11px;
    color: #606080;
    width: 55px;
    flex-shrink: 0;
  }

  .prop-value {
    font-size: 12px;
    color: #a0a0b0;
  }

  .prop-value.mono {
    font-family: monospace;
    font-size: 11px;
  }

  .prop-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-top: 1px solid #0f3460;
    padding-top: 8px;
    margin-top: 2px;
  }

  .section-title {
    color: #606080;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .uv-grid {
    display: grid;
    grid-template-columns: 18px 1fr 18px 1fr;
    gap: 6px;
    align-items: center;
  }

  .uv-grid label {
    width: auto;
  }

  input[type="number"],
  input[type="text"],
  select {
    flex: 1;
    background: #12121f;
    border: 1px solid #0f3460;
    color: #e0e0e0;
    padding: 3px 6px;
    font-size: 11px;
    font-family: monospace;
    border-radius: 2px;
    width: 100%;
  }

  input[type="number"]:focus,
  input[type="text"]:focus,
  select:focus {
    outline: none;
    border-color: #e94560;
  }

  input[type="checkbox"] {
    accent-color: #e94560;
  }

  select {
    cursor: pointer;
  }

  .divider {
    border: none;
    border-top: 1px solid #0f3460;
    margin: 8px 0;
  }

  .delete-btn {
    background: transparent;
    border: 1px solid #e94560;
    color: #e94560;
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 2px;
    font-family: inherit;
    width: 100%;
  }

  .secondary-btn {
    background: transparent;
    border: 1px solid #0f3460;
    color: #a0a0b0;
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 2px;
    font-family: inherit;
    width: 100%;
  }

  .secondary-btn:hover {
    background: #0f3460;
    color: #e0e0e0;
  }

  .delete-btn:hover {
    background: #e94560;
    color: #12121f;
  }
</style>
