<script lang="ts">
  import { project, assetDataUrls } from "../stores/project.svelte";
  import { status, readableError } from "../stores/status.svelte";
  import * as api from "../api";
  import PixelEditor from "./PixelEditor.svelte";
  import UvEditorDialog from "./UvEditorDialog.svelte";
  import type { NineSlice, UvRect } from "../types";

  let editingAsset = $state<string | null>(null);
  let editingGuidesAsset = $state<string | null>(null);
  let showingTexturePackPicker = $state(false);
  let selectedTexturePackAssets = $state<string[]>([]);
  let texturePackPointerStarted = false;

  const minecraftPackAssets = api.MINECRAFT_TEXTURE_PACK_ASSETS;

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
      if (editingGuidesAsset === name) editingGuidesAsset = null;
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

  async function startEditingGuides(name: string) {
    try {
      await project.ensureAssetDataUrl(name);
      editingGuidesAsset = name;
    } catch (error) {
      status.error(`Failed to load ${displayName(name)}: ${readableError(error)}`);
    }
  }

  async function applyAssetGuides(name: string, value: UvRect | NineSlice | null) {
    if (!value) return;
    try {
      await project.updateAssetMetadata(name, {
        ...(project.assetMetadata[name] ?? {}),
        nine_slice: value as NineSlice,
      });
      editingGuidesAsset = null;
      status.success(`Updated guides for ${displayName(name)}.`);
    } catch (error) {
      status.error(`Failed to update guides for ${displayName(name)}: ${readableError(error)}`);
    }
  }

  async function clearAssetGuides(name: string) {
    try {
      await project.updateAssetMetadata(name, {
        ...(project.assetMetadata[name] ?? {}),
        nine_slice: null,
      });
      editingGuidesAsset = null;
      status.success(`Cleared guides for ${displayName(name)}.`);
    } catch (error) {
      status.error(`Failed to clear guides for ${displayName(name)}: ${readableError(error)}`);
    }
  }

  function openTexturePackPicker() {
    selectedTexturePackAssets = [];
    showingTexturePackPicker = true;
  }

  function selectableTexturePackAssets(): string[] {
    return minecraftPackAssets
      .map(asset => asset.name)
      .filter(name => !project.assets.includes(name));
  }

  function selectAllTexturePackAssets() {
    selectedTexturePackAssets = selectableTexturePackAssets();
  }

  function toggleTexturePackAsset(name: string) {
    selectedTexturePackAssets = selectedTexturePackAssets.includes(name)
      ? selectedTexturePackAssets.filter(asset => asset !== name)
      : [...selectedTexturePackAssets, name];
  }

  async function loadSelectedTexturePackAssets() {
    try {
      const loaded = await project.loadTexturePack("minecraft", selectedTexturePackAssets);
      showingTexturePackPicker = false;
      status.success(`Loaded ${loaded.length} texture${loaded.length === 1 ? "" : "s"} from Minecraft style.`);
    } catch (error) {
      status.error(`Failed to load texture pack: ${readableError(error)}`);
    }
  }

  function handleTexturePackBackdropPointerDown(event: PointerEvent) {
    texturePackPointerStarted = event.target === event.currentTarget;
  }

  function closeTexturePackPickerOnBackdrop(event: MouseEvent) {
    if (texturePackPointerStarted && event.target === event.currentTarget) {
      showingTexturePackPicker = false;
    }
    texturePackPointerStarted = false;
  }
</script>

