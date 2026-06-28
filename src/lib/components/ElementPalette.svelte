<script lang="ts">
  import {
    ArrowRight,
    Droplets,
    Grid3X3,
    Hand,
    Image,
    MousePointer,
    MousePointerClick,
    Square,
    ToggleLeft,
    Type,
    Zap,
  } from "@lucide/svelte";
  import type { EditorTool } from "../stores/editor.svelte";
  import { editor } from "../stores/editor.svelte";
  import { project } from "../stores/project.svelte";

  const tools = [
    { id: "select" as EditorTool, label: "Select", shortcut: "V", icon: MousePointer },
    { id: "pan" as EditorTool, label: "Pan", shortcut: "H", icon: Hand },
    { id: "slot" as EditorTool, label: "Slot", shortcut: "S", icon: Square },
    { id: "texture" as EditorTool, label: "Texture", shortcut: "T", icon: Image },
    { id: "progress" as EditorTool, label: "Progress", shortcut: "P", icon: ArrowRight },
    { id: "energy_bar" as EditorTool, label: "Meter", shortcut: "M", icon: Zap },
    { id: "fluid_tank" as EditorTool, label: "Tank", shortcut: "F", icon: Droplets },
    { id: "text" as EditorTool, label: "Text", shortcut: "X", icon: Type },
    { id: "button" as EditorTool, label: "Button", shortcut: "B", icon: MousePointerClick },
    { id: "toggle_button" as EditorTool, label: "Toggle", shortcut: "G", icon: ToggleLeft },
  ];

  let slotGridColumns = $state(9);
  let slotGridRows = $state(3);
  let slotGridX = $state(8);
  let slotGridY = $state(18);
  let slotGridCadence = $state(18);

  function selectTool(tool: EditorTool) {
    editor.tool = tool;
  }

  let activeTool = $derived.by(() => {
    void editor.toolRevision;
    return editor.tool;
  });

  function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;

    return target instanceof HTMLInputElement
      || target instanceof HTMLSelectElement
      || target instanceof HTMLTextAreaElement
      || target.isContentEditable
      || target.closest("[contenteditable='true']") !== null
      || target.closest('[role="dialog"]') !== null;
  }

  // Keyboard shortcuts
  function onKeydown(e: KeyboardEvent) {
    if (isEditableTarget(e.target)) return;
    switch (e.key.toLowerCase()) {
      case "v": editor.tool = "select"; break;
      case "h": editor.tool = "pan"; break;
      case "s": editor.tool = "slot"; break;
      case "t": editor.tool = "texture"; break;
      case "p": editor.tool = "progress"; break;
      case "m": editor.tool = "energy_bar"; break;
      case "f": editor.tool = "fluid_tank"; break;
      case "x": editor.tool = "text"; break;
      case "b": editor.tool = "button"; break;
      case "g": editor.tool = "toggle_button"; break;
      case "delete":
      case "backspace":
        if (editor.selectedElementId) {
          project.removeElement(editor.selectedElementId);
          editor.clearSelection();
        }
        break;
      case "escape":
        editor.clearSelection();
        editor.tool = "select";
        break;
    }
  }

  async function addSlotGrid() {
    const columns = Math.max(1, Math.min(12, Math.round(slotGridColumns)));
    const rows = Math.max(1, Math.min(12, Math.round(slotGridRows)));
    const cadence = Math.max(16, Math.min(32, Math.round(slotGridCadence)));
    const created: string[] = [];

    for (let row = 0; row < rows; row += 1) {
      for (let column = 0; column < columns; column += 1) {
        const element = await project.addElement("slot", slotGridX + column * cadence, slotGridY + row * cadence, {
          asset: "textures/minecraft/slot.png",
          slot_index: row * columns + column,
        });
        created.push(element.id);
      }
    }

    editor.setSelectedElements(created, created[0] ?? null);
    editor.tool = "select";
  }
</script>

<svelte:window onkeydown={onKeydown} />

