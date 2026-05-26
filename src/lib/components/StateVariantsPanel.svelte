<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { status, readableError } from "../stores/status.svelte";
  import * as api from "../api";
  import type { EditScope, ProjectState, StateOverrideTargetKind } from "../types";

  let confirmRemoveId = $state<string | null>(null);
  let confirmClearId = $state<string | null>(null);
  let clearing = $state(false);

  let selectedState = $derived.by(() => {
    return project.activeStateId ? project.states.find(state => state.id === project.activeStateId) ?? null : null;
  });

  let selectedOverrideCount = $derived.by(() => {
    if (!project.activeStateId) return 0;
    const overrides = project.stateOverrides[project.activeStateId];
    return Object.keys(overrides?.elements ?? {}).length
      + Object.keys(overrides?.attached_regions ?? {}).length
      + Object.keys(overrides?.groups ?? {}).length;
  });

  function normalizeStateId(label: string): string {
    const normalized = label
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "");
    return normalized || "state";
  }

  function nextStateId(label: string): string {
    const base = normalizeStateId(label);
    const existing = new Set(project.states.map(state => state.id));
    if (!existing.has(base)) return base;

    let index = 2;
    while (existing.has(`${base}_${index}`)) index += 1;
    return `${base}_${index}`;
  }

  function stateMeta(state: ProjectState): string {
    const bits = [state.id];
    if (state.initial) bits.push("initial");
    if (state.export_role) bits.push(state.export_role);
    return bits.join(" · ");
  }

  async function runAction(action: () => Promise<void>, successMessage?: string) {
    try {
      await action();
      if (successMessage) status.success(successMessage);
    } catch (error) {
      status.error(readableError(error));
    }
  }

  async function addState() {
    const label = "New State";
    const id = nextStateId(label);
    await runAction(async () => {
      await project.addState({
        id,
        label,
        description: null,
        initial: project.states.length === 0,
        export_role: null,
      });
      await project.setActiveState(id, project.editScope);
    }, "State added.");
  }

  async function selectState(id: string) {
    await runAction(async () => {
      await project.setActiveState(id, project.editScope);
      confirmRemoveId = null;
      confirmClearId = null;
    });
  }

  async function selectBase() {
    await runAction(async () => {
      await project.setActiveState(project.activeStateId, "base");
    });
  }

  async function updateSelected(changes: {
    label?: string;
    description?: string | null;
    initial?: boolean;
    export_role?: string | null;
  }) {
    if (!selectedState) return;
    await runAction(async () => {
      await project.updateState(selectedState.id, changes);
    });
  }

  async function setScope(scope: EditScope) {
    await runAction(async () => {
      await project.setEditScope(scope);
    });
  }

  async function removeSelected() {
    if (!selectedState || confirmRemoveId !== selectedState.id) {
      confirmRemoveId = selectedState?.id ?? null;
      return;
    }

    const id = selectedState.id;
    await runAction(async () => {
      await project.removeState(id);
      confirmRemoveId = null;
    }, "State removed.");
  }

  async function clearAllOverrides() {
    if (!project.activeStateId || selectedOverrideCount === 0 || clearing) return;
    if (confirmClearId !== project.activeStateId) {
      confirmClearId = project.activeStateId;
      return;
    }

    const stateId = project.activeStateId;
    const overrides = project.stateOverrides[stateId];
    const requests: Array<{ target_type: StateOverrideTargetKind; target_id: string }> = [];

    for (const targetId of Object.keys(overrides?.elements ?? {})) {
      requests.push({ target_type: "element", target_id: targetId });
    }
    for (const targetId of Object.keys(overrides?.attached_regions ?? {})) {
      requests.push({ target_type: "attached_region", target_id: targetId });
    }
    for (const targetId of Object.keys(overrides?.groups ?? {})) {
      requests.push({ target_type: "group", target_id: targetId });
    }

    clearing = true;
    try {
      for (const request of requests) {
        await api.stateOverrideClear({
          state_id: stateId,
          target_type: request.target_type,
          target_id: request.target_id,
          field: null,
        }, project.activeProjectId ?? undefined);
      }
      await project.refreshSessions();
      await project.hydrateActiveProject();
      confirmClearId = null;
      status.success("State overrides cleared.");
    } catch (error) {
      status.error(readableError(error));
      await project.refreshSessions();
      await project.hydrateActiveProject();
    } finally {
      clearing = false;
    }
  }
</script>

