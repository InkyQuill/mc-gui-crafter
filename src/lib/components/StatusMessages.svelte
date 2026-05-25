<script lang="ts">
  import { status } from "../stores/status.svelte";

  const alertTypes = new Set(["warning", "error"]);
  const role = $derived(status.current && alertTypes.has(status.current.type) ? "alert" : "status");
</script>

{#if status.current}
  <div class="status-region" aria-live="polite">
    <div class="message" class:success={status.current.type === "success"} class:warning={status.current.type === "warning"} class:error={status.current.type === "error"} class:info={status.current.type === "info"} role={role}>
      <span class="marker" aria-hidden="true"></span>
      <p>{status.current.text}</p>
      <button type="button" aria-label="Dismiss message" onclick={() => status.clear()}>×</button>
    </div>
  </div>
{/if}

<style>
  .status-region {
    position: fixed;
    top: 46px;
    right: 14px;
    z-index: 3000;
    pointer-events: none;
  }

  .message {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    max-width: min(420px, calc(100vw - 28px));
    min-height: 34px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--app-bg);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    color: var(--text);
    padding: 7px 8px;
    pointer-events: auto;
  }

  .marker {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #6aa6ff;
  }

  .message.success .marker {
    background: #56d364;
  }

  .message.warning .marker {
    background: var(--warning);
  }

  .message.error .marker {
    background: var(--danger);
  }

  p {
    overflow: hidden;
    margin: 0;
    color: inherit;
    font-size: 12px;
    line-height: 1.35;
    text-overflow: ellipsis;
  }

  button {
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 3px;
    background: transparent;
    color: var(--muted-text);
    cursor: pointer;
    font: inherit;
    line-height: 1;
  }

  button:hover,
  button:focus-visible {
    background: var(--surface-raised);
    color: var(--text);
    outline: none;
  }
</style>
