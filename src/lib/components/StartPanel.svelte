<script lang="ts">
  import * as api from "../api";
  import { editor } from "../stores/editor.svelte";
  import { project, ProjectStore } from "../stores/project.svelte";
  import type { McpServerStatus, TemplateInfo } from "../api";

  let { onnew, onopen }: { onnew: () => void; onopen: () => void | Promise<void> } = $props();

  let recentProjects = $state<string[]>(ProjectStore.getRecentProjects());
  let rowErrors = $state<Record<string, string>>({});
  let openingPath = $state<string | null>(null);
  let templates = $state<TemplateInfo[]>([]);
  let templateError = $state<string | null>(null);
  let creatingTemplate = $state<string | null>(null);
  let mcpStatus = $state<McpServerStatus | null>(null);
  let mcpUnavailable = $state(false);

  const templateShortcuts = $derived(templates.filter(template => template.name !== "empty").slice(0, 4));
  const mcpLabel = $derived(mcpUnavailable ? "Unavailable" : mcpStatus ? "Online" : "Checking");

  $effect(() => {
    recentProjects = ProjectStore.getRecentProjects();

    api.templateList()
      .then(result => {
        templates = result;
        templateError = null;
      })
      .catch(error => {
        templateError = readableError(error);
      });

    api.mcpStatus()
      .then(status => {
        mcpStatus = status;
        mcpUnavailable = !status;
      })
      .catch(() => {
        mcpUnavailable = true;
      });
  });

  function readableError(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function pathLabel(path: string): string {
    return path.split(/[\\/]/).pop() || path;
  }

  function removeRecent(path: string) {
    project.removeRecentProject(path);
    recentProjects = ProjectStore.getRecentProjects();
    const { [path]: _removed, ...nextErrors } = rowErrors;
    rowErrors = nextErrors;
  }

  async function openRecent(path: string) {
    if (openingPath !== null) return;

    openingPath = path;
    const { [path]: _removed, ...nextErrors } = rowErrors;
    rowErrors = nextErrors;

    try {
      await project.openProject(path);
      editor.resetView();
    } catch (error) {
      rowErrors = { ...rowErrors, [path]: readableError(error) };
    } finally {
      if (openingPath === path) {
        openingPath = null;
      }
    }
  }

  async function createFromTemplate(template: TemplateInfo) {
    creatingTemplate = template.name;
    templateError = null;
    try {
      await project.newProject(
        "Untitled GUI",
        template.default_width,
        template.default_height,
        "forge",
        template.name,
      );
      editor.resetView();
    } catch (error) {
      templateError = readableError(error);
    } finally {
      creatingTemplate = null;
    }
  }
</script>

<section class="start-panel" aria-labelledby="start-panel-title">
  <div class="launcher">
    <div class="header">
      <div>
        <h1 id="start-panel-title">Start</h1>
        <p>Open an existing GUI project or create a focused workspace.</p>
      </div>

      <div class="mcp-status" aria-label={`MCP status: ${mcpLabel}`}>
        <span class={mcpLabel === "Online" ? "online" : ""}></span>
        <div>
          <strong>MCP</strong>
          <small>{mcpStatus?.address ?? mcpLabel}</small>
        </div>
      </div>
    </div>

    <div class="actions" aria-label="Project actions">
      <button class="primary-action" type="button" onclick={onnew}>New Project</button>
      <button class="secondary-action" type="button" onclick={onopen}>Open Project</button>
    </div>

    {#if templateShortcuts.length > 0}
      <div class="templates" aria-labelledby="template-shortcuts-title">
        <h2 id="template-shortcuts-title">Templates</h2>
        <div class="template-list">
          {#each templateShortcuts as template (template.name)}
            <button
              type="button"
              class="template-button"
              disabled={creatingTemplate !== null}
              onclick={() => createFromTemplate(template)}
            >
              <span>{template.name.replace(/_/g, " ")}</span>
              <small>{template.default_width}x{template.default_height}</small>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    {#if templateError}
      <p class="inline-error" role="status">{templateError}</p>
    {/if}

    <div class="recent" aria-labelledby="recent-projects-title">
      <div class="section-heading">
        <h2 id="recent-projects-title">Recent Projects</h2>
        {#if recentProjects.length > 0}
          <span>{recentProjects.length}</span>
        {/if}
      </div>

      {#if recentProjects.length === 0}
        <p class="empty-recent">No recent projects yet.</p>
      {:else}
        <ul>
          {#each recentProjects as path (path)}
            <li class={rowErrors[path] ? "error" : ""}>
              <div class="recent-main">
                <button
                  type="button"
                  class="recent-open"
                  disabled={openingPath !== null}
                  onclick={() => openRecent(path)}
                >
                  <span>{pathLabel(path)}</span>
                  <small>{path}</small>
                </button>
                <button
                  type="button"
                  class="remove-button"
                  aria-label={`Remove ${pathLabel(path)} from recent projects`}
                  onclick={() => removeRecent(path)}
                >
                  Remove
                </button>
              </div>

              {#if openingPath === path}
                <p class="row-note" role="status">Opening...</p>
              {/if}

              {#if rowErrors[path]}
                <p class="row-error" role="alert">
                  Failed to open: {rowErrors[path]}
                </p>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</section>

<style>
  .start-panel {
    height: 100%;
    display: grid;
    place-items: center;
    background:
      linear-gradient(180deg, rgba(22, 33, 62, 0.44), rgba(18, 18, 31, 0.7)),
      #12121f;
    color: #e0e0e0;
    padding: 28px;
  }

  .launcher {
    width: min(760px, 100%);
    display: grid;
    gap: 18px;
  }

  .header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 18px;
  }

  h1,
  h2,
  p {
    margin: 0;
  }

  h1 {
    font-size: 20px;
    line-height: 1.2;
  }

  h2 {
    color: #a0a0b0;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .header p {
    margin-top: 6px;
    color: #808090;
    font-size: 12px;
  }

  .mcp-status {
    min-width: 190px;
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 8px;
    color: #a0a0b0;
    font-size: 11px;
  }

  .mcp-status > span {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #606080;
  }

  .mcp-status > span.online {
    background: #56d364;
  }

  .mcp-status strong,
  .mcp-status small {
    display: block;
  }

  .mcp-status strong {
    color: #e0e0e0;
    font-size: 11px;
  }

  .mcp-status small {
    overflow: hidden;
    color: #808090;
    font-family: monospace;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions,
  .template-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  button {
    border-radius: 4px;
    cursor: pointer;
    font: inherit;
  }

  button:focus-visible {
    outline: 2px solid #e94560;
    outline-offset: 2px;
  }

  button:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .primary-action,
  .secondary-action {
    min-width: 128px;
    border: 1px solid #0f3460;
    padding: 8px 14px;
    font-size: 12px;
    font-weight: 700;
  }

  .primary-action {
    background: #e94560;
    border-color: #e94560;
    color: #12121f;
  }

  .primary-action:hover {
    background: #ff5a7a;
  }

  .secondary-action,
  .template-button,
  .recent-open,
  .remove-button {
    background: #16213e;
    color: #e0e0e0;
  }

  .secondary-action:hover,
  .template-button:hover,
  .recent-open:hover,
  .remove-button:hover {
    border-color: #e94560;
  }

  .templates,
  .recent {
    display: grid;
    gap: 8px;
  }

  .template-button {
    min-width: 136px;
    border: 1px solid #0f3460;
    padding: 8px 10px;
    text-align: left;
    text-transform: capitalize;
  }

  .template-button span,
  .template-button small {
    display: block;
  }

  .template-button span {
    font-size: 12px;
    font-weight: 700;
  }

  .template-button small {
    margin-top: 2px;
    color: #808090;
    font-family: monospace;
    font-size: 11px;
  }

  .inline-error,
  .row-error {
    color: #ff8ba0;
    font-size: 11px;
  }

  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .section-heading span {
    color: #606080;
    font-family: monospace;
    font-size: 11px;
  }

  .empty-recent {
    border: 1px dashed #0f3460;
    padding: 12px;
    color: #606080;
    font-size: 12px;
  }

  ul {
    display: grid;
    gap: 6px;
    list-style: none;
  }

  li {
    border: 1px solid #0f3460;
    background: #12121f;
  }

  li.error {
    border-color: #8f3046;
  }

  .recent-main {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: stretch;
  }

  .recent-open {
    min-width: 0;
    border: 0;
    padding: 9px 10px;
    text-align: left;
  }

  .recent-open span,
  .recent-open small {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .recent-open span {
    color: #e0e0e0;
    font-size: 12px;
    font-weight: 700;
  }

  .recent-open small {
    margin-top: 2px;
    color: #808090;
    font-family: monospace;
    font-size: 11px;
  }

  .remove-button {
    border: 0;
    border-left: 1px solid #0f3460;
    padding: 0 10px;
    color: #808090;
    font-size: 11px;
  }

  .row-note,
  .row-error {
    border-top: 1px solid #0f3460;
    padding: 7px 10px;
  }

  .row-note {
    color: #808090;
    font-size: 11px;
  }

  @media (max-width: 720px) {
    .start-panel {
      place-items: start stretch;
      padding: 18px;
    }

    .header,
    .recent-main {
      grid-template-columns: 1fr;
      display: grid;
    }

    .mcp-status {
      min-width: 0;
    }

    .remove-button {
      border-top: 1px solid #0f3460;
      border-left: 0;
      padding: 8px 10px;
      text-align: left;
    }
  }
</style>
