<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import type { Animation } from "../types";

  let collapsed = $state(true);
  let previewValue = $state(0.5);
  let isPlaying = $state(false);
  let playInterval: ReturnType<typeof setInterval> | null = null;
  let panelEl: HTMLDivElement | undefined = $state();

  // New animation form
  let newName = $state("");
  let newType = $state<Animation["type"]>("fill");
  let newDataKey = $state("");

  function addAnimation() {
    if (!newName || !newDataKey) return;
    project.addAnimation(newName, newType, newDataKey);
    newName = "";
    newDataKey = "";
  }

  function togglePlay() {
    if (isPlaying) {
      stopPlay();
    } else {
      startPlay();
    }
  }

  function toggleCollapsed() {
    collapsed = !collapsed;
  }

  function handleHeaderKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    toggleCollapsed();
  }

  function startPlay() {
    isPlaying = true;
    previewValue = 0;
    playInterval = setInterval(() => {
      previewValue = (previewValue + 0.02) % 1.0;
    }, 30);
  }

  function stopPlay() {
    isPlaying = false;
    if (playInterval) {
      clearInterval(playInterval);
      playInterval = null;
    }
  }

  // Keyboard: Space to toggle play
  function onKeydown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement) return;
    if (!panelEl?.contains(e.target as Node)) return;
    if (e.key === " " && !e.ctrlKey) {
      e.preventDefault();
      togglePlay();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="animation-panel" class:collapsed bind:this={panelEl}>
  <div
    class="panel-header"
    role="button"
    tabindex="0"
    aria-expanded={!collapsed}
    onclick={toggleCollapsed}
    onkeydown={handleHeaderKeydown}
  >
    <button
      type="button"
      class="panel-toggle"
      onclick={(e: MouseEvent) => { e.stopPropagation(); toggleCollapsed(); }}
      aria-expanded={!collapsed}
    >
      <span class="header-title">Animations</span>
      <span class="header-count">{project.animations.length}</span>
      <span class="collapse-icon">{collapsed ? "▸" : "▾"}</span>
    </button>

    <div class="preview-controls">
      <button class="ctrl-btn" onclick={(e: MouseEvent) => { e.stopPropagation(); previewValue = 0; }} title="Reset">⏮</button>
      <button class="ctrl-btn" onclick={(e: MouseEvent) => { e.stopPropagation(); togglePlay(); }} title="Play/Pause (Space)">
        {isPlaying ? "⏸" : "▶"}
      </button>
      <input
        type="range"
        class="scrubber"
        min="0" max="100"
        value={Math.round(previewValue * 100)}
        oninput={(e) => { previewValue = parseInt(e.currentTarget.value) / 100; }}
        onclick={(e: MouseEvent) => e.stopPropagation()}
      />
      <span class="value-label">{Math.round(previewValue * 100)}%</span>
    </div>
  </div>

  {#if !collapsed}
    <div class="panel-body">
      <!-- New animation form -->
      <div class="add-form">
        <input
          type="text"
          placeholder="Name"
          bind:value={newName}
          class="form-input"
        />
        <select bind:value={newType} class="form-select">
          <option value="fill">Fill</option>
          <option value="cycle">Cycle</option>
          <option value="pulse">Pulse</option>
          <option value="toggle">Toggle</option>
        </select>
        <input
          type="text"
          placeholder="Data key"
          bind:value={newDataKey}
          class="form-input"
        />
        <button class="add-btn" onclick={addAnimation} disabled={!newName || !newDataKey}>+</button>
      </div>

      <!-- Animation list -->
      <div class="anim-list">
        {#each project.animations as anim}
          <div class="anim-item">
            <div class="anim-header">
              <span class="anim-name">{anim.id}</span>
              <span class="anim-type">{anim.type}</span>
              <span class="anim-key">{anim.data_key}</span>
              <span class="anim-preview">
                <div class="mini-bar">
                  <div class="mini-fill" style="width: {previewValue * 100}%"></div>
                </div>
              </span>
              <button class="remove-btn" onclick={() => project.removeAnimation(anim.id)}>×</button>
            </div>

            <!-- Edit controls -->
            <div class="anim-edit">
              {#if anim.type === "fill"}
                <label>
                  Direction
                  <select
                    value={anim.direction ?? "left_to_right"}
                    onchange={(e) => project.updateAnimation(anim.id, { direction: e.currentTarget.value as Animation["direction"] })}
                  >
                    <option value="left_to_right">→</option>
                    <option value="right_to_left">←</option>
                    <option value="bottom_to_top">↑</option>
                    <option value="top_to_bottom">↓</option>
                  </select>
                </label>
                <label>Min <input type="number" value={anim.min_value ?? 0} min="0" max="1" step="0.1"
                  oninput={(e) => project.updateAnimation(anim.id, { min_value: parseFloat(e.currentTarget.value) })} /></label>
                <label>Max <input type="number" value={anim.max_value ?? 1} min="0" max="1" step="0.1"
                  oninput={(e) => project.updateAnimation(anim.id, { max_value: parseFloat(e.currentTarget.value) })} /></label>
              {:else if anim.type === "cycle"}
                <label>Frames <input type="number" value={anim.frame_count ?? 8} min="1" max="64"
                  oninput={(e) => project.updateAnimation(anim.id, { frame_count: parseInt(e.currentTarget.value) })} /></label>
                <label>FPS <input type="number" value={anim.fps ?? 12} min="1" max="60"
                  oninput={(e) => project.updateAnimation(anim.id, { fps: parseInt(e.currentTarget.value) })} /></label>
              {:else if anim.type === "pulse"}
                <label>Min Scale <input type="number" value={anim.min_value ?? 0.8} min="0" max="2" step="0.1"
                  oninput={(e) => project.updateAnimation(anim.id, { min_value: parseFloat(e.currentTarget.value) })} /></label>
                <label>Max Scale <input type="number" value={anim.max_value ?? 1.2} min="0" max="2" step="0.1"
                  oninput={(e) => project.updateAnimation(anim.id, { max_value: parseFloat(e.currentTarget.value) })} /></label>
              {/if}

              <!-- Bind to element -->
              {#if editor.selectedElementId}
                {@const selEl = project.elementById(editor.selectedElementId)}
                {#if selEl && (selEl.type === "progress" || selEl.type === "energy_bar" || selEl.type === "fluid_tank")}
                  <button
                    class="bind-btn"
                    class:bound={selEl.animation === anim.id}
                    onclick={() => project.bindAnimationToElement(
                      editor.selectedElementId!,
                      selEl.animation === anim.id ? undefined : anim.id
                    )}
                  >
                    {selEl.animation === anim.id ? "Unbind" : "Bind to selected"}
                  </button>
                {/if}
              {/if}
            </div>
          </div>
        {/each}

        {#if project.animations.length === 0}
          <p class="muted">No animations. Add one above.</p>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .animation-panel {
    background: var(--surface);
    border-top: 1px solid var(--border);
    flex-shrink: 0;
    user-select: none;
  }

  .animation-panel.collapsed .panel-body {
    display: none;
  }

  .animation-panel.collapsed {
    height: auto;
  }

  .panel-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    font-size: 11px;
    color: var(--muted-text);
    height: 28px;
    cursor: pointer;
  }

  .panel-header:hover,
  .panel-toggle:hover {
    background: var(--surface-raised);
  }

  .panel-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: 0;
    color: inherit;
    cursor: pointer;
    font: inherit;
    padding: 2px 4px;
    border-radius: 2px;
  }

  .header-title {
    font-weight: 600;
    color: var(--muted-text);
    text-transform: uppercase;
    letter-spacing: 1px;
    font-size: 10px;
  }

  .header-count {
    color: var(--accent);
    font-family: monospace;
  }

  .preview-controls {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
    justify-content: center;
  }

  .ctrl-btn {
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted-text);
    font-size: 12px;
    padding: 1px 4px;
    cursor: pointer;
    border-radius: 2px;
    font-family: monospace;
  }

  .ctrl-btn:hover {
    background: var(--surface-raised);
    color: var(--text);
  }

  .scrubber {
    width: 100px;
    height: 4px;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .value-label {
    font-size: 10px;
    color: var(--muted-text);
    font-family: monospace;
    min-width: 30px;
  }

  .collapse-icon {
    font-size: 10px;
    color: var(--muted-text);
  }

  .panel-body {
    padding: 8px 12px;
    border-top: 1px solid var(--border);
  }

  .add-form {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }

  .form-input, .form-select {
    background: var(--app-bg);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 3px 6px;
    font-size: 11px;
    font-family: monospace;
    border-radius: 2px;
  }

  .form-input { flex: 1; }
  .form-select { width: 70px; }

  .form-input:focus, .form-select:focus {
    outline: none;
    border-color: var(--accent);
  }

  .add-btn {
    background: var(--accent);
    border: none;
    color: var(--app-bg);
    width: 24px;
    font-size: 14px;
    font-weight: 700;
    cursor: pointer;
    border-radius: 2px;
  }

  .add-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .anim-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 180px;
    overflow-y: auto;
  }

  .anim-item {
    background: var(--app-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 6px;
  }

  .anim-header {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .anim-name {
    font-size: 12px;
    color: var(--text);
    font-family: monospace;
    min-width: 80px;
  }

  .anim-type {
    font-size: 10px;
    color: var(--accent);
    background: var(--surface);
    padding: 1px 6px;
    border-radius: 2px;
    text-transform: uppercase;
  }

  .anim-key {
    font-size: 10px;
    color: var(--warning);
    font-family: monospace;
    flex: 1;
  }

  .anim-preview {
    width: 60px;
  }

  .mini-bar {
    height: 6px;
    background: var(--surface);
    border-radius: 3px;
    overflow: hidden;
  }

  .mini-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.05s linear;
  }

  .remove-btn {
    background: transparent;
    border: none;
    color: var(--muted-text);
    font-size: 14px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
  }

  .remove-btn:hover {
    color: var(--danger);
  }

  .anim-edit {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .anim-edit label {
    font-size: 10px;
    color: var(--muted-text);
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .anim-edit select,
  .anim-edit input[type="number"] {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 2px 4px;
    font-size: 10px;
    font-family: monospace;
    border-radius: 2px;
    width: 50px;
  }

  .anim-edit select { width: auto; }
  .anim-edit select:focus, .anim-edit input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .bind-btn {
    background: transparent;
    border: 1px solid var(--accent);
    color: var(--accent);
    padding: 2px 8px;
    font-size: 10px;
    cursor: pointer;
    border-radius: 2px;
    font-family: inherit;
  }

  .bind-btn:hover, .bind-btn.bound {
    background: var(--accent);
    color: var(--app-bg);
  }

  .muted {
    color: var(--muted-text);
    font-size: 11px;
    text-align: center;
    padding: 8px;
  }
</style>
