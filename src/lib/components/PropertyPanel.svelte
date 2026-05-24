<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import UvEditorDialog from "./UvEditorDialog.svelte";
  import type { CodegenMode, Element, SlotRole, UvRect } from "../types";

  type UvTarget = "uv" | "icon_uv";

  let uvEditorTarget = $state<UvTarget | null>(null);
  let selectedElementId = $derived.by(() => {
    void editor.selectionRevision;
    return editor.selectedElementId;
  });
  let selectedEl = $derived(selectedElementId ? project.elementById(selectedElementId) : null);
  let selectedGroup = $derived(selectedElementId ? project.groupForElement(selectedElementId) : null);
  let fontOptions = $derived.by(() => {
    const options = project.fonts.filter((font, index, fonts) => fonts.findIndex(candidate => candidate.id === font.id) === index);
    if (!options.some(font => font.id === "minecraft:default")) {
      options.unshift({ id: "minecraft:default", source: { type: "minecraft" } });
    }
    return options;
  });
  const slotRoleOptions: SlotRole[] = [
    "machine",
    "player_inventory",
    "hotbar",
    "scrollable_inventory",
    "virtual_storage",
    "upgrade",
    "upgrade_settings",
    "filter",
    "ghost",
    "offhand",
  ];

  function updateProp(key: string, value: unknown) {
    if (!editor.selectedElementId) return;
    project.updateElement(editor.selectedElementId, { [key]: value });
  }

  function updateSelectedElement(changes: Partial<Element>) {
    if (!selectedEl) return;
    project.updateElement(selectedEl.id, changes);
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

  function updateIconUv(key: "x" | "y" | "width" | "height", value: string) {
    if (!selectedEl) return;
    const next = {
      x: selectedEl.icon_uv?.x ?? 0,
      y: selectedEl.icon_uv?.y ?? 0,
      width: selectedEl.icon_uv?.width ?? 16,
      height: selectedEl.icon_uv?.height ?? 16,
      [key]: Math.max(key === "width" || key === "height" ? 1 : 0, numberValue(value)),
    };
    updateSelectedElement({ icon_uv: next });
  }

  function optionalText(value: string): string | null {
    return value.trim() || null;
  }

  function optionalU32(value: string): number | null | undefined {
    if (value === "") return null;
    if (!/^\d+$/.test(value)) return undefined;
    const parsed = Number(value);
    return Number.isSafeInteger(parsed) ? parsed : undefined;
  }

  function updateOptionalU32(key: "slot_index" | "columns" | "visible_rows" | "total_rows", value: string) {
    const parsed = optionalU32(value);
    if (parsed !== undefined) {
      updateSelectedElement({ [key]: parsed });
    }
  }

  function openUvEditor(target: UvTarget) {
    uvEditorTarget = target;
  }

  function applyUvSelection(asset: string, uv: UvRect | null) {
    if (!selectedEl || !uvEditorTarget) return;
    if (uvEditorTarget === "icon_uv") {
      updateSelectedElement({ icon: asset, icon_uv: uv });
    } else {
      updateSelectedElement({ asset, uv });
    }
    uvEditorTarget = null;
  }

  function clearUvSelection() {
    if (!selectedEl || !uvEditorTarget) return;
    if (uvEditorTarget === "icon_uv") {
      updateSelectedElement({ icon_uv: null });
    } else {
      updateSelectedElement({ uv: null });
    }
    uvEditorTarget = null;
  }
</script>

<aside class="properties">
  <h3>Properties</h3>

  {#if project.isOpen}
    <div class="props-form project-form">
      <div class="prop-row">
        <label for="project-codegen-mode">Code Gen</label>
        <select
          id="project-codegen-mode"
          value={project.exportSettings.codegen_mode}
          onchange={(event) => project.updateExportSettings({ codegen_mode: event.currentTarget.value as CodegenMode })}
        >
          <option value="simple">Simple</option>
          <option value="modular">Modular</option>
        </select>
      </div>
      <div class="prop-row">
        <label for="project-runtime-helpers">Runtime</label>
        <input
          id="project-runtime-helpers"
          type="checkbox"
          checked={project.exportSettings.generate_runtime_helpers}
          onchange={(event) => project.updateExportSettings({ generate_runtime_helpers: event.currentTarget.checked })}
        />
      </div>
    </div>

    <hr class="divider" />
  {/if}

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
      {:else if selectedEl.type === "texture" || selectedEl.type === "progress" || selectedEl.type === "fluid_tank" || selectedEl.type === "energy_bar" || selectedEl.type === "button" || selectedEl.type === "toggle_button"}
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

      {#if selectedEl.type === "slot" || selectedEl.type === "virtual_slot_cell"}
        <div class="prop-section">
          <div class="section-title">Slot</div>
          <div class="prop-row">
            <label for="prop-slot-role">Role</label>
            <select
              id="prop-slot-role"
              value={selectedEl.slot_role ?? ""}
              onchange={(e) => updateSelectedElement({ slot_role: (e.currentTarget.value || null) as SlotRole | null })}
            >
              <option value="">(none)</option>
              {#each slotRoleOptions as role (role)}
                <option value={role}>{role}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <label for="prop-inventory-group">Group</label>
            <input
              id="prop-inventory-group"
              type="text"
              value={selectedEl.inventory_group ?? ""}
              oninput={(e) => updateSelectedElement({ inventory_group: optionalText(e.currentTarget.value) })}
            />
          </div>
          <div class="prop-row">
            <label for="prop-slot-index">Index</label>
            <input
              id="prop-slot-index"
              type="number"
              min="0"
              step="1"
              value={selectedEl.slot_index ?? ""}
              oninput={(e) => updateOptionalU32("slot_index", e.currentTarget.value)}
            />
          </div>
          <div class="prop-row">
            <label for="prop-scroll-binding">Scroll</label>
            <input
              id="prop-scroll-binding"
              type="text"
              value={selectedEl.scroll_binding ?? ""}
              oninput={(e) => updateSelectedElement({ scroll_binding: optionalText(e.currentTarget.value) })}
            />
          </div>
        </div>
      {/if}

      {#if selectedEl.type === "scrollbar"}
        <div class="prop-section">
          <div class="section-title">Scrollbar</div>
          <div class="prop-row">
            <label for="prop-scroll-columns">Columns</label>
            <input
              id="prop-scroll-columns"
              type="number"
              min="0"
              step="1"
              value={selectedEl.columns ?? ""}
              oninput={(e) => updateOptionalU32("columns", e.currentTarget.value)}
            />
          </div>
          <div class="prop-row">
            <label for="prop-visible-rows">Visible</label>
            <input
              id="prop-visible-rows"
              type="number"
              min="0"
              step="1"
              value={selectedEl.visible_rows ?? ""}
              oninput={(e) => updateOptionalU32("visible_rows", e.currentTarget.value)}
            />
          </div>
          <div class="prop-row">
            <label for="prop-total-rows">Total</label>
            <input
              id="prop-total-rows"
              type="number"
              min="0"
              step="1"
              value={selectedEl.total_rows ?? ""}
              oninput={(e) => updateOptionalU32("total_rows", e.currentTarget.value)}
            />
          </div>
          <div class="prop-row">
            <label for="prop-target-group">Target</label>
            <input
              id="prop-target-group"
              type="text"
              value={selectedEl.target_group ?? ""}
              oninput={(e) => updateSelectedElement({ target_group: optionalText(e.currentTarget.value) })}
            />
          </div>
        </div>
      {/if}

      {#if selectedEl.type === "texture" || selectedEl.type === "progress"}
        <div class="prop-row">
          <label for="prop-asset">{selectedEl.type === "progress" ? "Source" : "Texture"}</label>
          <select
            id="prop-asset"
            value={selectedEl.asset ?? ""}
            onchange={(e) => updateProp("asset", e.currentTarget.value || undefined)}
          >
            <option value="">(none)</option>
            {#each project.assets as a (a)}
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
          <button class="secondary-btn" onclick={() => openUvEditor("uv")} disabled={project.assets.length === 0}>
            Pick Region...
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

      {#if selectedEl.type === "text" || selectedEl.type === "button" || selectedEl.type === "toggle_button"}
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
            {#each fontOptions as font (font.id)}
              <option value={font.id}>{font.id}</option>
            {/each}
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

      {#if selectedEl.type === "button" || selectedEl.type === "toggle_button"}
        <div class="prop-section">
          <div class="section-title">Button</div>
          <div class="prop-row">
            <label for="prop-tooltip">Tooltip</label>
            <input
              id="prop-tooltip"
              type="text"
              value={selectedEl.tooltip ?? ""}
              oninput={(e) => updateSelectedElement({ tooltip: optionalText(e.currentTarget.value) })}
            />
          </div>
          <div class="prop-row">
            <label for="prop-binding">Binding</label>
            <input
              id="prop-binding"
              type="text"
              value={selectedEl.binding ?? ""}
              oninput={(e) => updateSelectedElement({ binding: optionalText(e.currentTarget.value) })}
            />
          </div>
          <div class="prop-row">
            <label for="prop-icon">Icon</label>
            <select
              id="prop-icon"
              value={selectedEl.icon ?? ""}
              onchange={(e) => updateSelectedElement({ icon: optionalText(e.currentTarget.value), icon_uv: optionalText(e.currentTarget.value) ? selectedEl.icon_uv : null })}
            >
              <option value="">(none)</option>
              {#each project.assets as a (a)}
                <option value={a}>{a.replace("textures/", "").replace(".png", "")}</option>
              {/each}
            </select>
          </div>
          <div class="uv-grid">
            <label for="prop-icon-uv-x">Icon X</label>
            <input id="prop-icon-uv-x" type="number" min="0" value={selectedEl.icon_uv?.x ?? 0} oninput={(e) => updateIconUv("x", e.currentTarget.value)} />
            <label for="prop-icon-uv-y">Icon Y</label>
            <input id="prop-icon-uv-y" type="number" min="0" value={selectedEl.icon_uv?.y ?? 0} oninput={(e) => updateIconUv("y", e.currentTarget.value)} />
            <label for="prop-icon-uv-width">Icon W</label>
            <input id="prop-icon-uv-width" type="number" min="1" value={selectedEl.icon_uv?.width ?? 16} oninput={(e) => updateIconUv("width", e.currentTarget.value)} />
            <label for="prop-icon-uv-height">Icon H</label>
            <input id="prop-icon-uv-height" type="number" min="1" value={selectedEl.icon_uv?.height ?? 16} oninput={(e) => updateIconUv("height", e.currentTarget.value)} />
          </div>
          <button class="secondary-btn" onclick={() => updateSelectedElement({ icon_uv: null })}>
            Clear Icon UV
          </button>
          <button class="secondary-btn" onclick={() => openUvEditor("icon_uv")} disabled={project.assets.length === 0}>
            Pick Icon Region...
          </button>
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

{#if selectedEl && uvEditorTarget}
  <UvEditorDialog
    title={uvEditorTarget === "icon_uv" ? "Pick Button Icon Region" : "Pick Texture Region"}
    assets={project.assets}
    asset={uvEditorTarget === "icon_uv" ? selectedEl.icon ?? null : selectedEl.asset ?? null}
    uv={uvEditorTarget === "icon_uv" ? selectedEl.icon_uv ?? null : selectedEl.uv ?? null}
    onapply={applyUvSelection}
    onclear={clearUvSelection}
    onclose={() => uvEditorTarget = null}
  />
{/if}

<style>
  .properties {
    padding: 10px;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted-text);
    margin-bottom: 8px;
  }

  .muted {
    color: var(--muted-text);
    font-size: 12px;
  }

  .props-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .project-form {
    margin-bottom: 8px;
  }

  .prop-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .prop-label,
  .prop-row label {
    font-size: 11px;
    color: var(--muted-text);
    width: 55px;
    flex-shrink: 0;
  }

  .prop-value {
    font-size: 12px;
    color: var(--muted-text);
  }

  .prop-value.mono {
    font-family: monospace;
    font-size: 11px;
  }

  .prop-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-top: 1px solid var(--border);
    padding-top: 8px;
    margin-top: 2px;
  }

  .section-title {
    color: var(--muted-text);
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
    background: var(--app-bg);
    border: 1px solid var(--border);
    color: var(--text);
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
    border-color: var(--accent);
  }

  input[type="checkbox"] {
    accent-color: var(--accent);
  }

  select {
    cursor: pointer;
  }

  .divider {
    border: none;
    border-top: 1px solid var(--border);
    margin: 8px 0;
  }

  .delete-btn {
    background: transparent;
    border: 1px solid var(--danger);
    color: var(--danger);
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 2px;
    font-family: inherit;
    width: 100%;
  }

  .secondary-btn {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--muted-text);
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 2px;
    font-family: inherit;
    width: 100%;
  }

  .secondary-btn:hover {
    background: var(--surface-raised);
    color: var(--text);
  }

  .delete-btn:hover {
    background: var(--danger);
    color: var(--app-bg);
  }
</style>
