<script lang="ts">
  import type { ProjectSessionSummary } from "../types";

  let {
    sessions,
    activeProjectId,
    onswitch,
    onclose,
  }: {
    sessions: ProjectSessionSummary[];
    activeProjectId: string | null;
    onswitch: (projectId: string) => void | Promise<void>;
    onclose: (projectId: string) => void | Promise<void>;
  } = $props();

  function labelFor(session: ProjectSessionSummary): string {
    return session.name || session.path?.split(/[\\/]/).pop() || "Untitled";
  }
</script>

{#if sessions.length > 0}
  <nav class="project-tabs" aria-label="Open projects">
    {#each sessions as session (session.id)}
      <div class:active={session.id === activeProjectId} class="tab">
        <button
          class="tab-main"
          onclick={() => onswitch(session.id)}
          title={session.path ?? session.name}
          aria-current={session.id === activeProjectId ? "page" : undefined}
        >
          <span class="dirty" aria-hidden="true">{session.is_dirty ? "●" : ""}</span>
          <span class="label">{labelFor(session)}</span>
        </button>
        <button class="tab-close" onclick={() => onclose(session.id)} title={`Close ${labelFor(session)}`} aria-label={`Close ${labelFor(session)}`}>
          ×
        </button>
      </div>
    {/each}
  </nav>
{/if}

<style>
  .project-tabs {
    display: flex;
    align-items: stretch;
    flex: 1 1 120px;
    min-width: 90px;
    overflow: hidden;
    border-left: 1px solid #0f3460;
  }

  .tab {
    display: flex;
    align-items: center;
    min-width: 72px;
    max-width: 180px;
    flex: 1 1 132px;
    border-right: 1px solid #0f3460;
    background: #121a32;
  }

  .tab.active {
    background: #1a1a2e;
    box-shadow: inset 0 -2px 0 #e94560;
  }

  .tab-main,
  .tab-close {
    height: 28px;
    border: 0;
    background: transparent;
    color: #a0a0b0;
    font-family: inherit;
    cursor: pointer;
    border-radius: 3px;
  }

  .tab-main {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    flex: 1;
    padding: 0 6px 0 8px;
    font-size: 11px;
  }

  .tab-close {
    width: 26px;
    height: 26px;
    flex: 0 0 26px;
    padding: 0;
    font-size: 14px;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .tab-main:hover,
  .tab-close:hover {
    color: #e0e0e0;
    background: #0f3460;
  }

  .tab-main:focus-visible,
  .tab-close:focus-visible {
    outline: 2px solid #e94560;
    outline-offset: -2px;
  }

  .dirty {
    width: 8px;
    flex: 0 0 8px;
    color: #e9a23b;
    font-size: 9px;
  }

  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