<aside class="states-panel">
  <div class="panel-header">
    <h3>States ({project.states.length})</h3>
    <button onclick={addState} disabled={!project.isOpen} title="Add state">Add</button>
  </div>

  <div class="scope-row">
    <button
      class:active={project.editScope === "base"}
      disabled={!project.isOpen}
      onclick={selectBase}
      title="Edit base layout"
    >
      Base
    </button>
    <button
      class:active={project.editScope === "state"}
      disabled={!project.isOpen || !project.activeStateId}
      onclick={() => setScope("state")}
      title="Edit active state overrides"
    >
      State Override
    </button>
  </div>

  <div class="state-list" aria-label="State variants">
    {#if project.states.length === 0}
      <p class="muted">No states</p>
    {:else}
      {#each project.states as state (state.id)}
        <button
          class="state-row"
          class:active={project.activeStateId === state.id}
          onclick={() => selectState(state.id)}
          title={state.description ?? state.id}
        >
          <span class="state-title">{state.label || state.id}</span>
          <span class="state-meta">{stateMeta(state)}</span>
        </button>
      {/each}
    {/if}
  </div>

  {#if selectedState}
    <div class="editor-section">
      <label>
        <span>Label</span>
        <input
          type="text"
          value={selectedState.label}
          onblur={(event) => updateSelected({ label: event.currentTarget.value.trim() || selectedState?.label || selectedState?.id })}
        />
      </label>

      <label>
        <span>Description</span>
        <textarea
          rows="3"
          value={selectedState.description ?? ""}
          onblur={(event) => updateSelected({ description: event.currentTarget.value.trim() || null })}
        ></textarea>
      </label>

      <label>
        <span>Export Role</span>
        <input
          type="text"
          value={selectedState.export_role ?? ""}
          onblur={(event) => updateSelected({ export_role: event.currentTarget.value.trim() || null })}
        />
      </label>

      <label class="check-row">
        <input
          type="checkbox"
          checked={selectedState.initial === true}
          onchange={(event) => updateSelected({ initial: event.currentTarget.checked })}
        />
        <span>Initial state</span>
      </label>
    </div>

    <div class="override-section">
      <div>
        <strong>{selectedOverrideCount}</strong>
        <span>override targets</span>
      </div>
      <button
        class:confirming={confirmClearId === selectedState.id}
        onclick={clearAllOverrides}
        disabled={selectedOverrideCount === 0 || clearing}
        title="Clear all overrides for selected state"
      >
        {confirmClearId === selectedState.id ? "Confirm Clear" : "Clear All"}
      </button>
    </div>

    <div class="danger-row">
      <button
        class:confirming={confirmRemoveId === selectedState.id}
        onclick={removeSelected}
        title="Remove selected state"
      >
        {confirmRemoveId === selectedState.id ? "Confirm Remove" : "Remove"}
      </button>
    </div>
  {/if}
</aside>

<style>
  .states-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    min-height: 100%;
    color: var(--text);
    font-size: 12px;
  }

  .panel-header,
  .override-section,
  .danger-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  h3 {
    margin: 0;
    font-size: 12px;
    text-transform: uppercase;
    color: var(--muted-text);
    letter-spacing: 0;
  }

  button {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--muted-text);
    padding: 5px 8px;
    font: inherit;
    font-size: 11px;
    border-radius: 3px;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    background: var(--surface-raised);
    color: var(--text);
  }

  button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  button.active {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--surface-raised);
  }

  .scope-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
  }

  .scope-row button {
    border: 0;
    border-radius: 0;
    min-width: 0;
  }

  .scope-row button + button {
    border-left: 1px solid var(--border);
  }

  .state-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 72px;
  }

  .state-row {
    display: grid;
    gap: 2px;
    width: 100%;
    min-height: 42px;
    text-align: left;
    padding: 7px 8px;
  }

  .state-title {
    min-width: 0;
    color: var(--text);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state-meta,
  .muted,
  .override-section span {
    color: var(--muted-text);
    font-size: 11px;
  }

  .muted {
    margin: 8px 0;
  }

  .editor-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  label {
    display: grid;
    gap: 4px;
    color: var(--muted-text);
  }

  input[type="text"],
  textarea {
    width: 100%;
    box-sizing: border-box;
    background: var(--surface-raised);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 3px;
    padding: 5px 6px;
    font: inherit;
    font-size: 12px;
  }

  textarea {
    resize: vertical;
    min-height: 56px;
  }

  input:focus,
  textarea:focus,
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .check-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .check-row input {
    margin: 0;
  }

  .override-section {
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .override-section strong {
    color: var(--text);
    margin-right: 4px;
  }

  .override-section button.confirming {
    border-color: var(--danger);
    color: var(--danger);
  }

  .danger-row {
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .danger-row button {
    margin-left: auto;
    border-color: var(--danger);
    color: var(--danger);
  }

  .danger-row button.confirming {
    background: var(--danger);
    color: white;
  }
</style>
