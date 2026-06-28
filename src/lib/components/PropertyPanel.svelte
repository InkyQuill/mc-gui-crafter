<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import * as api from "../api";
  import { readableError, status } from "../stores/status.svelte";
  import UvEditorDialog from "./UvEditorDialog.svelte";
  import type { AttachedRegion, AttachedRegionAnchor, AttachedRegionState, CodegenMode, Element, NineSlice, Size, SlotRole, TextureRenderMode, UvRect } from "../types";

  type UvTarget = "uv" | "icon_uv";

  let uvEditorTarget = $state<UvTarget | null>(null);
  let editingNineSlice = $state(false);
  let lastLoggedSizeMismatch = "";
  let selectedElementId = $derived.by(() => {
    void editor.selectionRevision;
    return editor.selectedElementId;
  });
  let selectedEl = $derived(selectedElementId ? project.effectiveElementById(selectedElementId) : null);
  let selectedElements = $derived.by(() => {
    void editor.selectionRevision;
    return [...editor.selectedIds]
      .map(id => project.effectiveElementById(id))
      .filter((element): element is Element => Boolean(element));
  });
  let hasMultiSelection = $derived(selectedElements.length > 1);
  let selectedSlots = $derived(selectedElements.filter(element => element.type === "slot" || element.type === "virtual_slot_cell"));
  let selectedTargetSize = $derived.by((): Size | null => {
    if (!selectedEl) return null;
    if (selectedEl.width === undefined || selectedEl.height === undefined) return null;
    return {
      width: selectedEl.width,
      height: selectedEl.height,
    };
  });
  let selectedGroup = $derived(selectedElementId ? project.groupForElement(selectedElementId) : null);
  let selectedRegion = $derived.by(() => {
    void editor.regionSelectionRevision;
    const id = editor.selectedAttachedRegionId;
    return id ? project.effectiveAttachedRegionById(id) : null;
  });
  let visibleContentSizeMismatch = $derived.by(() => {
    const bounds = project.visibleContentBounds;
    if (!project.isOpen || !bounds) return null;
    const sizeMismatch = bounds.width !== project.guiSize.width || bounds.height !== project.guiSize.height;
    const originMismatch = bounds.x !== 0 || bounds.y !== 0;
    if (!sizeMismatch && !originMismatch) return null;
    return {
      bounds,
      canResizeOnly: bounds.x === 0 && bounds.y === 0,
    };
  });
  let fontOptions = $derived.by(() => {
    const options = project.fonts.filter((font, index, fonts) => fonts.findIndex(candidate => candidate.id === font.id) === index);
    if (!options.some(font => font.id === "minecraft:default")) {
      options.unshift({ id: "minecraft:default", source: { type: "minecraft" } });
    }
    return options;
  });

  $effect(() => {
    const mismatch = visibleContentSizeMismatch;
    const key = mismatch
      ? `${project.activeProjectId}:${mismatch.bounds.x}:${mismatch.bounds.y}:${mismatch.bounds.width}:${mismatch.bounds.height}:${project.guiSize.width}:${project.guiSize.height}`
      : "";
    if (!mismatch) {
      lastLoggedSizeMismatch = "";
      return;
    }
    if (key === lastLoggedSizeMismatch) return;
    lastLoggedSizeMismatch = key;
    void api.appendSessionLog({
      level: "warning",
      source: "ui",
      category: "validation",
      message: "Project size differs from visible content bounds",
      details: {
        project_id: project.activeProjectId,
        project_size: project.guiSize,
        visible_content_bounds: mismatch.bounds,
        can_resize_only: mismatch.canResizeOnly,
      },
    });
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
  const attachedRegionAnchors: AttachedRegionAnchor[] = ["left", "right", "top", "bottom", "free"];
  const attachedRegionStates: AttachedRegionState[] = ["static", "toggleable"];

  function updateProp(key: string, value: unknown) {
    if (!editor.selectedElementId) return;
    project.updateElement(editor.selectedElementId, { [key]: value });
  }

  function updateTextureProp(key: "asset", value: string | null): void;
  function updateTextureProp(key: "uv", value: UvRect | null): void;
  function updateTextureProp(key: "asset" | "uv", value: string | UvRect | null) {
    const changes: api.ElementChanges = key === "asset"
      ? { asset: value as string | null }
      : { uv: value as UvRect | null };
    updateProp(key, changes[key]);
  }

  function updateSelectedElement(changes: Partial<Element>) {
    if (!selectedEl) return;
    project.updateElement(selectedEl.id, changes);
  }

  function updateRegion(id: string, changes: Partial<AttachedRegion>) {
    void project.updateAttachedRegion(id, changes);
  }

  function elementOverrideMarker(field: "visible" | "x" | "y" | "width" | "height" | "attached_region" | "layer"): string {
    return selectedEl && project.hasElementOverride(selectedEl.id, field) ? "*" : "";
  }

  function regionOverrideMarker(field: "visible" | "x" | "y" | "width" | "height"): string {
    return selectedRegion && project.hasAttachedRegionOverride(selectedRegion.id, field) ? "*" : "";
  }

  function numberValue(value: string, fallback = 0): number {
    const parsed = Number.parseInt(value, 10);
    return Number.isFinite(parsed) ? parsed : fallback;
  }

  type MixedValue<T> = { mixed: true; value: null } | { mixed: false; value: T | null };

  function structurallyEqual(left: unknown, right: unknown): boolean {
    if (Object.is(left, right)) return true;
    if (left === null || right === null) return left === right;
    if (typeof left !== "object" || typeof right !== "object") return false;
    return JSON.stringify(left) === JSON.stringify(right);
  }

  function mixedValue<T>(elements: Element[], read: (element: Element) => T): MixedValue<T> {
    if (elements.length === 0) return { mixed: false, value: null };
    const value = read(elements[0]);
    return elements.every(element => structurallyEqual(read(element), value))
      ? { mixed: false, value }
      : { mixed: true, value: null };
  }

  function mixedSelectValue<T extends string | null | undefined>(field: MixedValue<T>): string {
    return field.mixed ? "__mixed__" : field.value ?? "";
  }

  function updateSelectedElements(changes: api.ElementChanges) {
    updateSelectedElementsWhere(() => true, changes);
  }

  function updateSelectedElementsWhere(predicate: (element: Element) => boolean, changes: api.ElementChanges) {
    const patches = selectedElements
      .filter(predicate)
      .map(element => ({ id: element.id, changes }));
    void project.updateElements(patches);
  }

  let multiLayer = $derived(mixedValue(selectedElements, element => element.layer ?? "background"));
  let multiVisible = $derived(mixedValue(selectedElements, element => element.visible ?? true));
  let multiAttachedRegion = $derived(mixedValue(selectedElements, element => element.attached_region ?? ""));
  let multiSlotAsset = $derived(mixedValue(selectedSlots, element => element.asset ?? ""));
  let multiSlotRole = $derived(mixedValue(selectedSlots, element => element.slot_role ?? ""));
  let multiInventoryGroup = $derived(mixedValue(selectedSlots, element => element.inventory_group ?? ""));
  let multiScrollBinding = $derived(mixedValue(selectedSlots, element => element.scroll_binding ?? ""));

  function updateUv(key: "x" | "y" | "width" | "height", value: string) {
    if (!selectedEl) return;
    const next = {
      x: selectedEl.uv?.x ?? 0,
      y: selectedEl.uv?.y ?? 0,
      width: selectedEl.uv?.width ?? selectedEl.width ?? selectedEl.size ?? 16,
      height: selectedEl.uv?.height ?? selectedEl.height ?? selectedEl.size ?? 16,
      [key]: Math.max(key === "width" || key === "height" ? 1 : 0, numberValue(value)),
    };
    updateTextureProp("uv", next);
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

  function applyUvSelection(asset: string, value: UvRect | NineSlice | null) {
    if (!selectedEl || !uvEditorTarget) return;
    const uv = value as UvRect | null;
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
      updateTextureProp("uv", null);
    }
    uvEditorTarget = null;
  }

  function openNineSliceEditor() {
    if (!selectedEl?.asset) return;
    void project.ensureAssetDataUrl(selectedEl.asset);
    editingNineSlice = true;
  }

  function applyNineSlice(asset: string, value: UvRect | NineSlice | null) {
    if (!selectedEl || !value) return;
    updateSelectedElement({
      asset,
      render_mode: "nine_slice",
      nine_slice: value as NineSlice,
    });
    editingNineSlice = false;
  }

  function clearNineSlice() {
    updateSelectedElement({ nine_slice: null });
    editingNineSlice = false;
  }

  function useAssetGuides() {
    updateSelectedElement({ nine_slice: null });
  }

  async function resizeProjectToVisibleContent() {
    const mismatch = visibleContentSizeMismatch;
    if (!mismatch?.canResizeOnly) return;
    try {
      await project.resizeProject(mismatch.bounds.width, mismatch.bounds.height);
      status.success(`Project resized to ${mismatch.bounds.width}x${mismatch.bounds.height}.`);
    } catch (error) {
      status.error(`Resize failed: ${readableError(error)}`);
    }
  }

  async function updateMainGuiCenterAxis(axis: "x" | "y", value: string) {
    try {
      await project.updateMainGuiCenter({
        ...project.mainGuiCenter,
        [axis]: numberValue(value, project.mainGuiCenter[axis]),
      });
    } catch (error) {
      status.error(`Center axes update failed: ${readableError(error)}`);
    }
  }

  async function resetMainGuiCenter() {
    try {
      await project.updateMainGuiCenter({
        x: Math.floor(project.guiSize.width / 2),
        y: Math.floor(project.guiSize.height / 2),
      });
    } catch (error) {
      status.error(`Center axes reset failed: ${readableError(error)}`);
    }
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
      <div class="prop-row">
        <label for="project-center-x">Center X</label>
        <input
          id="project-center-x"
          type="number"
          step="1"
          value={project.mainGuiCenter.x}
          onchange={(event) => updateMainGuiCenterAxis("x", event.currentTarget.value)}
        />
      </div>
      <div class="prop-row">
        <label for="project-center-y">Center Y</label>
        <input
          id="project-center-y"
          type="number"
          step="1"
          value={project.mainGuiCenter.y}
          onchange={(event) => updateMainGuiCenterAxis("y", event.currentTarget.value)}
        />
      </div>
      <button class="secondary-btn" onclick={resetMainGuiCenter}>
        Reset center axes
      </button>
      </div>

      {#if visibleContentSizeMismatch}
        <div class="project-warning">
          <div class="warning-title">Project size mismatch</div>
          <p>
            Visible content is {visibleContentSizeMismatch.bounds.width}x{visibleContentSizeMismatch.bounds.height}
            at {visibleContentSizeMismatch.bounds.x},{visibleContentSizeMismatch.bounds.y};
            project size is {project.guiSize.width}x{project.guiSize.height}.
          </p>
          <p>
            Exported textures use visible bounds, while generated screen code centers the main GUI on the red axes.
          </p>
          {#if visibleContentSizeMismatch.canResizeOnly}
            <button class="secondary-btn" onclick={resizeProjectToVisibleContent}>
              Resize project to {visibleContentSizeMismatch.bounds.width}x{visibleContentSizeMismatch.bounds.height}
            </button>
          {:else}
            <p>
              Move visible content by {-visibleContentSizeMismatch.bounds.x},{-visibleContentSizeMismatch.bounds.y}, then resize to
              {visibleContentSizeMismatch.bounds.width}x{visibleContentSizeMismatch.bounds.height}.
            </p>
          {/if}
        </div>
      {/if}

      <hr class="divider" />
  {/if}

  {#if hasMultiSelection}
    <div class="props-form">
      <div class="prop-row">
        <span class="prop-label">Selection</span>
        <span class="prop-value">{selectedElements.length} objects</span>
      </div>
      <div class="prop-row">
        <label for="multi-prop-layer">Layer</label>
        <select
          id="multi-prop-layer"
          value={mixedSelectValue(multiLayer)}
          onchange={(e) => updateSelectedElements({ layer: e.currentTarget.value as api.ElementChanges["layer"] })}
        >
          {#if multiLayer.mixed}
            <option value="__mixed__" disabled>Mixed</option>
          {/if}
          <option value="background">Background</option>
          <option value="overlay">Overlay</option>
          <option value="animatable">Animatable</option>
        </select>
      </div>
      <div class="prop-row">
        <label for="multi-prop-attached-region">Region</label>
        <select
          id="multi-prop-attached-region"
          value={mixedSelectValue(multiAttachedRegion)}
          onchange={(e) => updateSelectedElements({ attached_region: e.currentTarget.value || null })}
        >
          {#if multiAttachedRegion.mixed}
            <option value="__mixed__" disabled>Mixed</option>
          {/if}
          <option value="">(none)</option>
          {#each project.effectiveAttachedRegions as region (region.id)}
            <option value={region.id}>{region.id}</option>
          {/each}
        </select>
      </div>
      <div class="prop-row">
        <label for="multi-prop-visible">Visible</label>
        <input
          id="multi-prop-visible"
          type="checkbox"
          checked={!multiVisible.mixed && multiVisible.value === true}
          indeterminate={multiVisible.mixed}
          onchange={(e) => updateSelectedElements({ visible: e.currentTarget.checked })}
        />
      </div>

      {#if selectedSlots.length === selectedElements.length}
        <div class="prop-section">
          <div class="section-title">Slot</div>
          <div class="prop-row">
            <label for="multi-prop-slot-asset">Background</label>
            <select
              id="multi-prop-slot-asset"
              value={mixedSelectValue(multiSlotAsset)}
              onchange={(e) => updateSelectedElements({ asset: e.currentTarget.value || null })}
            >
              {#if multiSlotAsset.mixed}
                <option value="__mixed__" disabled>Mixed</option>
              {/if}
              <option value="">(none)</option>
              {#each project.assets as a (a)}
                <option value={a}>{a.replace("textures/", "").replace(".png", "")}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <label for="multi-prop-slot-role">Role</label>
            <select
              id="multi-prop-slot-role"
              value={mixedSelectValue(multiSlotRole)}
              onchange={(e) => updateSelectedElements({ slot_role: (e.currentTarget.value || null) as SlotRole | null })}
            >
              {#if multiSlotRole.mixed}
                <option value="__mixed__" disabled>Mixed</option>
              {/if}
              <option value="">(none)</option>
              {#each slotRoleOptions as role (role)}
                <option value={role}>{role}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <label for="multi-prop-inventory-group">Group</label>
            <input
              id="multi-prop-inventory-group"
              type="text"
              value={multiInventoryGroup.mixed ? "" : multiInventoryGroup.value ?? ""}
              placeholder={multiInventoryGroup.mixed ? "Mixed" : ""}
              oninput={(e) => updateSelectedElements({ inventory_group: optionalText(e.currentTarget.value) })}
            />
          </div>
          <div class="prop-row">
            <label for="multi-prop-scroll-binding">Scroll</label>
            <input
              id="multi-prop-scroll-binding"
              type="text"
              value={multiScrollBinding.mixed ? "" : multiScrollBinding.value ?? ""}
              placeholder={multiScrollBinding.mixed ? "Mixed" : ""}
              oninput={(e) => updateSelectedElements({ scroll_binding: optionalText(e.currentTarget.value) })}
            />
          </div>
          <button class="secondary-btn" onclick={() => updateSelectedElements({ uv: null })}>
            Clear UV
          </button>
        </div>
      {/if}
    </div>
  {:else if selectedEl}
    <div class="props-form">
      <div class="prop-row">
        <span class="prop-label">ID</span>
        <span class="prop-value mono">{selectedEl.id}</span>
        {#if project.hasElementOverride(selectedEl.id)}
          <button class="override-clear-btn" title="Clear state overrides" onclick={() => project.clearElementOverride(selectedEl.id)}>×</button>
        {:else if project.isElementStateOwned(selectedEl.id)}
          <span class="state-marker">Owned</span>
        {/if}
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
        <label for="prop-x">X{elementOverrideMarker("x")}</label>
        <input
          id="prop-x"
          type="number"
          value={selectedEl.x}
          oninput={(e) => updateProp("x", parseInt(e.currentTarget.value) || 0)}
        />
        {#if project.hasElementOverride(selectedEl.id, "x")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearElementOverride(selectedEl.id, "x")}>×</button>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-y">Y{elementOverrideMarker("y")}</label>
        <input
          id="prop-y"
          type="number"
          value={selectedEl.y}
          oninput={(e) => updateProp("y", parseInt(e.currentTarget.value) || 0)}
        />
        {#if project.hasElementOverride(selectedEl.id, "y")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearElementOverride(selectedEl.id, "y")}>×</button>
        {/if}
      </div>

      <div class="prop-row">
        <label for="prop-layer">Layer{elementOverrideMarker("layer")}</label>
        <select
          id="prop-layer"
          value={selectedEl.layer ?? "background"}
          onchange={(e) => updateProp("layer", e.currentTarget.value)}
        >
          <option value="background">Background</option>
          <option value="overlay">Overlay</option>
          <option value="animatable">Animatable</option>
        </select>
        {#if project.hasElementOverride(selectedEl.id, "layer")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearElementOverride(selectedEl.id, "layer")}>×</button>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-attached-region">Region{elementOverrideMarker("attached_region")}</label>
        <select
          id="prop-attached-region"
          value={selectedEl.attached_region ?? ""}
          onchange={(e) => updateSelectedElement({ attached_region: e.currentTarget.value || null })}
        >
          <option value="">(none)</option>
          {#each project.effectiveAttachedRegions as region (region.id)}
            <option value={region.id}>{region.id}</option>
          {/each}
        </select>
        {#if project.hasElementOverride(selectedEl.id, "attached_region")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearElementOverride(selectedEl.id, "attached_region")}>×</button>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-visible">Visible{elementOverrideMarker("visible")}</label>
        <input
          id="prop-visible"
          type="checkbox"
          checked={selectedEl.visible ?? true}
          onchange={(e) => updateSelectedElement({ visible: e.currentTarget.checked })}
        />
        {#if project.hasElementOverride(selectedEl.id, "visible")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearElementOverride(selectedEl.id, "visible")}>×</button>
        {/if}
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
          <label for="prop-width">Width{elementOverrideMarker("width")}</label>
          <input
            id="prop-width"
            type="number"
            value={selectedEl.width ?? ""}
            oninput={(e) => updateProp("width", parseInt(e.currentTarget.value) || undefined)}
          />
          {#if project.hasElementOverride(selectedEl.id, "width")}
            <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearElementOverride(selectedEl.id, "width")}>×</button>
          {/if}
        </div>
        <div class="prop-row">
          <label for="prop-height">Height{elementOverrideMarker("height")}</label>
          <input
            id="prop-height"
            type="number"
            value={selectedEl.height ?? ""}
            oninput={(e) => updateProp("height", parseInt(e.currentTarget.value) || undefined)}
          />
          {#if project.hasElementOverride(selectedEl.id, "height")}
            <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearElementOverride(selectedEl.id, "height")}>×</button>
          {/if}
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

      {#if selectedEl.type === "texture" || selectedEl.type === "progress" || selectedEl.type === "slot" || selectedEl.type === "virtual_slot_cell"}
        <div class="prop-row">
          <label for="prop-asset">{selectedEl.type === "progress" ? "Source" : selectedEl.type === "slot" || selectedEl.type === "virtual_slot_cell" ? "Background" : "Texture"}</label>
          <select
            id="prop-asset"
            value={selectedEl.asset ?? ""}
            onchange={(e) => updateTextureProp("asset", e.currentTarget.value || null)}
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
              value={selectedEl.uv?.width ?? selectedEl.width ?? selectedEl.size ?? 16}
              oninput={(e) => updateUv("width", e.currentTarget.value)}
            />
            <label for="prop-uv-height">H</label>
            <input
              id="prop-uv-height"
              type="number"
              min="1"
              value={selectedEl.uv?.height ?? selectedEl.height ?? selectedEl.size ?? 16}
              oninput={(e) => updateUv("height", e.currentTarget.value)}
            />
          </div>
          <button class="secondary-btn" onclick={() => updateTextureProp("uv", null)}>
            Clear UV
          </button>
          <button class="secondary-btn" onclick={() => openUvEditor("uv")} disabled={project.assets.length === 0}>
            Pick Region...
          </button>
        </div>
      {/if}

      {#if selectedEl.type === "texture"}
        <div class="prop-section">
          <div class="section-title">Texture Render</div>
          <div class="prop-row">
            <label for="prop-render-mode">Mode</label>
            <select
              id="prop-render-mode"
              value={selectedEl.render_mode ?? "plain"}
              onchange={(e) => updateSelectedElement({ render_mode: e.currentTarget.value as TextureRenderMode })}
            >
              <option value="plain">Plain</option>
              <option value="nine_slice">Nine Slice</option>
            </select>
          </div>
          {#if (selectedEl.render_mode ?? "plain") === "nine_slice"}
            <button class="secondary-btn" onclick={openNineSliceEditor} disabled={!selectedEl.asset || project.assets.length === 0}>
              Edit Guides...
            </button>
            <button class="secondary-btn" onclick={clearNineSlice}>
              Clear Guides
            </button>
            <button class="secondary-btn" onclick={useAssetGuides} disabled={!selectedEl.asset || !project.assetMetadata[selectedEl.asset]?.nine_slice}>
              Use Asset Guides
            </button>
          {/if}
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
  {:else if selectedRegion}
    <div class="props-form">
      <div class="prop-row">
        <span class="prop-label">ID</span>
        <span class="prop-value mono">{selectedRegion.id}</span>
        {#if project.hasAttachedRegionOverride(selectedRegion.id)}
          <button class="override-clear-btn" title="Clear state overrides" onclick={() => project.clearAttachedRegionOverride(selectedRegion.id)}>×</button>
        {:else if project.isAttachedRegionStateOwned(selectedRegion.id)}
          <span class="state-marker">Owned</span>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-region-anchor">Anchor</label>
        <select
          id="prop-region-anchor"
          value={selectedRegion.anchor}
          onchange={(e) => updateRegion(selectedRegion.id, { anchor: e.currentTarget.value as AttachedRegionAnchor })}
        >
          {#each attachedRegionAnchors as anchor (anchor)}
            <option value={anchor}>{anchor}</option>
          {/each}
        </select>
      </div>
      <div class="prop-row">
        <label for="prop-region-x">X{regionOverrideMarker("x")}</label>
        <input
          id="prop-region-x"
          type="number"
          value={selectedRegion.x}
          onchange={(e) => updateRegion(selectedRegion.id, { x: numberValue(e.currentTarget.value, selectedRegion.x) })}
        />
        {#if project.hasAttachedRegionOverride(selectedRegion.id, "x")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearAttachedRegionOverride(selectedRegion.id, "x")}>×</button>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-region-y">Y{regionOverrideMarker("y")}</label>
        <input
          id="prop-region-y"
          type="number"
          value={selectedRegion.y}
          onchange={(e) => updateRegion(selectedRegion.id, { y: numberValue(e.currentTarget.value, selectedRegion.y) })}
        />
        {#if project.hasAttachedRegionOverride(selectedRegion.id, "y")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearAttachedRegionOverride(selectedRegion.id, "y")}>×</button>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-region-width">Width{regionOverrideMarker("width")}</label>
        <input
          id="prop-region-width"
          type="number"
          min="1"
          value={selectedRegion.width}
          onchange={(e) => updateRegion(selectedRegion.id, { width: Math.max(1, numberValue(e.currentTarget.value, selectedRegion.width)) })}
        />
        {#if project.hasAttachedRegionOverride(selectedRegion.id, "width")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearAttachedRegionOverride(selectedRegion.id, "width")}>×</button>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-region-height">Height{regionOverrideMarker("height")}</label>
        <input
          id="prop-region-height"
          type="number"
          min="1"
          value={selectedRegion.height}
          onchange={(e) => updateRegion(selectedRegion.id, { height: Math.max(1, numberValue(e.currentTarget.value, selectedRegion.height)) })}
        />
        {#if project.hasAttachedRegionOverride(selectedRegion.id, "height")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearAttachedRegionOverride(selectedRegion.id, "height")}>×</button>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-region-visible">Visible{regionOverrideMarker("visible")}</label>
        <input
          id="prop-region-visible"
          type="checkbox"
          checked={selectedRegion.visible ?? true}
          onchange={(e) => updateRegion(selectedRegion.id, { visible: e.currentTarget.checked })}
        />
        {#if project.hasAttachedRegionOverride(selectedRegion.id, "visible")}
          <button class="override-clear-btn" title="Clear state override" onclick={() => project.clearAttachedRegionOverride(selectedRegion.id, "visible")}>×</button>
        {/if}
      </div>
      <div class="prop-row">
        <label for="prop-region-state">State</label>
        <select
          id="prop-region-state"
          value={selectedRegion.state}
          onchange={(e) => updateRegion(selectedRegion.id, { state: e.currentTarget.value as AttachedRegionState })}
        >
          {#each attachedRegionStates as state (state)}
            <option value={state}>{state}</option>
          {/each}
        </select>
      </div>
      <div class="prop-row">
        <label for="prop-region-kind">Kind</label>
        <input
          id="prop-region-kind"
          type="text"
          value={selectedRegion.kind ?? ""}
          onchange={(e) => updateRegion(selectedRegion.id, { kind: optionalText(e.currentTarget.value) })}
        />
      </div>
      <div class="prop-row">
        <label for="prop-region-semantic-group">Semantic</label>
        <input
          id="prop-region-semantic-group"
          type="text"
          value={selectedRegion.semantic_group ?? ""}
          onchange={(e) => updateRegion(selectedRegion.id, { semantic_group: optionalText(e.currentTarget.value) })}
        />
      </div>
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

{#if selectedEl && editingNineSlice && selectedEl.asset}
  <UvEditorDialog
    title="Edit Texture Guides"
    mode="nine_slice"
    assets={project.assets}
    asset={selectedEl.asset}
    nineSlice={selectedEl.nine_slice ?? null}
    targetSize={selectedTargetSize}
    onapply={applyNineSlice}
    onclear={clearNineSlice}
    onclose={() => editingNineSlice = false}
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

  .project-warning {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 8px 0;
    padding: 8px;
    border: 1px solid color-mix(in srgb, var(--accent) 60%, var(--border));
    border-radius: 4px;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .project-warning p {
    margin: 0;
    color: var(--muted-text);
    font-size: 11px;
    line-height: 1.35;
  }

  .warning-title {
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
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

  .state-marker {
    color: var(--accent);
    font-size: 9px;
    text-transform: uppercase;
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

  .override-clear-btn {
    flex: 0 0 22px;
    min-width: 22px;
    height: 22px;
    background: transparent;
    border: 1px solid var(--border);
    color: var(--accent);
    border-radius: 2px;
    cursor: pointer;
    font-family: monospace;
    font-size: 11px;
  }

  .override-clear-btn:hover {
    background: var(--surface-raised);
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
