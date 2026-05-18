<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import * as api from "../api";
  import type { TemplateInfo } from "../api";
  import { getGuiPreset, guiPresets, type GuiPresetId } from "../guiPresets";
  import { preferences } from "../stores/preferences.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const initialPreset = getGuiPreset(preferences.values.defaultPreset) ?? getGuiPreset("custom");

  let templates = $state<TemplateInfo[]>([]);
  let selectedTemplate = $state<string>("empty");
  let selectedPreset = $state<GuiPresetId>(initialPreset?.id ?? "custom");
  let width = $state(initialPreset?.width ?? 176);
  let height = $state(initialPreset?.height ?? 166);
  let modTarget = $state<"forge" | "fabric" | "neoforge">("forge");
  let customGridWidth = $state(3);
  let customGridHeight = $state(3);
  let customGridOutputSlot = $state(true);
  let customGridProgressArrow = $state(true);
  let customGridPlayerInventory = $state(true);

  $effect(() => {
    api.templateList().then(t => { templates = t; });
  });

  function selectTemplate(name: string) {
    selectedTemplate = name;
    selectedPreset = "custom";
    const t = templates.find(t => t.name === name);
    if (t) {
      width = t.default_width;
      height = t.default_height;
    }
  }

  function selectPreset(event: Event) {
    selectedPreset = (event.currentTarget as HTMLSelectElement).value as GuiPresetId;
    selectedTemplate = "empty";
    const preset = getGuiPreset(selectedPreset);
    if (preset) {
      width = preset.width;
      height = preset.height;
    }
  }

  function updateDimension(dimension: "width" | "height", event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const value = Math.max(Number(input.min) || 16, Math.round(input.valueAsNumber || 16));
    if (dimension === "width") {
      width = value;
    } else {
      height = value;
    }
    selectedPreset = "custom";
    selectedTemplate = "empty";
  }

  function updateCustomGridDimension(dimension: "width" | "height", event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const value = Math.min(
      Number(input.max),
      Math.max(Number(input.min), Math.round(input.valueAsNumber || Number(input.min))),
    );

    if (dimension === "width") {
      customGridWidth = value;
    } else {
      customGridHeight = value;
    }
  }

  function templateDescription(template: TemplateInfo) {
    if (template.name === "custom_grid") {
      return "Default 3x3 custom grid starter layout";
    }

    return template.description;
  }

  async function handleCreate() {
    const template = selectedTemplate === "empty" ? undefined : selectedTemplate;
    await project.newProject("Untitled GUI", width, height, modTarget, template);
    editor.resetView();
    onclose();
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

<div class="dialog-overlay" role="presentation" onclick={handleOverlayClick} onkeydown={handleOverlayKeydown}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="new-project-title">
    <h2 id="new-project-title">New Project</h2>

    <div class="template-grid">
      {#each templates as t (t.name)}
        <button
          class="template-card"
          class:selected={selectedTemplate === t.name}
          onclick={() => selectTemplate(t.name)}
        >
          <span class="template-name">{t.name.replace(/_/g, " ")}</span>
          <span class="template-size">{t.default_width}×{t.default_height}</span>
          <span class="template-desc">{templateDescription(t)}</span>
          <span class="template-elements">{t.element_count} elements</span>
        </button>
      {/each}
    </div>

    {#if selectedTemplate === "custom_grid"}
      <div class="custom-grid-options" aria-labelledby="custom-grid-options-title">
        <div class="custom-grid-header">
          <span id="custom-grid-options-title">Custom Grid Options</span>
          <span class="custom-grid-note">
            Current backend creates the default 3×3 custom grid. These choices are for a later
            parameterized-template pass.
          </span>
        </div>

        <div class="custom-grid-fields">
          <label for="np-custom-grid-width">
            <span>Grid W</span>
            <input
              id="np-custom-grid-width"
              type="number"
              value={customGridWidth}
              min="1"
              max="9"
              oninput={(event) => updateCustomGridDimension("width", event)}
            />
          </label>

          <label for="np-custom-grid-height">
            <span>Grid H</span>
            <input
              id="np-custom-grid-height"
              type="number"
              value={customGridHeight}
              min="1"
              max="6"
              oninput={(event) => updateCustomGridDimension("height", event)}
            />
          </label>

          <label class="custom-grid-check" for="np-custom-grid-output">
            <input
              id="np-custom-grid-output"
              type="checkbox"
              bind:checked={customGridOutputSlot}
            />
            <span>Output slot</span>
          </label>

          <label class="custom-grid-check" for="np-custom-grid-progress">
            <input
              id="np-custom-grid-progress"
              type="checkbox"
              bind:checked={customGridProgressArrow}
            />
            <span>Progress arrow</span>
          </label>

          <label class="custom-grid-check" for="np-custom-grid-inventory">
            <input
              id="np-custom-grid-inventory"
              type="checkbox"
              bind:checked={customGridPlayerInventory}
            />
            <span>Player inventory</span>
          </label>
        </div>
      </div>
    {/if}

    <div class="form-row">
      <label for="np-preset">Preset</label>
      <select id="np-preset" value={selectedPreset} onchange={selectPreset}>
        {#each guiPresets as preset (preset.id)}
          <option value={preset.id}>{preset.label} ({preset.width}×{preset.height})</option>
        {/each}
      </select>
    </div>

    <div class="form-row">
      <label for="np-width">Width</label>
      <input
        id="np-width"
        type="number"
        value={width}
        min="16"
        max="1024"
        oninput={(event) => updateDimension("width", event)}
      />
      <label for="np-height">Height</label>
      <input
        id="np-height"
        type="number"
        value={height}
        min="16"
        max="1024"
        oninput={(event) => updateDimension("height", event)}
      />
    </div>

    <div class="form-row">
      <label for="np-target">Target</label>
      <select id="np-target" bind:value={modTarget}>
        <option value="forge">Forge</option>
        <option value="fabric">Fabric</option>
        <option value="neoforge">NeoForge</option>
      </select>
    </div>

    <div class="dialog-actions">
      <button class="cancel-btn" onclick={onclose}>Cancel</button>
      <button class="create-btn" onclick={handleCreate}>Create</button>
    </div>
  </div>
</div>

<style>
  .dialog-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 16px;
  }

  .dialog {
    width: min(560px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow: auto;
    background: #1a1a2e;
    border: 1px solid #0f3460;
    border-radius: 8px;
    padding: 16px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  }

  h2 {
    font-size: 15px;
    color: #e0e0e0;
    margin: 0 0 12px;
  }

  .template-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-bottom: 12px;
  }

  .template-card {
    background: #12121f;
    border: 1px solid #0f3460;
    border-radius: 6px;
    padding: 10px;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
    color: #a0a0b0;
    transition: border-color 0.15s;
  }

  .template-card:hover {
    border-color: #e94560;
  }

  .template-card.selected {
    border-color: #e94560;
    background: #1a0f1f;
  }

  .template-name {
    display: block;
    font-size: 13px;
    font-weight: 600;
    color: #e0e0e0;
    text-transform: capitalize;
  }

  .template-size {
    display: block;
    font-size: 11px;
    color: #e94560;
    font-family: monospace;
    margin-top: 2px;
  }

  .template-desc {
    display: block;
    font-size: 11px;
    color: #606080;
    margin-top: 4px;
  }

  .template-elements {
    display: block;
    font-size: 10px;
    color: #505060;
    margin-top: 2px;
  }

  .form-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
    min-width: 0;
  }

  .form-row label {
    font-size: 11px;
    color: #606080;
    min-width: 45px;
  }

  input[type="number"],
  select {
    background: #12121f;
    border: 1px solid #0f3460;
    color: #e0e0e0;
    padding: 4px 8px;
    font-size: 12px;
    font-family: monospace;
    border-radius: 4px;
    width: 80px;
  }

  select {
    width: auto;
    max-width: 100%;
  }

  input:focus,
  select:focus {
    outline: 2px solid #e94560;
    outline-offset: 2px;
  }

  .custom-grid-options {
    background: #12121f;
    border: 1px solid #0f3460;
    border-radius: 6px;
    padding: 10px;
    margin-bottom: 12px;
  }

  .custom-grid-header {
    display: grid;
    gap: 4px;
    margin-bottom: 8px;
    min-width: 0;
  }

  .custom-grid-header > span:first-child {
    color: #e0e0e0;
    font-size: 12px;
    font-weight: 600;
  }

  .custom-grid-note {
    color: #808090;
    font-size: 11px;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }

  .custom-grid-fields {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(118px, 1fr));
    gap: 8px;
    min-width: 0;
  }

  .custom-grid-fields label {
    min-width: 0;
  }

  .custom-grid-fields label:not(.custom-grid-check) {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 56px;
    align-items: center;
    gap: 6px;
    color: #606080;
    font-size: 11px;
  }

  .custom-grid-fields input[type="number"] {
    width: 56px;
  }

  .custom-grid-check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #a0a0b0;
    font-size: 11px;
    min-height: 26px;
  }

  .custom-grid-check input {
    flex: 0 0 auto;
  }

  .custom-grid-check span,
  .custom-grid-fields label:not(.custom-grid-check) span {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .cancel-btn, .create-btn {
    padding: 6px 16px;
    font-size: 12px;
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
  }

  .cancel-btn {
    background: transparent;
    border: 1px solid #0f3460;
    color: #808090;
  }

  .cancel-btn:hover {
    background: #0f3460;
  }

  .create-btn {
    background: #e94560;
    border: 1px solid #e94560;
    color: #12121f;
    font-weight: 600;
  }

  .create-btn:hover {
    background: #ff5a7a;
  }

  .cancel-btn:focus-visible,
  .create-btn:focus-visible,
  .template-card:focus-visible {
    outline: 2px solid #e94560;
    outline-offset: 2px;
  }

  @media (max-width: 560px) {
    .template-grid {
      grid-template-columns: 1fr;
    }

    .form-row {
      flex-wrap: wrap;
    }
  }
</style>
