<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import type { AttachedRegion, Element, Group, SemanticGroup } from "../types";

  type LayerRow =
    | { kind: "group"; id: string; label: string; meta: string; elements: Element[] }
    | { kind: "attached_region"; region: AttachedRegion; meta: string; elements: Element[] }
    | { kind: "element"; element: Element; meta: string };

  let collapsedGroups = $state<Set<string>>(new Set());

  function displayId(id: string): string {
    return id.length > 26 ? `${id.slice(0, 23)}...` : id;
  }

  function elementMeta(el: Element): string {
    const layer = el.layer ?? "background";
    if (el.type === "slot" || el.type === "virtual_slot_cell") {
      return `${el.type} · ${layer}${el.slot_role ? ` · ${el.slot_role}` : ""}${el.slot_index !== undefined && el.slot_index !== null ? ` · #${el.slot_index}` : ""}`;
    }
    if (el.type === "progress") {
      return `${el.type} · ${layer}${el.direction ? ` · ${el.direction}` : ""}`;
    }
    if (el.width || el.height) {
      return `${el.type} · ${layer} · ${el.width ?? "?"}x${el.height ?? "?"}`;
    }
    return `${el.type} · ${layer} · ${el.x},${el.y}`;
  }

  function groupMeta(group: Group | SemanticGroup, count: number): string {
    if ("kind" in group) return `${group.kind} · ${count} elements`;
    return `${count} elements`;
  }

  function attachedRegionMeta(region: AttachedRegion, count: number): string {
    return `${region.anchor} · ${region.width}x${region.height} · ${count} elements`;
  }

  function iconForElement(el: Element): string {
    switch (el.type) {
      case "slot": return "◻";
      case "texture": return "▣";
      case "progress": return "→";
      case "text": return "T";
      case "fluid_tank": return "▥";
      case "energy_bar": return "⚡";
      case "button": return "▭";
      case "toggle_button": return "◉";
      default: return "•";
    }
  }

  function toggleGroup(id: string) {
    const next = new Set(collapsedGroups);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    collapsedGroups = next;
  }

  function groupedRows(): LayerRow[] {
    const consumed = new Set<string>();
    const rows: LayerRow[] = [];
    const reversed = [...project.elements].reverse();

    for (const region of project.attachedRegions) {
      const elements = project.elements.filter(element => element.attached_region === region.id);
      for (const element of elements) consumed.add(element.id);
      rows.push({ kind: "attached_region", region, meta: attachedRegionMeta(region, elements.length), elements });
    }

    for (const semantic of project.semanticGroups) {
      const elements = project.elements.filter(element => element.inventory_group === semantic.id);
      if (elements.length >= 3) {
        for (const element of elements) consumed.add(element.id);
        rows.push({ kind: "group", id: semantic.id, label: semantic.id, meta: groupMeta(semantic, elements.length), elements });
      }
    }

    for (const group of project.groups) {
      if (rows.some(row => row.kind === "group" && row.id === group.id)) continue;
      const elements = group.elements.map(id => project.elementById(id)).filter(element => element !== undefined);
      if (elements.length >= 3) {
        for (const element of elements) consumed.add(element.id);
        rows.push({ kind: "group", id: group.id, label: group.id, meta: groupMeta(group, elements.length), elements });
      }
    }

    for (const element of reversed) {
      if (consumed.has(element.id)) continue;
      rows.push({ kind: "element", element, meta: elementMeta(element) });
    }

    return rows;
  }

  function toggleVisibility(el: Element) {
    project.updateElement(el.id, { visible: !(el.visible ?? true) });
  }

  let selectedGroupIds = $derived.by(() => {
    void editor.selectionRevision;
    const ids = new Set<string>();
    for (const id of editor.selectedIds) {
      const group = project.groupForElement(id);
      if (group) ids.add(group.id);
    }
    return ids;
  });

  let selectedElementId = $derived.by(() => {
    void editor.selectionRevision;
    return editor.selectedElementId;
  });

  let selectedAttachedRegionId = $derived.by(() => {
    void editor.regionSelectionRevision;
    return editor.selectedAttachedRegionId;
  });

  let selectedCount = $derived.by(() => {
    void editor.selectionRevision;
    return editor.selectedIds.size;
  });

  let rows = $derived.by(() => {
    void project.revision;
    void project.elements.length;
    void project.attachedRegions.length;
    void project.groups.length;
    void project.semanticGroups.length;
    void editor.selectionRevision;
    void editor.regionSelectionRevision;
    return groupedRows();
  });
</script>

