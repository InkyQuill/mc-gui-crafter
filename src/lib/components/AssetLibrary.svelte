<script lang="ts">
  import { project, assetDataUrls } from "../stores/project.svelte";
  import { status, readableError } from "../stores/status.svelte";
  import * as api from "../api";
  import PixelEditor from "./PixelEditor.svelte";

  let editingAsset = $state<string | null>(null);

  async function handleImport() {
    let path: string | null = null;
    try {
      const dialog = await import("@tauri-apps/plugin-dialog");
      const result = await dialog.open({
        filters: [{ name: "PNG Images", extensions: ["png"] }],
        multiple: false,
      });
      if (!result) return;
      path = result as string;
    } catch {
      openBrowserImport();
      return;
    }

    try {
      const asset = await api.assetImport(path, project.activeProjectId ?? undefined);
      if (!project.assets.includes(asset.name)) {
        project.assets = [...project.assets, asset.name];
      }
      if (asset.data_url) assetDataUrls.set(asset.name, asset.data_url);
      await project.refreshSessions();
      await project.syncFromBackend();
      status.success(`Imported ${displayName(asset.name)}.`);
    } catch (error) {
      status.error(`Failed to import asset: ${readableError(error)}`);
    }
  }

  async function handleImportFont() {
    let path: string | null = null;
    try {
      const dialog = await import("@tauri-apps/plugin-dialog");
      const result = await dialog.open({
        filters: [{ name: "Font Files", extensions: ["ttf", "otf"] }],
        multiple: false,
      });
      if (!result) return;
      path = result as string;
    } catch {
      status.warning("Font import requires the desktop app.");
      return;
    }

    try {
      await project.importFont(path!);
      status.success(`Imported font from ${path}`);
    } catch (error) {
      status.error(`Failed to import font: ${readableError(error)}`);
    }
  }

  async function handleRemove(name: string) {
    try {
      const removed = await api.assetRemove(name, project.activeProjectId ?? undefined);
      if (!removed) {
        status.warning(`${displayName(name)} was not found in this project.`);
        return;
      }
      project.assets = project.assets.filter(a => a !== name);
      const metadata = { ...project.assetMetadata };
      delete metadata[name];
      project.assetMetadata = metadata;
      assetDataUrls.delete(name);
      if (editingAsset === name) editingAsset = null;
      status.success(`Removed ${displayName(name)}.`);
    } catch (error) {
      status.error(`Failed to remove ${displayName(name)}: ${readableError(error)}`);
    }
  }

  function openBrowserImport() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      const reader = new FileReader();
      reader.onload = () => {
        const dataUrl = reader.result as string;
        const name = `textures/${file.name.replace(/\.[^.]+$/, "")}.png`;
        void importBrowserAsset(name, dataUrl);
      };
      reader.onerror = () => {
        status.error(`Failed to read ${file.name}.`);
      };
      reader.readAsDataURL(file);
    };
    input.click();
  }

  async function importBrowserAsset(name: string, dataUrl: string) {
    try {
      const asset = await api.assetImport(name, project.activeProjectId ?? undefined, dataUrl);
      if (!project.assets.includes(asset.name)) {
        project.assets = [...project.assets, asset.name];
      }
      assetDataUrls.set(asset.name, asset.data_url ?? dataUrl);
      await project.refreshSessions();
      await project.syncFromBackend();
      status.success(`Imported ${displayName(asset.name)}.`);
    } catch (error) {
      status.error(`Failed to import asset: ${readableError(error)}`);
    }
  }

  function displayName(fullPath: string): string {
    return fullPath.replace("textures/", "").replace(".png", "");
  }

  async function startEditing(name: string) {
    try {
      await project.ensureAssetDataUrl(name);
      editingAsset = name;
    } catch (error) {
      status.error(`Failed to load ${displayName(name)}: ${readableError(error)}`);
    }
  }
</script>

<aside class="assets">
  <h3>Assets ({project.assets.length})</h3>

  <button class="import-btn" onclick={handleImport}>
    + Import PNG
  </button>

  {#if project.fonts.length > 0}
    <h3>Fonts ({project.fonts.length})</h3>
    <ul class="font-list">
      {#each project.fonts as font (font.id)}
        <li class="font-item">
          <span>{font.id}</span>
          <span class="font-type">{font.source.type}</span>
        </li>
      {/each}
    </ul>
  {/if}

  <button class="import-btn" onclick={handleImportFont}>
    + Import Font
  </button>

  {#if editingAsset}
    {@const dataUrl = project.getAssetDataUrl(editingAsset)}
    {#if dataUrl}
      <PixelEditor
        assetName={editingAsset}
        dataUrl={dataUrl}
        onclose={() => editingAsset = null}
        onsaved={async (newDataUrl: string) => {
          try {
            const asset = await api.assetUpdate(
              editingAsset!,
              newDataUrl,
              project.activeProjectId ?? undefined,
            );
            assetDataUrls.set(asset.name, asset.data_url ?? newDataUrl);
            await project.refreshSessions();
            await project.syncFromBackend();
            status.success(`Updated ${displayName(asset.name)}.`);
          } catch (error) {
            status.error(`Failed to update ${displayName(editingAsset!)}: ${readableError(error)}`);
            throw error;
          }
        }}
      />
    {/if}
  {:else}
    {#if project.assets.length === 0}
      <p class="muted">No textures imported</p>
    {:else}
      <div class="asset-grid">
        {#each project.assets as name (name)}
          {@const dataUrl = project.getAssetDataUrl(name)}
          <div class="asset-item">
            <button class="asset-thumb" onclick={() => startEditing(name)} title="Click to edit">
              {#if dataUrl}
                <img src={dataUrl} alt={name} />
              {:else}
                <span class="no-preview">?</span>
              {/if}
              <span class="asset-label">{displayName(name)}</span>
            </button>
            <button class="remove-btn" onclick={() => handleRemove(name)} title="Remove">×</button>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</aside>

<style>
  .assets {
    padding: 10px;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted-text);
    margin-bottom: 8px;
  }

  .muted {
    color: var(--muted-text);
    font-size: 12px;
  }

  .import-btn {
    background: var(--surface-raised);
    border: 1px solid var(--accent-2);
    color: var(--muted-text);
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 3px;
    font-family: inherit;
    width: 100%;
    margin-bottom: 8px;
  }

  .import-btn:hover {
    background: var(--accent-2);
    color: var(--text);
  }

  .asset-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .asset-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .asset-thumb {
    background: var(--app-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    font-family: inherit;
  }

  .asset-thumb:hover {
    border-color: var(--accent);
  }

  .asset-thumb img {
    image-rendering: pixelated;
    max-width: 100%;
    max-height: 64px;
    object-fit: contain;
  }

  .no-preview {
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted-text);
    font-size: 18px;
  }

  .asset-label {
    font-size: 10px;
    color: var(--muted-text);
    font-family: monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .remove-btn {
    background: transparent;
    border: none;
    color: var(--muted-text);
    font-size: 12px;
    cursor: pointer;
    padding: 1px;
    line-height: 1;
    text-align: center;
  }

  .remove-btn:hover {
    color: var(--danger);
  }

  .font-list {
    list-style: none;
    padding: 0;
    margin: 0 0 8px 0;
  }

  .font-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 3px 6px;
    font-size: 11px;
    color: var(--text);
    background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    margin-bottom: 2px;
  }

  .font-type {
    font-size: 9px;
    color: var(--muted-text);
    text-transform: uppercase;
  }
</style>
