<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import type { Element } from "../types";

  function getElementLabel(el: Element): string {
    const id = el.id.length > 18 ? el.id.substring(0, 15) + "..." : el.id;
    return `${el.type} ${id}`;
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

  let selectedCount = $derived.by(() => {
    void editor.selectionRevision;
    return editor.selectedIds.size;
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

  {#if project.elements.length === 0}
    <p class="muted">No elements</p>
  {:else}
    <div class="layer-list">
      {#each [...project.elements].reverse() as el}
        {@const idx = project.elements.indexOf(el)}
        {@const group = project.groupForElement(el.id)}
        {@const isLast = idx === 0}
        {@const isFirst = idx === project.elements.length - 1}
        <div class="layer-row">
          <button
            class="layer-item"
            class:selected={selectedElementId === el.id}
            class:hidden-el={!(el.visible ?? true)}
            onclick={() => editor.selectElement(el.id)}
          >
            <span class="layer-icon">
              {#if el.type === "slot"}◻
              {:else if el.type === "texture"}▣
              {:else if el.type === "progress"}→
              {:else if el.type === "text"}T
              {:else if el.type === "fluid_tank"}▥
              {:else if el.type === "energy_bar"}⚡
              {/if}
            </span>
            <span class="layer-label">{getElementLabel(el)}</span>
            {#if group}
              <span class="group-chip">{group.id.replace("group_", "#")}</span>
            {/if}
            <span class="layer-coords">{el.x},{el.y}</span>
          </button>
          <div class="layer-actions">
            <button
              class="reorder-btn"
              disabled={isFirst}
              onclick={() => project.moveElementDown(el.id)}
              title="Move down (send backward)"
            >↓</button>
            <button
              class="reorder-btn"
              disabled={isLast}
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
      {/each}
    </div>
  {/if}
</aside>

<style>
  .layers {
    padding: 10px;
    border-top: 1px solid var(--border);
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
    gap: 1px;
    max-height: 260px;
    overflow-y: auto;
  }

  .layer-row {
    display: flex;
    align-items: stretch;
    gap: 1px;
  }

  .layer-item {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted-text);
    padding: 3px 6px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 2px;
    font-family: monospace;
    text-align: left;
    flex: 1;
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
    width: 16px;
    text-align: center;
  }

  .layer-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .layer-coords {
    color: var(--muted-text);
    font-size: 10px;
  }

  .group-chip {
    color: var(--muted-text);
    border: 1px solid var(--border);
    border-radius: 2px;
    padding: 0 3px;
    font-size: 10px;
    max-width: 54px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
