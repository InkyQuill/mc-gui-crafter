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
      assetDataUrls.set(asset.name, asset.data_url);
      await project.refreshSessions();
      await project.syncFromBackend();
      status.success(`Imported ${displayName(asset.name)}.`);
    } catch (error) {
      status.error(`Failed to import asset: ${readableError(error)}`);
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
      assetDataUrls.set(asset.name, asset.data_url);
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
</script>

<aside class="assets">
  <h3>Assets ({project.assets.length})</h3>

  <button class="import-btn" onclick={handleImport}>
    + Import PNG
  </button>

  {#if editingAsset}
    {@const dataUrl = assetDataUrls.get(editingAsset)}
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
            assetDataUrls.set(asset.name, asset.data_url);
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
        {#each project.assets as name}
          {@const dataUrl = assetDataUrls.get(name)}
          <div class="asset-item">
            <button class="asset-thumb" onclick={() => editingAsset = name} title="Click to edit">
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
    color: #606080;
    margin-bottom: 8px;
  }

  .muted {
    color: #505060;
    font-size: 12px;
  }

  .import-btn {
    background: #0f3460;
    border: 1px solid #1a5aa0;
    color: #a0b0d0;
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
    border-radius: 3px;
    font-family: inherit;
    width: 100%;
    margin-bottom: 8px;
  }

  .import-btn:hover {
    background: #1a5aa0;
    color: #e0e0e0;
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
    background: #12121f;
    border: 1px solid #0f3460;
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
    border-color: #e94560;
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
    color: #505060;
    font-size: 18px;
  }

  .asset-label {
    font-size: 10px;
    color: #808090;
    font-family: monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .remove-btn {
    background: transparent;
    border: none;
    color: #505060;
    font-size: 12px;
    cursor: pointer;
    padding: 1px;
    line-height: 1;
    text-align: center;
  }

  .remove-btn:hover {
    color: #e94560;
  }
</style>