<aside class="palette">
  <h3>Elements</h3>
  <div class="tool-list">
    {#each tools as tool (tool.id)}
      {@const ToolIcon = tool.icon}
      <button
        class="tool-btn"
        class:active={activeTool === tool.id}
        onclick={() => selectTool(tool.id)}
        title={`${tool.label} (${tool.shortcut})`}
      >
        <span class="tool-icon">
          <ToolIcon size={14} strokeWidth={1.75} />
        </span>
        <span class="tool-label">{tool.label}</span>
        <span class="tool-shortcut">{tool.shortcut}</span>
      </button>
    {/each}
  </div>

  <p class="hint">
    {#if activeTool === "select"}
      Click to select. Drag to move.
    {:else if activeTool === "pan"}
      Drag to pan canvas.
    {:else if activeTool === "slot"}
      Click canvas to place slot.
    {:else if activeTool === "texture"}
      Click canvas to place texture region.
    {:else if activeTool === "progress"}
      Click canvas to place a progress sprite.
    {:else if activeTool === "energy_bar"}
      Click canvas to place a meter.
    {:else if activeTool === "fluid_tank"}
      Click canvas to place a tank meter.
    {:else if activeTool === "text"}
      Click canvas to place text.
    {:else if activeTool === "button"}
      Click canvas to place button.
    {:else if activeTool === "toggle_button"}
      Click canvas to place toggle.
    {/if}
  </p>

  <details class="slot-grid-tool">
    <summary>
      <Grid3X3 size={14} strokeWidth={1.75} />
      <span>Add slot grid</span>
    </summary>
    <div class="slot-grid-fields">
      <label>
        <span>Cols</span>
        <input type="number" min="1" max="12" bind:value={slotGridColumns} />
      </label>
      <label>
        <span>Rows</span>
        <input type="number" min="1" max="12" bind:value={slotGridRows} />
      </label>
      <label>
        <span>X</span>
        <input type="number" bind:value={slotGridX} />
      </label>
      <label>
        <span>Y</span>
        <input type="number" bind:value={slotGridY} />
      </label>
      <label>
        <span>Step</span>
        <input type="number" min="16" max="32" bind:value={slotGridCadence} />
      </label>
    </div>
    <button class="slot-grid-add" onclick={addSlotGrid} disabled={!project.isOpen}>
      Add {Math.max(1, Math.round(slotGridColumns)) * Math.max(1, Math.round(slotGridRows))} slots
    </button>
  </details>

  <hr class="divider" />

  <h3>GUI</h3>
  <div class="gui-info">
    <span>{project.guiSize.width}×{project.guiSize.height}</span>
    <span class="muted">{project.elementCount} elements</span>
  </div>
</aside>


<style>
  .palette {
    padding: 10px;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted-text);
    margin-bottom: 8px;
  }

  .tool-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .tool-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted-text);
    padding: 6px 8px;
    font-size: 12px;
    cursor: pointer;
    border-radius: 3px;
    font-family: inherit;
    text-align: left;
    width: 100%;
  }

  .tool-btn:hover {
    background: var(--surface-raised);
    color: var(--text);
  }

  .tool-btn.active {
    background: var(--surface-raised);
    color: var(--accent);
    border-color: var(--accent);
  }

  .tool-icon {
    width: 20px;
    height: 16px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .tool-label {
    flex: 1;
  }

  .tool-shortcut {
    font-size: 10px;
    color: var(--muted-text);
    background: var(--surface);
    padding: 1px 5px;
    border-radius: 2px;
    font-family: monospace;
  }

  .hint {
    font-size: 11px;
    color: var(--muted-text);
    margin-top: 8px;
    line-height: 1.4;
  }

  .slot-grid-tool {
    margin-top: 10px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: color-mix(in srgb, var(--surface-raised) 55%, transparent);
  }

  .slot-grid-tool summary {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
    padding: 6px 8px;
    color: var(--muted-text);
    font-size: 12px;
    user-select: none;
  }

  .slot-grid-tool[open] summary {
    border-bottom: 1px solid var(--border);
    color: var(--text);
  }

  .slot-grid-fields {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 4px;
    padding: 7px;
  }

  .slot-grid-fields label {
    display: grid;
    gap: 3px;
    color: var(--muted-text);
    font-size: 10px;
  }

  .slot-grid-fields input {
    min-width: 0;
    width: 100%;
    background: var(--app-bg);
    border: 1px solid var(--border);
    border-radius: 2px;
    color: var(--text);
    font: inherit;
    font-size: 11px;
    padding: 3px 4px;
  }

  .slot-grid-add {
    width: calc(100% - 14px);
    margin: 0 7px 7px;
    background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--text);
    cursor: pointer;
    font: inherit;
    font-size: 11px;
    padding: 5px 7px;
  }

  .slot-grid-add:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .slot-grid-add:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .divider {
    border: none;
    border-top: 1px solid var(--border);
    margin: 12px 0;
  }

  .gui-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
    color: var(--muted-text);
    font-family: monospace;
  }

  .muted {
    color: var(--muted-text);
    font-size: 11px;
  }
</style>