<aside class="layers">
  <h3>Layers ({project.elements.length})</h3>
  <div class="group-actions">
    <button
      disabled={selectedCount < 2}
      onclick={() => project.createGroup(editor.selectedIds)}
      title="Group selected elements"
    >
      Group
    </button>
    <button
      disabled={selectedGroupIds.size === 0}
      onclick={() => project.ungroupElements(editor.selectedIds)}
      title="Ungroup selected element groups"
    >
      Ungroup
    </button>
  </div>

  {#if project.elements.length === 0 && project.attachedRegions.length === 0}
    <p class="muted">No elements</p>
  {:else}
    {#snippet elementRow(el: Element, nested = false)}
      {@const idx = project.elements.indexOf(el)}
      {@const isBackmost = idx === 0}
      {@const isFrontmost = idx === project.elements.length - 1}
      <div class="layer-row" class:nested>
        <button
          class="layer-item"
          class:selected={selectedElementId === el.id}
          class:hidden-el={!(el.visible ?? true)}
          onclick={() => editor.selectElement(el.id)}
        >
          <span class="layer-icon">{iconForElement(el)}</span>
          <span class="layer-text">
            <span class="layer-title">{displayId(el.id)}</span>
            <span class="layer-meta">{elementMeta(el)}</span>
          </span>
        </button>
        <div class="layer-actions">
          <button
            class="reorder-btn"
            disabled={isBackmost}
            onclick={() => project.moveElementDown(el.id)}
            title="Move down (send backward)"
          >↓</button>
          <button
            class="reorder-btn"
            disabled={isFrontmost}
            onclick={() => project.moveElementUp(el.id)}
            title="Move up (bring forward)"
          >↑</button>
          <button
            class="visibility-btn"
            onclick={() => toggleVisibility(el)}
            title={el.visible === false ? "Show" : "Hide"}
          >
            {el.visible === false ? "◌" : "●"}
          </button>
        </div>
      </div>
    {/snippet}

    <div class="layer-list">
      {#each rows as row}
        {#if row.kind === "group"}
          <div class="group-row">
            <button class="group-main" onclick={() => toggleGroup(row.id)}>
              <span class="disclosure">{collapsedGroups.has(row.id) ? "▸" : "▾"}</span>
              <span class="group-text">
                <span class="group-title">{displayId(row.label)}</span>
                <span class="group-meta">{row.meta}</span>
              </span>
            </button>
          </div>
          {#if !collapsedGroups.has(row.id)}
            {#each row.elements as el (el.id)}
              {@render elementRow(el, true)}
            {/each}
          {/if}
        {:else if row.kind === "attached_region"}
          <div class="group-row">
            <button
              class="group-main"
              class:selected={selectedAttachedRegionId === row.region.id}
              class:hidden-el={row.region.visible === false}
              onclick={() => {
                editor.selectAttachedRegion(row.region.id);
                toggleGroup(`attached:${row.region.id}`);
              }}
            >
              <span class="disclosure">{collapsedGroups.has(`attached:${row.region.id}`) ? "▸" : "▾"}</span>
              <span class="group-text">
                <span class="group-title">{displayId(row.region.id)}</span>
                <span class="group-meta">{row.meta}</span>
              </span>
            </button>
          </div>
          {#if !collapsedGroups.has(`attached:${row.region.id}`)}
            {#each row.elements as el (el.id)}
              {@render elementRow(el, true)}
            {/each}
          {/if}
        {:else}
          {@render elementRow(row.element)}
        {/if}
      {/each}
    </div>
  {/if}
</aside>

<style>
  .layers {
    padding: 8px;
    min-height: 0;
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

  .group-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
    margin-bottom: 8px;
  }

  .group-actions button {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--muted-text);
    padding: 4px 6px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 2px;
    font-family: inherit;
  }

  .group-actions button:hover:not(:disabled) {
    background: var(--surface-raised);
    color: var(--text);
  }

  .group-actions button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .layer-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow-y: auto;
    max-height: none;
  }

  .layer-row {
    display: flex;
    align-items: stretch;
    gap: 1px;
  }

  .group-main,
  .layer-item {
    min-width: 0;
  }

  .group-main {
    width: 100%;
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    gap: 6px;
    align-items: center;
    background: var(--surface-raised);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 5px 6px;
    text-align: left;
    cursor: pointer;
    font: inherit;
  }

  .group-main.selected {
    color: var(--accent);
    border-color: var(--accent);
  }

  .group-main.hidden-el {
    opacity: 0.35;
  }

  .group-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .group-title,
  .layer-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group-meta,
  .layer-meta {
    color: var(--muted-text);
    font-size: 10px;
  }

  .layer-item {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr);
    align-items: center;
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted-text);
    padding: 4px 6px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 2px;
    font-family: monospace;
    text-align: left;
    flex: 1;
    min-height: 38px;
  }

  .layer-item:hover {
    background: var(--surface-raised);
  }

  .layer-item.selected {
    background: var(--surface-raised);
    color: var(--accent);
    border-color: var(--accent);
  }

  .layer-item.hidden-el {
    opacity: 0.35;
  }

  .layer-icon {
    font-size: 12px;
    width: 18px;
    text-align: center;
  }

  .layer-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .layer-row.nested {
    padding-left: 12px;
  }

  .layer-actions {
    display: flex;
    gap: 1px;
  }

  .reorder-btn, .visibility-btn {
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted-text);
    padding: 0 5px;
    font-size: 10px;
    cursor: pointer;
    border-radius: 2px;
    font-family: monospace;
    min-width: 22px;
  }

  .reorder-btn:hover:not(:disabled), .visibility-btn:hover {
    background: var(--surface-raised);
    color: var(--muted-text);
  }

  .reorder-btn:disabled {
    opacity: 0.2;
    cursor: default;
  }

  .visibility-btn {
    font-size: 10px;
  }
</style>
