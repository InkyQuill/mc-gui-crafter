<script lang="ts">
  import { GuiRenderer } from "../engine/renderer";
  import { project } from "../stores/project.svelte";
  import { editor } from "../stores/editor.svelte";
  import { preferences } from "../stores/preferences.svelte";

  let containerEl: HTMLDivElement | undefined = $state();
  let renderer: GuiRenderer | null = $state(null);
  let initialized = $state(false);

  $effect(() => {
    if (!containerEl) return;
    initialized = true;

    const r = new GuiRenderer(containerEl);
    let disposed = false;
    r.init()
      .then(() => {
        if (disposed) return;
        renderer = r;
        editor.resetView(project.guiSize);
        r.render();
      })
      .catch(error => {
        if (disposed) return;
        console.error("Failed to initialize GUI renderer", error);
        initialized = false;
      });

    return () => {
      disposed = true;
      r.destroy();
      if (renderer === r) {
        renderer = null;
      }
      initialized = false;
    };
  });

  // Re-render when backend/project mirrors change.
  $effect(() => {
    void project.renderVersion;
    void project.revision;
    void project.effectiveElements.length;
    void project.effectiveAttachedRegions.length;
    void project.activeStateId;
    void project.editScope;
    void project.guiSize.width;
    void project.guiSize.height;
    void project.assets.length;
    void project.animations.length;
    void project.groups.length;
    void project.fontRenderDataVersion;
    for (const group of project.groups) {
      void group.id;
      void group.elements.length;
    }
    for (const element of project.effectiveElements) {
      void element.type;
      void element.x;
      void element.y;
      void element.width;
      void element.height;
      void element.size;
      void element.asset;
      void element.icon;
      void element.icon_uv?.x;
      void element.icon_uv?.y;
      void element.icon_uv?.width;
      void element.icon_uv?.height;
      void element.content;
      void element.font;
      void element.color;
      void element.shadow;
      void element.tooltip;
      void element.binding;
      void element.visible;
      void element.animation;
      void element.uv?.x;
      void element.uv?.y;
      void element.uv?.width;
      void element.uv?.height;
    }
    for (const animation of project.animations) {
      void animation.id;
      void animation.data_key;
      void animation.type;
    }
    if (renderer) {
      renderer.render();
    }
  });

  // Update transform when zoom/pan changes
  $effect(() => {
    void editor.zoom;
    void editor.panX;
    void editor.panY;
    if (renderer) {
      renderer.updateTransform();
    }
  });

  // Re-render when selection changes
  $effect(() => {
    void editor.selectionRevision;
    void editor.selectedElementId;
    if (renderer) {
      renderer.render();
    }
  });

  // Re-render when grid preferences change
  $effect(() => {
    void preferences.values.showGrid;
    void preferences.values.majorGridSize;
    void preferences.values.minorGridSize;
    void preferences.values.snapToGrid;
    void preferences.values.snapSize;
    if (renderer) {
      renderer.render();
    }
  });
</script>

<div class="canvas-wrapper" bind:this={containerEl}>
  {#if !initialized}
    <div class="canvas-placeholder">
      <p>GUI Canvas</p>
      <small>Create a new project to start</small>
    </div>
  {/if}
</div>

<style>
  .canvas-wrapper {
    width: 100%;
    height: 100%;
    overflow: hidden;
    position: relative;
    cursor: crosshair;
  }

  .canvas-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #404050;
  }

  .canvas-placeholder p {
    font-size: 18px;
    margin-bottom: 4px;
  }

  .canvas-placeholder small {
    font-size: 12px;
  }
</style>
