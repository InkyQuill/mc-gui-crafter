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
    flex: 1 1 auto;
    min-width: 0;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
  }

  .tab {
    display: flex;
    align-items: center;
    min-width: 112px;
    max-width: 220px;
    flex: 1 0 148px;
    border-right: 1px solid var(--border);
    background: var(--surface);
  }

  .tab.active {
    background: var(--surface);
    box-shadow: inset 0 -2px 0 var(--accent);
  }

  .tab-main,
  .tab-close {
    height: 28px;
    border: 0;
    background: transparent;
    color: var(--muted-text);
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
    color: var(--text);
    background: var(--surface-raised);
  }

  .tab-main:focus-visible,
  .tab-close:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .dirty {
    width: 8px;
    flex: 0 0 8px;
    color: var(--warning);
    font-size: 9px;
  }

  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 940px) {
    .tab {
      min-width: 96px;
      flex-basis: 120px;
    }
  }
</style>
