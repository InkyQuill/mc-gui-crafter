<script lang="ts">
  import { guiPresets } from "../guiPresets";
  import { preferences, type EditorPreferences } from "../stores/preferences.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let dialogEl = $state<HTMLDivElement | undefined>();
  const dialogId = $props.id();
  const focusableSelector = [
    "button:not(:disabled)",
    "input:not(:disabled)",
    "select:not(:disabled)",
    "textarea:not(:disabled)",
    "a[href]",
    '[tabindex]:not([tabindex="-1"])',
  ].join(",");

  $effect(() => {
    const previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const firstFocusable = getFocusableElements()[0];

    (firstFocusable ?? dialogEl)?.focus();

    return () => {
      if (previouslyFocused && document.contains(previouslyFocused)) {
        previouslyFocused.focus();
      }
    };
  });

  function getFocusableElements(): HTMLElement[] {
    if (!dialogEl) return [];

    return Array.from(dialogEl.querySelectorAll<HTMLElement>(focusableSelector))
      .filter((element) => element.offsetParent !== null || element === document.activeElement);
  }

  function updateBoolean(key: "showGrid" | "snapToGrid", event: Event) {
    preferences.update({ [key]: (event.currentTarget as HTMLInputElement).checked });
  }

  function updateNumber(key: "majorGridSize" | "minorGridSize" | "snapSize", event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const value = Math.max(Number(input.min) || 1, Math.round(input.valueAsNumber || 1));
    preferences.update({ [key]: value });
  }

  function updateDefaultPreset(event: Event) {
    preferences.update({ defaultPreset: (event.currentTarget as HTMLSelectElement).value });
  }

  function updateTheme(event: Event) {
    preferences.update({ theme: (event.currentTarget as HTMLSelectElement).value as EditorPreferences["theme"] });
  }

  function handleReset() {
    preferences.reset();
  }

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      onclose();
    }
  }

  function trapFocus(event: KeyboardEvent) {
    const focusable = getFocusableElements();
    if (focusable.length === 0) {
      event.preventDefault();
      dialogEl?.focus();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];

    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    } else if (event.key === "Tab") {
      trapFocus(event);
    }
  }
</script>

<div class="dialog-overlay" role="presentation" onclick={handleOverlayClick}>
  <div
    bind:this={dialogEl}
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="{dialogId}-title"
    tabindex="-1"
    onkeydown={handleKeydown}
  >
    <div class="dialog-header">
      <h2 id="{dialogId}-title">Preferences</h2>
      <button class="close-btn" type="button" aria-label="Close preferences" title="Close" onclick={onclose}>×</button>
    </div>

    <section class="section" aria-labelledby="{dialogId}-grid">
      <h3 id="{dialogId}-grid">Grid</h3>

      <label class="check-row">
        <input
          type="checkbox"
          checked={preferences.values.showGrid}
          onchange={(event) => updateBoolean("showGrid", event)}
        />
        <span>Show grid</span>
      </label>

      <label class="check-row">
        <input
          type="checkbox"
          checked={preferences.values.snapToGrid}
          onchange={(event) => updateBoolean("snapToGrid", event)}
        />
        <span>Snap to grid</span>
      </label>

      <div class="field-grid">
        <label for="{dialogId}-major">Major</label>
        <input
          id="{dialogId}-major"
          type="number"
          min="1"
          max="256"
          value={preferences.values.majorGridSize}
          oninput={(event) => updateNumber("majorGridSize", event)}
        />

        <label for="{dialogId}-minor">Minor</label>
        <input
          id="{dialogId}-minor"
          type="number"
          min="1"
          max="256"
          value={preferences.values.minorGridSize}
          oninput={(event) => updateNumber("minorGridSize", event)}
        />

        <label for="{dialogId}-snap">Snap</label>
        <input
          id="{dialogId}-snap"
          type="number"
          min="1"
          max="256"
          value={preferences.values.snapSize}
          oninput={(event) => updateNumber("snapSize", event)}
        />
      </div>
    </section>

    <section class="section" aria-labelledby="{dialogId}-project">
      <h3 id="{dialogId}-project">New Project</h3>

      <div class="select-row">
        <label for="{dialogId}-preset">Default preset</label>
        <select id="{dialogId}-preset" value={preferences.values.defaultPreset} onchange={updateDefaultPreset}>
          {#each guiPresets as preset}
            <option value={preset.id}>{preset.label} ({preset.width}×{preset.height})</option>
          {/each}
        </select>
      </div>

      <div class="select-row">
        <label for="{dialogId}-theme">Theme</label>
        <select id="{dialogId}-theme" value={preferences.values.theme} onchange={updateTheme}>
          <option value="dark">Dark</option>
          <option value="high_contrast">High contrast</option>
        </select>
      </div>
    </section>

    <div class="dialog-actions">
      <button class="reset-btn" type="button" onclick={handleReset}>Reset Preferences</button>
      <button class="done-btn" type="button" onclick={onclose}>Done</button>
    </div>
  </div>
</div>

<style>
  .dialog-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.62);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 16px;
  }

  .dialog {
    width: min(460px, calc(100vw - 32px));
    background: #1a1a2e;
    border: 1px solid #0f3460;
    border-radius: 8px;
    padding: 16px;
    max-height: calc(100vh - 32px);
    overflow: auto;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    outline: none;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  h2 {
    color: #e0e0e0;
    font-size: 15px;
    margin: 0;
  }

  h3 {
    color: #8080a0;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0;
    margin: 0 0 8px;
    text-transform: uppercase;
  }

  .section {
    border-top: 1px solid #0f3460;
    padding: 12px 0;
  }

  .check-row,
  .select-row,
  .field-grid {
    font-size: 12px;
  }

  .check-row {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #d8d8e0;
    min-height: 26px;
  }

  .check-row input {
    accent-color: #e94560;
  }

  .field-grid {
    display: grid;
    grid-template-columns: 54px minmax(0, 1fr) 54px minmax(0, 1fr) 54px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }

  .select-row {
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    margin-top: 8px;
  }

  label {
    color: #8080a0;
  }

  input[type="number"],
  select {
    width: 100%;
    min-width: 0;
    background: #12121f;
    border: 1px solid #0f3460;
    border-radius: 4px;
    color: #e0e0e0;
    font: inherit;
    font-size: 12px;
    padding: 5px 7px;
  }

  input[type="number"] {
    font-family: monospace;
  }

  input:focus,
  select:focus,
  button:focus-visible {
    outline: 2px solid #e94560;
    outline-offset: 2px;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
  }

  button {
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
    font-size: 12px;
  }

  .close-btn {
    width: 28px;
    height: 28px;
    background: transparent;
    border: 1px solid transparent;
    color: #a0a0b0;
    font-size: 18px;
    line-height: 1;
  }

  .close-btn:hover {
    background: #0f3460;
    color: #e0e0e0;
  }

  .reset-btn,
  .done-btn {
    padding: 6px 12px;
  }

  .reset-btn {
    background: transparent;
    border: 1px solid #0f3460;
    color: #a0a0b0;
  }

  .reset-btn:hover {
    background: #0f3460;
    color: #e0e0e0;
  }

  .done-btn {
    background: #e94560;
    border: 1px solid #e94560;
    color: #12121f;
    font-weight: 700;
  }

  .done-btn:hover {
    background: #ff5a7a;
  }

  :global(:root[data-theme="high_contrast"] body) {
    background: #000;
    color: #fff;
  }

  :global(:root[data-theme="high_contrast"] .toolbar),
  :global(:root[data-theme="high_contrast"] .dialog) {
    border-color: #ffffff;
  }
</style>