<aside class="assets">
  <h3>Assets ({project.assets.length})</h3>

  <button class="import-btn" onclick={handleImport}>
    + Import PNG
  </button>

  <button class="import-btn" onclick={openTexturePackPicker} disabled={!project.isOpen}>
    + Load Texture Pack
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
          {@const dataUrl = project.peekAssetDataUrl(name)}
          <div class="asset-item">
            <button class="asset-thumb" onclick={() => startEditing(name)} title="Click to edit">
              {#if dataUrl}
                <img src={dataUrl} alt={name} />
              {:else}
                <span class="no-preview">Loading</span>
              {/if}
              <span class="asset-label">{displayName(name)}</span>
            </button>
            <div class="asset-actions">
              <button class="guide-btn" onclick={() => startEditingGuides(name)} title="Edit guides">Guides</button>
              <button class="remove-btn" onclick={() => handleRemove(name)} title="Remove">×</button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</aside>

{#if showingTexturePackPicker}
  <div class="modal-backdrop" role="presentation" onpointerdown={handleTexturePackBackdropPointerDown} onclick={closeTexturePackPickerOnBackdrop}>
    <div class="texture-pack-dialog" role="dialog" aria-modal="true" aria-labelledby="texture-pack-title">
      <header>
        <h2 id="texture-pack-title">Minecraft Style</h2>
        <button type="button" class="close-btn" aria-label="Close texture pack picker" onclick={() => showingTexturePackPicker = false}>×</button>
      </header>

      <div class="texture-pack-tools" role="group" aria-label="Texture pack selection">
        <button type="button" class="secondary-btn" onclick={selectAllTexturePackAssets}>Select all</button>
        <button type="button" class="secondary-btn" onclick={() => selectedTexturePackAssets = []}>Clear</button>
      </div>

      <div class="texture-pack-list">
        {#each minecraftPackAssets as asset (asset.name)}
          {@const present = project.assets.includes(asset.name)}
          <label class:present>
            <input
              type="checkbox"
              checked={selectedTexturePackAssets.includes(asset.name)}
              disabled={present}
              onchange={() => toggleTexturePackAsset(asset.name)}
            />
            <span>{displayName(asset.name)}</span>
            <small>{asset.metadata.width}x{asset.metadata.height}{present ? " · already in project" : ""}</small>
          </label>
        {/each}
      </div>

      <footer>
        <button type="button" class="secondary-btn" onclick={() => showingTexturePackPicker = false}>Cancel</button>
        <button type="button" class="import-btn" onclick={loadSelectedTexturePackAssets} disabled={selectedTexturePackAssets.length === 0}>
          Load {selectedTexturePackAssets.length}
        </button>
      </footer>
    </div>
  </div>
{/if}

{#if editingGuidesAsset}
  <UvEditorDialog
    title={`Edit Guides: ${displayName(editingGuidesAsset)}`}
    mode="nine_slice"
    assets={[editingGuidesAsset]}
    asset={editingGuidesAsset}
    nineSlice={project.assetMetadata[editingGuidesAsset]?.nine_slice ?? null}
    onapply={applyAssetGuides}
    onclear={() => {
      if (editingGuidesAsset) void clearAssetGuides(editingGuidesAsset);
    }}
    onclose={() => editingGuidesAsset = null}
  />
{/if}

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

  .import-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgba(0, 0, 0, 0.42);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
  }

  .texture-pack-dialog {
    width: min(420px, 100%);
    max-height: min(640px, 90vh);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.35);
  }

  .texture-pack-dialog header,
  .texture-pack-dialog footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px;
    border-bottom: 1px solid var(--border);
  }

  .texture-pack-dialog footer {
    justify-content: flex-end;
    border-top: 1px solid var(--border);
    border-bottom: 0;
  }

  .texture-pack-dialog footer .import-btn {
    width: auto;
    margin-bottom: 0;
  }

  .texture-pack-dialog h2 {
    margin: 0;
    font-size: 13px;
    flex: 1;
  }

  .texture-pack-tools {
    display: flex;
    gap: 6px;
    padding: 8px 10px 0;
  }

  .close-btn,
  .secondary-btn {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--muted-text);
    border-radius: 3px;
    cursor: pointer;
    font-family: inherit;
    font-size: 11px;
    padding: 4px 8px;
  }

  .close-btn {
    width: 24px;
    height: 24px;
    padding: 0;
  }

  .close-btn:hover,
  .secondary-btn:hover {
    color: var(--text);
    background: var(--surface-raised);
  }

  .texture-pack-list {
    overflow: auto;
    padding: 8px;
    display: grid;
    gap: 4px;
  }

  .texture-pack-list label {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 6px;
    font-size: 11px;
  }

  .texture-pack-list label.present {
    opacity: 0.62;
  }

  .texture-pack-list span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: monospace;
  }

  .texture-pack-list small {
    color: var(--muted-text);
    font-size: 10px;
    white-space: nowrap;
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

  .asset-actions {
    display: grid;
    grid-template-columns: 1fr 20px;
    gap: 2px;
  }

  .guide-btn {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--muted-text);
    font-size: 10px;
    cursor: pointer;
    padding: 1px 4px;
    line-height: 1.2;
    font-family: inherit;
  }

  .guide-btn:hover {
    background: var(--surface-raised);
    color: var(--text);
  }

  .remove-btn {
    background: transparent;
    border: 1px solid transparent;
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
