<script lang="ts">
  import type { EditorTool } from "../stores/editor.svelte";
  import { editor } from "../stores/editor.svelte";
  import { project } from "../stores/project.svelte";

  const tools: { id: EditorTool; label: string; shortcut: string }[] = [
    { id: "select", label: "Select", shortcut: "V" },
    { id: "pan", label: "Pan", shortcut: "H" },
    { id: "slot", label: "Slot", shortcut: "S" },
    { id: "texture", label: "Texture", shortcut: "T" },
    { id: "text", label: "Text", shortcut: "X" },
    { id: "button", label: "Button", shortcut: "B" },
    { id: "toggle_button", label: "Toggle", shortcut: "G" },
  ];

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
</script>

<svelte:window onkeydown={onKeydown} />

<aside class="palette">
  <h3>Elements</h3>
  <div class="tool-list">
    {#each tools as tool}
      <button
        class="tool-btn"
        class:active={activeTool === tool.id}
        onclick={() => selectTool(tool.id)}
        title={`${tool.label} (${tool.shortcut})`}
      >
        <span class="tool-icon">
          {#if tool.id === "select"}↖
          {:else if tool.id === "pan"}✥
          {:else if tool.id === "slot"}◻
          {:else if tool.id === "texture"}▣
          {:else if tool.id === "text"}T
          {:else if tool.id === "button"}▭
          {:else if tool.id === "toggle_button"}◉
          {/if}
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
    {:else if activeTool === "text"}
      Click canvas to place text.
    {:else if activeTool === "button"}
      Click canvas to place button.
    {:else if activeTool === "toggle_button"}
      Click canvas to place toggle.
    {/if}
  </p>

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
    font-size: 14px;
    width: 20px;
    text-align: center;
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
