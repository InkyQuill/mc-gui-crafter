<script lang="ts">
  import { Copy } from "@lucide/svelte";
  import { project } from "../stores/project.svelte";
  import { status, readableError } from "../stores/status.svelte";
  import * as api from "../api";
  import type { CodegenMode, ExportScope, ModTarget } from "../types";

  let { onclose }: { onclose: () => void } = $props();

  let target = $state<ModTarget>("forge");
  let modId = $state("mymod");
  let packageName = $state("com.example.mymod");
  let className = $state(project.name.replace(/[^a-zA-Z0-9]/g, ""));
  let exportScope = $state<ExportScope>("full_mod");
  let codegenMode = $state<CodegenMode>(project.exportSettings.codegen_mode);
  let generateRuntimeHelpers = $state(project.exportSettings.generate_runtime_helpers);
  let generateSemanticRegistry = $derived(codegenMode === "modular");
  let overwriteExisting = $state(false);
  let outputDir = $state("");
  let exporting = $state(false);
  let resultFiles = $state<string[]>([]);
  let errorMsg = $state("");
  let preview = $state<api.ExportPreview | null>(null);
  let previewLoading = $state(false);
  let previewError = $state("");
  let previewRequestId = 0;
  let overlayPointerStarted = false;
  let overwriteWarnings = $derived(preview?.warnings.filter(warning => warning.startsWith("Target file already exists")) ?? []);
  let nonOverwriteWarnings = $derived(preview?.warnings.filter(warning => !warning.startsWith("Target file already exists")) ?? []);
  let overwritePaths = $derived(new Set(overwriteWarnings.map(warning => warning.replace("Target file already exists and will be overwritten: ", ""))));
  let canExport = $derived(
    Boolean(outputDir)
      && !exporting
      && !previewLoading
      && preview !== null
      && preview.errors.length === 0
      && !previewError,
  );

  $effect(() => {
    const request = {
      target,
      modId,
      packageName,
      className,
      outputDir,
      projectId: project.activeProjectId ?? undefined,
      revision: project.revision,
      codegenMode,
      generateRuntimeHelpers,
      generateSemanticRegistry,
      overwriteExisting,
      exportScope,
    };

    resultFiles = [];
    if (!request.outputDir) {
      preview = null;
      previewError = "";
      previewLoading = false;
      return;
    }

    const requestId = ++previewRequestId;
    previewLoading = true;
    previewError = "";

    const timer = window.setTimeout(async () => {
      try {
        const nextPreview = await api.projectExportPreview(
          request.target,
          request.modId,
          request.packageName,
          request.className,
          request.outputDir,
          request.projectId,
          {
            codegen_mode: request.codegenMode,
            generate_runtime_helpers: request.generateRuntimeHelpers,
            generate_semantic_registry: request.generateSemanticRegistry,
            export_scope: request.exportScope,
            overwrite: request.overwriteExisting,
          },
        );
        if (requestId === previewRequestId) {
          preview = nextPreview;
        }
      } catch (error) {
        if (requestId === previewRequestId) {
          preview = null;
          previewError = readableError(error);
        }
      } finally {
        if (requestId === previewRequestId) {
          previewLoading = false;
        }
      }
    }, 300);

    return () => {
      window.clearTimeout(timer);
    };
  });

  async function pickDirectory() {
    try {
      const dialog = await import("@tauri-apps/plugin-dialog");
      const result = await dialog.open({
        directory: true,
        multiple: false,
        title: "Select export directory",
      });
      if (result) {
        outputDir = result as string;
      }
    } catch {
      outputDir = prompt("Enter export directory:") || "";
    }
  }

  async function handleExport() {
    if (!canExport) return;
    exporting = true;
    errorMsg = "";
    try {
      const files = await api.projectExport(
        target,
        modId,
        packageName,
        className,
        outputDir,
        project.activeProjectId ?? undefined,
        {
          codegen_mode: codegenMode,
          generate_runtime_helpers: generateRuntimeHelpers,
          generate_semantic_registry: generateSemanticRegistry,
          export_scope: exportScope,
          overwrite: overwriteExisting,
        },
      );
      resultFiles = files;
      status.success(`Exported ${files.length} files.`);
    } catch (e) {
      errorMsg = readableError(e);
      status.error(`Export failed: ${errorMsg}`);
    }
    exporting = false;
  }

  async function copyToClipboard(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      status.success("Path copied.");
    } catch (error) {
      status.error(`Failed to copy path: ${readableError(error)}`);
    }
  }

  function handleOverlayPointerDown(event: PointerEvent) {
    overlayPointerStarted = event.target === event.currentTarget;
  }

  function handleOverlayClick(event: MouseEvent) {
    if (overlayPointerStarted && event.target === event.currentTarget) {
      onclose();
    }
    overlayPointerStarted = false;
  }

  function handleOverlayKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onclose();
    }
  }
</script>

