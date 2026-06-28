<script lang="ts">
  import {
    ArrowDown,
    ArrowRight,
    ArrowUp,
    BringToFront,
    Droplets,
    Eye,
    EyeOff,
    Image,
    Layers,
    MousePointerClick,
    PanelTop,
    SendToBack,
    Square,
    ToggleLeft,
    Type,
    X,
    Zap,
  } from "@lucide/svelte";
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import type { AttachedRegion, Element, Group, SemanticGroup } from "../types";

  type LayerRow =
    | { kind: "group"; id: string; label: string; meta: string; elements: Element[] }
    | { kind: "attached_region"; region: AttachedRegion; meta: string; elements: Element[] }
    | { kind: "element"; element: Element; meta: string };

  let collapsedGroups = $state<Set<string>>(new Set());
  let contextMenu = $state<{ element: Element; x: number; y: number } | null>(null);

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
    return `${region.anchor} · ${region.state} · ${count} elements`;
  }

  function iconForElement(el: Element) {
    switch (el.type) {
      case "slot":
      case "virtual_slot_cell": return Square;
      case "texture": return Image;
      case "progress": return ArrowRight;
      case "text": return Type;
      case "fluid_tank": return Droplets;
      case "energy_bar": return Zap;
      case "button": return MousePointerClick;
      case "toggle_button": return ToggleLeft;
      case "panel": return PanelTop;
      default: return Layers;
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
    const effectiveElements = project.effectiveElements;
    const effectiveRegions = project.effectiveAttachedRegions;
    const reversed = [...effectiveElements].reverse();

    for (const region of effectiveRegions) {
      const elements = effectiveElements.filter(element => element.attached_region === region.id);
      if (elements.length === 0) continue;
      for (const element of elements) consumed.add(element.id);
      rows.push({ kind: "attached_region", region, meta: attachedRegionMeta(region, elements.length), elements });
    }

    for (const semantic of project.semanticGroups) {
      const elements = effectiveElements.filter(element => element.inventory_group === semantic.id);
      if (elements.length >= 3) {
        for (const element of elements) consumed.add(element.id);
        rows.push({ kind: "group", id: semantic.id, label: semantic.id, meta: groupMeta(semantic, elements.length), elements });
      }
    }

    for (const group of project.groups) {
      if (rows.some(row => row.kind === "group" && row.id === group.id)) continue;
      const elements = group.elements.map(id => project.effectiveElementById(id)).filter(element => element !== undefined);
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

  function stateBadgeForElement(el: Element): string | null {
    if (project.hasElementOverride(el.id)) return "Override";
    if (project.isElementStateOwned(el.id)) return "Owned";
    return null;
  }

  function stateBadgeForRegion(region: AttachedRegion): string | null {
    if (project.hasAttachedRegionOverride(region.id)) return "Override";
    if (project.isAttachedRegionStateOwned(region.id)) return "Owned";
    return null;
  }

  function rowKey(row: LayerRow): string {
    if (row.kind === "element") return `element:${row.element.id}`;
    if (row.kind === "attached_region") return `attached_region:${row.region.id}`;
    return `group:${row.id}`;
  }

  function visibleElementIds(layerRows: LayerRow[]): string[] {
    const ids: string[] = [];
    for (const row of layerRows) {
      if (row.kind === "element") {
        ids.push(row.element.id);
      } else if (row.kind === "group" && !collapsedGroups.has(row.id)) {
        ids.push(...row.elements.map(element => element.id));
      } else if (row.kind === "attached_region" && !collapsedGroups.has(`attached:${row.region.id}`)) {
        ids.push(...row.elements.map(element => element.id));
      }
    }
    return ids;
  }

  function selectElementFromList(id: string, event: MouseEvent | KeyboardEvent) {
    if (event.shiftKey) {
      editor.selectElementRange(visibleElementIds(rows), id, event.ctrlKey || event.metaKey);
    } else {
      editor.selectElement(id, event.ctrlKey || event.metaKey);
    }
  }

  function selectElementFromKeyboard(id: string, event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    selectElementFromList(id, event);
  }

  function openElementContextMenu(el: Element, event: MouseEvent) {
    event.preventDefault();
    editor.selectElement(el.id, event.ctrlKey || event.metaKey);
    contextMenu = { element: el, x: event.clientX, y: event.clientY };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  function runContextAction(action: () => void | Promise<void>) {
    void action();
    closeContextMenu();
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

  let selectedElementIds = $derived.by(() => {
    void editor.selectionRevision;
    return editor.selectedIds;
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
    void project.effectiveElements.length;
    void project.effectiveAttachedRegions.length;
    void project.activeStateId;
    void project.editScope;
    void project.groups.length;
    void project.semanticGroups.length;
    void editor.selectionRevision;
    void editor.regionSelectionRevision;
    return groupedRows();
  });
</script>

<svelte:window onclick={closeContextMenu} onkeydown={(event) => {
  if (event.key === "Escape") closeContextMenu();
}} />

<aside class="layers">
  <h3>Layers ({project.effectiveElements.length})</h3>
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

  {#if rows.length === 0}
    <p class="muted">No elements</p>
  {:else}
    {#snippet elementRow(el: Element, nested = false)}
      {@const idx = project.elements.findIndex(element => element.id === el.id)}
      {@const isBackmost = idx === 0}
      {@const isFrontmost = idx === project.elements.length - 1}
      {@const stateBadge = stateBadgeForElement(el)}
      {@const isSelected = selectedElementIds.has(el.id)}
      {@const ElementIcon = iconForElement(el)}
      <div class="layer-row" class:nested>
        <button
          class="layer-item"
          class:selected={isSelected}
          class:hidden-el={!(el.visible ?? true)}
          onclick={(event) => selectElementFromList(el.id, event)}
          onkeydown={(event) => selectElementFromKeyboard(el.id, event)}
          oncontextmenu={(event) => openElementContextMenu(el, event)}
        >
          <span class="layer-icon"><ElementIcon size={14} strokeWidth={1.75} /></span>
          <span class="layer-text">
            <span class="layer-title">{displayId(el.id)}</span>
            <span class="layer-meta">
              {elementMeta(el)}
              {#if stateBadge}
                <span class="state-badge">{stateBadge}</span>
              {/if}
            </span>
          </span>
        </button>
        <div class="layer-actions">
          {#if project.hasElementOverride(el.id)}
            <button
              class="clear-btn"
              onclick={() => project.clearElementOverride(el.id)}
              title="Clear state overrides"
              aria-label={`Clear state overrides for ${el.id}`}
            ><X size={13} strokeWidth={1.75} /></button>
          {/if}
          <button
            class="reorder-btn"
            disabled={isBackmost}
            onclick={() => project.moveElementDown(el.id)}
            title="Move down (send backward)"
            aria-label={`Move ${el.id} backward`}
          ><ArrowDown size={13} strokeWidth={1.75} /></button>
          <button
            class="reorder-btn"
            disabled={isFrontmost}
            onclick={() => project.moveElementUp(el.id)}
            title="Move up (bring forward)"
            aria-label={`Move ${el.id} forward`}
          ><ArrowUp size={13} strokeWidth={1.75} /></button>
          <button
            class="visibility-btn"
            onclick={() => toggleVisibility(el)}
            title={el.visible === false ? "Show" : "Hide"}
            aria-label={el.visible === false ? `Show ${el.id}` : `Hide ${el.id}`}
          >
            {#if el.visible === false}
              <EyeOff size={13} strokeWidth={1.75} />
            {:else}
              <Eye size={13} strokeWidth={1.75} />
            {/if}
          </button>
        </div>
      </div>
    {/snippet}

    <div class="layer-list">
      {#each rows as row (rowKey(row))}
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
          {@const stateBadge = stateBadgeForRegion(row.region)}
          <div class="group-row">
            <button
              class="group-main"
              class:selected={selectedAttachedRegionId === row.region.id}
              class:hidden-el={row.region.visible === false}
              onclick={() => editor.selectAttachedRegion(row.region.id)}
              ondblclick={() => toggleGroup(`attached:${row.region.id}`)}
              title="Double-click to collapse or expand"
            >
              <span class="disclosure">{collapsedGroups.has(`attached:${row.region.id}`) ? "▸" : "▾"}</span>
              <span class="group-text">
                <span class="group-title">{displayId(row.region.id)}</span>
                <span class="group-meta">
                  {row.meta}
                  {#if stateBadge}
                    <span class="state-badge">{stateBadge}</span>
                  {/if}
                </span>
              </span>
            </button>
            {#if project.hasAttachedRegionOverride(row.region.id)}
              <button
                class="clear-btn group-clear"
                onclick={() => project.clearAttachedRegionOverride(row.region.id)}
                title="Clear state overrides"
              >×</button>
            {/if}
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

  {#if contextMenu}
    {@const idx = project.elements.findIndex(element => element.id === contextMenu!.element.id)}
    <div
      class="context-menu"
      style={`left: ${contextMenu.x}px; top: ${contextMenu.y}px;`}
      role="menu"
      tabindex="-1"
      oncontextmenu={(event) => event.preventDefault()}
    >
      <button
        role="menuitem"
        disabled={idx === project.elements.length - 1}
        onclick={() => runContextAction(() => project.bringElementToFront(contextMenu!.element.id))}
      >
        <BringToFront size={14} strokeWidth={1.75} /> Bring to front
      </button>
      <button
        role="menuitem"
        disabled={idx === 0}
        onclick={() => runContextAction(() => project.sendElementToBack(contextMenu!.element.id))}
      >
        <SendToBack size={14} strokeWidth={1.75} /> Send to back
      </button>
      <button
        role="menuitem"
        onclick={() => runContextAction(() => toggleVisibility(contextMenu!.element))}
      >
        {#if contextMenu.element.visible === false}
          <Eye size={14} strokeWidth={1.75} /> Show
        {:else}
          <EyeOff size={14} strokeWidth={1.75} /> Hide
        {/if}
      </button>
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

  .group-row {
    display: flex;
    align-items: stretch;
    gap: 1px;
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

  .state-badge {
    display: inline-block;
    margin-left: 5px;
    color: var(--accent);
    font-size: 9px;
    text-transform: uppercase;
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
    height: 18px;
    text-align: center;
    display: inline-flex;
    align-items: center;
    justify-content: center;
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

  .reorder-btn, .visibility-btn, .clear-btn {
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted-text);
    padding: 0;
    font-size: 10px;
    cursor: pointer;
    border-radius: 2px;
    font-family: monospace;
    min-width: 24px;
    min-height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .reorder-btn:hover:not(:disabled), .visibility-btn:hover, .clear-btn:hover {
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

  .clear-btn {
    color: var(--accent);
  }

  .group-clear {
    align-self: stretch;
  }

  .context-menu {
    position: fixed;
    z-index: 1200;
    min-width: 160px;
    padding: 4px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  }

  .context-menu button {
    width: 100%;
    min-height: 30px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    background: transparent;
    border: 0;
    border-radius: 3px;
    color: var(--text);
    cursor: pointer;
    font: inherit;
    font-size: 11px;
    text-align: left;
  }

  .context-menu button:hover:not(:disabled) {
    background: var(--surface-raised);
  }

  .context-menu button:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
