<script lang="ts">
  import { project } from "../stores/project.svelte";
  import { status, readableError } from "../stores/status.svelte";
  import * as api from "../api";
  import type { ModTarget } from "../types";

  let { onclose }: { onclose: () => void } = $props();

  let target = $state<ModTarget>("forge");
  let modId = $state("mymod");
  let packageName = $state("com.example.mymod");
  let className = $state(project.name.replace(/[^a-zA-Z0-9]/g, ""));
  let outputDir = $state("");
  let exporting = $state(false);
  let resultFiles = $state<string[]>([]);
  let errorMsg = $state("");

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
    if (!outputDir) return;
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

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      onclose();
    }
  }

  function handleOverlayKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onclose();
    }
  }
</script>

<div class="dialog-overlay" role="presentation" onclick={handleOverlayClick} onkeydown={handleOverlayKeydown}>
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

      <div class="form-row">
        <label for="exp-output">Output</label>
        <button id="exp-output" class="pick-btn" onclick={pickDirectory}>
          {outputDir || "Choose directory..."}
        </button>
      </div>
    </div>

    {#if resultFiles.length > 0}
      <div class="result">
        <h3>Generated Files</h3>
        <ul>
          {#each resultFiles as f}
            <li>
              <code>{f}</code>
              <button class="copy-btn" onclick={() => copyToClipboard(f)} title="Copy path">⎘</button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if errorMsg}
      <div class="error">{errorMsg}</div>
    {/if}

    <div class="actions">
      <button class="cancel-btn" onclick={onclose}>Close</button>
      <button class="export-btn" onclick={handleExport} disabled={!outputDir || exporting}>
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
  }
  .dialog {
    background: #1a1a2e;
    border: 1px solid #0f3460;
    border-radius: 8px; padding: 20px;
    min-width: 480px; max-width: 560px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.5);
  }
  h2 { font-size: 16px; color: #e0e0e0; margin-bottom: 16px; }
  h3 { font-size: 12px; color: #a0a0b0; margin: 12px 0 6px; }
  .form { display: flex; flex-direction: column; gap: 10px; }
  .form-row { display: flex; align-items: center; gap: 8px; }
  .form-row label { font-size: 11px; color: #606080; width: 85px; flex-shrink: 0; }
  input, select {
    flex: 1; background: #12121f; border: 1px solid #0f3460;
    color: #e0e0e0; padding: 5px 8px; font-size: 12px;
    font-family: monospace; border-radius: 4px;
  }
  input:focus, select:focus { outline: none; border-color: #e94560; }
  .pick-btn {
    flex: 1; background: #0f3460; border: 1px solid #1a5aa0;
    color: #a0b0d0; padding: 5px 8px; font-size: 12px;
    cursor: pointer; border-radius: 4px; font-family: inherit;
    text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .pick-btn:hover { background: #1a5aa0; color: #e0e0e0; }
  .result { margin-top: 12px; }
  .result ul { list-style: none; padding: 0; }
  .result li {
    display: flex; align-items: center; gap: 4px;
    padding: 3px 0; font-size: 11px; color: #808090;
  }
  .result code { font-family: monospace; font-size: 11px; color: #e9a23b; }
  .copy-btn {
    background: transparent; border: none; color: #505060;
    font-size: 12px; cursor: pointer;
  }
  .copy-btn:hover { color: #e0e0e0; }
  .error { color: #e94560; font-size: 12px; margin-top: 8px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  .cancel-btn, .export-btn {
    padding: 6px 16px; font-size: 12px; border-radius: 4px;
    cursor: pointer; font-family: inherit;
  }
  .cancel-btn {
    background: transparent; border: 1px solid #0f3460; color: #808090;
  }
  .export-btn {
    background: #e94560; border: 1px solid #e94560; color: #12121f; font-weight: 600;
  }
  .export-btn:disabled { opacity: 0.4; cursor: default; }
</style>