<div class="dialog-overlay" role="presentation" onpointerdown={handleOverlayPointerDown} onclick={handleOverlayClick} onkeydown={handleOverlayKeydown}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="export-project-title">
    <h2 id="export-project-title">Export Project</h2>

    <div class="form">
      <div class="form-row">
        <label for="exp-target">Mod Loader</label>
        <select id="exp-target" bind:value={target}>
          <option value="forge">Forge</option>
          <option value="fabric">Fabric</option>
          <option value="neoforge">NeoForge</option>
        </select>
      </div>

      <div class="form-row">
        <label for="exp-scope">Scope</label>
        <select id="exp-scope" bind:value={exportScope}>
          <option value="full_mod">Full mod scaffold</option>
          <option value="textures_only">Texture assets only</option>
        </select>
      </div>

      <div class="form-row">
        <label for="exp-modid">Mod ID</label>
        <input id="exp-modid" type="text" bind:value={modId} />
      </div>

      <div class="form-row">
        <label for="exp-pkg">Java Package</label>
        <input id="exp-pkg" type="text" bind:value={packageName} />
      </div>

      <div class="form-row">
        <label for="exp-class">Class Name</label>
        <input id="exp-class" type="text" bind:value={className} />
      </div>

      {#if exportScope === "full_mod"}
        <div class="form-row">
          <label for="exp-codegen">Code Generation</label>
          <select id="exp-codegen" bind:value={codegenMode}>
            <option value="simple">Simple</option>
            <option value="modular">Modular</option>
          </select>
        </div>

        <label class="check-row">
          <input type="checkbox" bind:checked={generateRuntimeHelpers} />
          <span>Generate runtime helpers</span>
        </label>
      {/if}

      <label class="check-row">
        <input type="checkbox" bind:checked={overwriteExisting} />
        <span>Overwrite existing files</span>
      </label>

      {#if exportScope === "full_mod" && codegenMode === "modular" && project.semanticGroups.length === 0}
        <div class="warning">Modular export has no semantic groups.</div>
      {/if}

      <div class="form-row">
        <label for="exp-output">Output</label>
        <button id="exp-output" class="pick-btn" onclick={pickDirectory}>
          {outputDir || "Choose directory..."}
        </button>
      </div>
    </div>

    {#if outputDir}
      <div class="preview">
        <div class="preview-header">
          <h3>Export Preview</h3>
          {#if previewLoading}
            <span>Updating...</span>
          {/if}
        </div>

        {#if previewError}
          <div class="error">{previewError}</div>
        {:else if preview}
          {#if overwriteWarnings.length > 0}
            <div class="warning overwrite-summary">
              {overwriteWarnings.length} existing {overwriteWarnings.length === 1 ? "file" : "files"} will be overwritten.
              Review highlighted rows in Planned files.
            </div>
          {/if}

          {#if nonOverwriteWarnings.length > 0}
            <div class="warning-list">
              <h3>Warnings</h3>
              <ul>
                {#each nonOverwriteWarnings as warning (warning)}
                  <li>{warning}</li>
                {/each}
              </ul>
            </div>
          {/if}

          {#if preview.errors.length > 0}
            <div class="error-list">
              <h3>Errors</h3>
              <ul>
                {#each preview.errors as previewErrorMessage (previewErrorMessage)}
                  <li>{previewErrorMessage}</li>
                {/each}
              </ul>
            </div>
          {/if}

          <details class="planned-files">
            <summary>
              <span>Planned files</span>
              <span>{preview.files.length} {preview.files.length === 1 ? "file" : "files"}</span>
            </summary>
            <ul>
              {#each preview.files as f (f)}
                <li class:overwrite={overwritePaths.has(f)}>
                  <span class="file-status">{overwritePaths.has(f) ? "Overwrite" : "Write"}</span>
                  <code>{f}</code>
                  <button class="copy-btn" onclick={() => copyToClipboard(f)} title="Copy path" aria-label={`Copy ${f}`}>
                    <Copy size={13} strokeWidth={1.75} />
                  </button>
                </li>
              {/each}
            </ul>
          </details>
        {/if}
      </div>
    {/if}

    {#if resultFiles.length > 0}
      <details class="file-list result">
        <summary>
          <span>Generated files</span>
          <span>{resultFiles.length} {resultFiles.length === 1 ? "file" : "files"}</span>
        </summary>
        <ul>
          {#each resultFiles as f (f)}
            <li>
              <span class="file-status written">Written</span>
              <code>{f}</code>
              <button class="copy-btn" onclick={() => copyToClipboard(f)} title="Copy path" aria-label={`Copy ${f}`}>
                <Copy size={13} strokeWidth={1.75} />
              </button>
            </li>
          {/each}
        </ul>
      </details>
    {/if}

    {#if errorMsg}
      <div class="error">{errorMsg}</div>
    {/if}

    <div class="actions">
      <button class="cancel-btn" onclick={onclose}>Close</button>
      <button class="export-btn" onclick={handleExport} disabled={!canExport}>
        {exporting ? "Exporting..." : "Export"}
      </button>
    </div>
  </div>
</div>

<style>
  .dialog-overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.6);
    display: flex; align-items: center; justify-content: center;
    z-index: 1000;
    padding: 16px;
  }
  .dialog {
    width: min(560px, calc(100vw - 32px));
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px; padding: 16px 16px 0;
    max-height: calc(100vh - 32px);
    overflow: auto;
    box-shadow: 0 8px 32px rgba(0,0,0,0.5);
  }
  h2 { font-size: 15px; color: var(--text); margin: 0 0 12px; }
  h3 { font-size: 12px; color: var(--muted-text); margin: 12px 0 6px; }
  .form { display: flex; flex-direction: column; gap: 10px; }
  .form-row { display: flex; align-items: center; gap: 8px; }
  .form-row label { font-size: 11px; color: var(--muted-text); width: 85px; flex-shrink: 0; }
  .check-row {
    display: flex; align-items: center; gap: 8px;
    color: var(--muted-text); font-size: 12px;
    padding-left: 93px;
  }
  .check-row input { flex: 0 0 auto; }
  input, select {
    flex: 1; background: var(--app-bg); border: 1px solid var(--border);
    color: var(--text); padding: 5px 8px; font-size: 12px;
    font-family: monospace; border-radius: 4px;
  }
  input:focus, select:focus { outline: 2px solid var(--accent); outline-offset: 2px; }
  .pick-btn {
    flex: 1; background: var(--surface-raised); border: 1px solid var(--accent-2);
    color: var(--muted-text); padding: 5px 8px; font-size: 12px;
    cursor: pointer; border-radius: 4px; font-family: inherit;
    text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .pick-btn:hover { background: var(--accent-2); color: var(--text); }
  .preview, .result { margin-top: 12px; }
  .preview-header {
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
  }
  .preview-header span { color: var(--muted-text); font-size: 11px; }
  .file-list,
  .planned-files {
    margin-top: 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: color-mix(in srgb, var(--surface-raised) 55%, transparent);
  }
  .file-list summary,
  .planned-files summary {
    cursor: pointer;
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 7px 9px;
    color: var(--muted-text);
    font-size: 11px;
    user-select: none;
  }
  .file-list summary:hover,
  .planned-files summary:hover { color: var(--text); }
  .file-list summary span:first-child,
  .planned-files summary span:first-child {
    color: var(--text);
    font-weight: 600;
  }
  .file-list[open] summary,
  .planned-files[open] summary {
    border-bottom: 1px solid var(--border);
  }
  .planned-files ul, .result ul, .warning-list ul, .error-list ul {
    list-style: none; padding: 0; margin: 0;
  }
  .file-list ul,
  .planned-files ul {
    max-height: 180px;
    overflow: auto;
    padding: 6px 8px;
  }
  .planned-files li, .result li {
    display: flex; align-items: center; gap: 4px;
    padding: 3px 0; font-size: 11px; color: var(--muted-text);
  }
  .planned-files li.overwrite {
    color: var(--warning);
  }
  .file-status {
    flex: 0 0 auto;
    min-width: 58px;
    padding: 1px 5px;
    border-radius: 999px;
    border: 1px solid var(--border);
    color: var(--muted-text);
    font-size: 9px;
    text-align: center;
    text-transform: uppercase;
  }
  .planned-files li.overwrite .file-status {
    border-color: color-mix(in srgb, var(--warning) 55%, transparent);
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 10%, transparent);
  }
  .file-status.written {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .planned-files code, .result code {
    flex: 1;
    font-family: monospace; font-size: 11px; color: var(--warning);
    overflow-wrap: anywhere;
  }
  .planned-files li:not(.overwrite) code {
    color: var(--muted-text);
  }
  .warning-list li, .error-list li {
    padding: 3px 0; font-size: 11px; line-height: 1.4;
  }
  .warning {
    color: var(--warning); font-size: 11px;
    border: 1px solid color-mix(in srgb, var(--warning) 45%, transparent);
    border-radius: 4px; padding: 6px 8px;
    background: color-mix(in srgb, var(--warning) 10%, transparent);
  }
  .warning-list li { color: var(--warning); }
  .error-list li { color: var(--danger); }
  .copy-btn {
    background: transparent; border: none; color: var(--muted-text);
    font-size: 12px; cursor: pointer;
    width: 26px; height: 26px; padding: 0;
    border-radius: 3px;
    display: inline-flex; align-items: center; justify-content: center;
    flex: 0 0 26px;
  }
  .copy-btn:hover { color: var(--text); }
  .error { color: var(--danger); font-size: 12px; margin-top: 8px; }
  .actions {
    position: sticky; bottom: 0;
    display: flex; justify-content: flex-end; gap: 8px;
    margin: 16px -16px 0;
    padding: 12px 16px 16px;
    background: var(--surface);
    border-top: 1px solid var(--border);
  }
  .cancel-btn, .export-btn {
    padding: 6px 16px; font-size: 12px; border-radius: 4px;
    cursor: pointer; font-family: inherit;
  }
  .cancel-btn {
    background: transparent; border: 1px solid var(--border); color: var(--muted-text);
  }
  .export-btn {
    background: var(--accent); border: 1px solid var(--accent); color: var(--app-bg); font-weight: 600;
  }
  .export-btn:disabled { opacity: 0.4; cursor: default; }
  button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
