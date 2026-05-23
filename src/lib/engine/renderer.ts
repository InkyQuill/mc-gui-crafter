import { Application, Assets, Container, Graphics, Rectangle, Text, TextStyle, Sprite, Texture } from "pixi.js";
import type { Element, FontRenderData, GlyphInfo, MinecraftFontProviderRenderData, Layer } from "../types";
import { project, assetDataUrls } from "../stores/project.svelte";
import { editor } from "../stores/editor.svelte";
import { preferences } from "../stores/preferences.svelte";

const SELECTED_TINT = 0xffff00;
const LEGACY_BACKGROUND_TEXTURE = "textures/background.png";

const GENERATED_TEXTURES = new Set([
  LEGACY_BACKGROUND_TEXTURE,
  "textures/generated/gui_panel.png",
  "textures/generated/slot.png",
  "textures/generated/button.png",
  "textures/generated/progress_arrow.png",
  "textures/generated/fluid_tank.png",
  "textures/generated/energy_bar.png",
  "textures/generated/scrollbar.png",
]);

function isGeneratedTexturePath(path: string | undefined): boolean {
  return path !== undefined && GENERATED_TEXTURES.has(path);
}

function canvasPalette() {
  switch (preferences.values.theme) {
    case "light":
      return {
        background: 0x9f9f9f,
        guiFill: 0xd8d8d8,
        line: 0x4a4a4a,
        border: 0x4a4a4a,
        guiAlpha: 0.42,
        minorAlpha: editor.zoom >= 2 ? 0.12 : 0,
        majorAlpha: 0.22,
        borderAlpha: 0.55,
      };
    case "high_contrast":
      return {
        background: 0x000000,
        guiFill: 0x000000,
        line: 0xffffff,
        border: 0xffffff,
        guiAlpha: 0.9,
        minorAlpha: editor.zoom >= 2 ? 0.2 : 0,
        majorAlpha: 0.5,
        borderAlpha: 1,
      };
    default:
      return {
        background: 0x101214,
        guiFill: 0xb8b8b8,
        line: 0xffffff,
        border: 0xffffff,
        guiAlpha: 0.3,
        minorAlpha: editor.zoom >= 2 ? 0.1 : 0,
        majorAlpha: 0.25,
        borderAlpha: 0.5,
      };
  }
}

const LAYER_ORDER: Record<Layer, number> = {
  background: 0,
  overlay: 1,
  animatable: 2,
};

type CanvasPointerEvent = PointerEvent & {
  global?: { x: number; y: number };
};

export class GuiRenderer {
  app: Application;
  private gridContainer: Container;
  private elementsContainer: Container;
  private overlayContainer: Container;
  private selectionGraphics: Graphics;
  private cursorLabel: Text;

  private containerEl: HTMLElement;
  private resizeObserver: ResizeObserver | null = null;
  private cleanupFns: (() => void)[] = [];
  private dragStartPositions = new Map<string, { x: number; y: number }>();
  private glyphTextureCache = new Map<string, Texture>();
  private fontSourceTextureCache = new Map<string, Texture>();
  private textTextureCache = new Map<string, Texture>();
  private loadingFontSources = new Set<string>();
  private glyphTextureCacheVersion = -1;
  private spacePanning = false;
  private isPanning = false;
  private panStartX = 0;
  private panStartY = 0;
  private panOrigX = 0;
  private panOrigY = 0;
  private renderFrame: number | null = null;
  private initPromise: Promise<void> | null = null;
  private pixiInitialized = false;
  private ready = false;
  private disposed = false;
  private appDestroyed = false;

  constructor(containerEl: HTMLElement) {
    this.containerEl = containerEl;

    this.app = new Application();
    this.gridContainer = new Container();
    this.elementsContainer = new Container();
    this.overlayContainer = new Container();
    this.selectionGraphics = new Graphics();
    this.cursorLabel = new Text({
      text: "",
      style: new TextStyle({ fontSize: 10, fill: 0xe94560, fontFamily: "monospace" }),
    });
  }

  async init() {
    if (this.disposed) return;
    if (this.ready) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = this.initApplication();
    return this.initPromise;
  }

  private async initApplication() {
    const rect = this.containerEl.getBoundingClientRect();
    editor.canvasWidth = rect.width;
    editor.canvasHeight = rect.height;

    await this.app.init({
      width: rect.width,
      height: rect.height,
      backgroundColor: canvasPalette().background,
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
    });

    this.pixiInitialized = true;

    if (this.disposed) {
      this.destroyPixiApp();
      return;
    }

    this.containerEl.appendChild(this.app.canvas as HTMLCanvasElement);

    // Layer order
    this.app.stage.addChild(this.gridContainer);
    this.app.stage.addChild(this.elementsContainer);
    this.app.stage.addChild(this.overlayContainer);
    this.overlayContainer.addChild(this.selectionGraphics);
    this.overlayContainer.addChild(this.cursorLabel);

    // Event mode
    this.app.stage.eventMode = "static";
    this.app.stage.hitArea = this.app.screen;

    this.setupEvents();
    this.setupResizeObserver();
    this.ready = true;
    this.updateTransform();
    this.render();
  }

  private setupResizeObserver() {
    this.resizeObserver = new ResizeObserver(() => {
      if (!this.ready || this.disposed) return;
      const rect = this.containerEl.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        this.app.renderer.resize(rect.width, rect.height);
        editor.canvasWidth = rect.width;
        editor.canvasHeight = rect.height;
        this.render();
      }
    });
    this.resizeObserver.observe(this.containerEl);
  }

  private setupEvents() {
    const stage = this.app.stage;
    const isPanMode = () => editor.tool === "pan" || this.spacePanning;

    const onPointerMove = (e: CanvasPointerEvent) => {
      const pointer = this.pointerCanvasPosition(e);
      const gui = this.screenToGui(pointer.x, pointer.y);
      editor.mouseGuiX = gui.x;
      editor.mouseGuiY = gui.y;
      this.updateCursorLabel(gui.x, gui.y);

      // Update cursor for resize handles
      let cursor = "crosshair";
      if (this.isPanning) {
        cursor = "grabbing";
      } else if (isPanMode()) {
        cursor = "grab";
      } else if (editor.isResizing) {
        cursor = "nwse-resize";
      } else if (editor.tool === "select" && editor.selectedElementId) {
        const selEl = project.elementById(editor.selectedElementId);
        if (selEl && this.hitTestHandle(selEl, gui.x, gui.y)) {
          cursor = "nwse-resize";
        }
      }
      if (this.app.canvas instanceof HTMLCanvasElement) {
        this.app.canvas.style.cursor = cursor;
      }

      if (this.isPanning) {
        editor.panX = this.panOrigX + pointer.x - this.panStartX;
        editor.panY = this.panOrigY + pointer.y - this.panStartY;
        this.updateTransform();
        return;
      }

      if (editor.isResizing && editor.resizeElementId && editor.resizeCorner) {
        const dx = (pointer.x - editor.resizeStartScreenX) / editor.zoom;
        const dy = (pointer.y - editor.resizeStartScreenY) / editor.zoom;
        const ox = editor.resizeOrigX;
        const oy = editor.resizeOrigY;
        const ow = editor.resizeOrigW;
        const oh = editor.resizeOrigH;

        let nx: number, ny: number, nw: number, nh: number;
        switch (editor.resizeCorner) {
          case "tl":
            nx = editor.snapCoordinate(ox + dx);
            ny = editor.snapCoordinate(oy + dy);
            nw = Math.max(4, ow - (nx - ox));
            nh = Math.max(4, oh - (ny - oy));
            break;
          case "tr":
            {
              const right = editor.snapCoordinate(ox + ow + dx);
              nx = ox;
              ny = editor.snapCoordinate(oy + dy);
              nw = Math.max(4, right - ox);
              nh = Math.max(4, oh - (ny - oy));
            }
            break;
          case "bl":
            {
              const bottom = editor.snapCoordinate(oy + oh + dy);
              nx = editor.snapCoordinate(ox + dx);
              ny = oy;
              nw = Math.max(4, ow - (nx - ox));
              nh = Math.max(4, bottom - oy);
            }
            break;
          case "br":
            nx = ox;
            ny = oy;
            nw = Math.max(4, editor.snapCoordinate(ox + ow + dx) - ox);
            nh = Math.max(4, editor.snapCoordinate(oy + oh + dy) - oy);
            break;
        }

        project.resizeElement(editor.resizeElementId, nx, ny, nw, nh, false);
        return;
      }

      if (editor.isDragging && editor.dragElementId) {
        const dx = (pointer.x - editor.dragStartX) / editor.zoom;
        const dy = (pointer.y - editor.dragStartY) / editor.zoom;
        const newDragX = editor.snapCoordinate(editor.dragOrigX + dx);
        const newDragY = editor.snapCoordinate(editor.dragOrigY + dy);
        const snappedDx = newDragX - editor.dragOrigX;
        const snappedDy = newDragY - editor.dragOrigY;

        if (this.dragStartPositions.size > 1) {
          const moves = [...this.dragStartPositions].map(([id, start]) => ({
            id,
            x: start.x + snappedDx,
            y: start.y + snappedDy,
          }));
          project.moveElements(moves, false);
        } else {
          const start = this.dragStartPositions.get(editor.dragElementId);
          if (start) {
            project.moveElement(editor.dragElementId, start.x + snappedDx, start.y + snappedDy, false);
          }
        }
      }
    };

    const onPointerUp = () => {
      this.isPanning = false;
      if (editor.isResizing && editor.resizeElementId) {
        const el = project.elementById(editor.resizeElementId);
        if (el) {
          const w = el.width ?? el.size ?? 18;
          const h = el.height ?? el.size ?? 18;
          project.resizeElement(editor.resizeElementId, el.x, el.y, w, h, true);
        }
      }
      if (editor.isDragging && editor.dragElementId) {
        project.commitMovedElements(this.dragStartPositions.keys());
      }
      this.dragStartPositions.clear();
      editor.isDragging = false;
      editor.dragElementId = null;
      editor.isResizing = false;
      editor.resizeElementId = null;
      editor.resizeCorner = null;
    };

    const onPointerDown = (e: CanvasPointerEvent) => {
      const pointer = this.pointerCanvasPosition(e);
      if (isPanMode()) {
        this.isPanning = true;
        this.panStartX = pointer.x;
        this.panStartY = pointer.y;
        this.panOrigX = editor.panX;
        this.panOrigY = editor.panY;
        return;
      }

      const gui = this.screenToGui(pointer.x, pointer.y);
      const shiftHeld = e.shiftKey;

      // Check resize handles on selected element first
      if (editor.tool === "select" && editor.selectedElementId && !shiftHeld) {
        const selEl = project.elementById(editor.selectedElementId);
        if (selEl) {
          const corner = this.hitTestHandle(selEl, gui.x, gui.y);
          if (corner) {
            const bounds = project.getElementBounds(editor.selectedElementId)!;
            editor.startResize(editor.selectedElementId, corner, pointer.x, pointer.y, bounds.x, bounds.y, bounds.w, bounds.h);
            return;
          }
        }
      }

      // Find clicked element: higher layers first, then later elements within the same layer.
      const sortedForHit = project.elements
        .map((el, index) => ({ el, index }))
        .sort((a, b) => {
          const aLayer = LAYER_ORDER[a.el.layer ?? "background"] ?? 0;
          const bLayer = LAYER_ORDER[b.el.layer ?? "background"] ?? 0;
          const layerDiff = bLayer - aLayer;
          return layerDiff !== 0 ? layerDiff : b.index - a.index;
        });
      const clicked = sortedForHit.find(({ el }) => this.hitTest(el, gui.x, gui.y))?.el;

      if (clicked && editor.tool === "select") {
        const keepMultiSelection = !shiftHeld && editor.selectedIds.size > 1 && editor.selectedIds.has(clicked.id);
        if (!keepMultiSelection) {
          editor.selectElement(clicked.id, shiftHeld);
        }
        const dragElementIds = editor.selectedIds.size > 1
          ? project.movementIdsForElements(editor.selectedIds)
          : project.movementIdsForElement(clicked.id);
        this.dragStartPositions = new Map(
          dragElementIds.map(id => {
            const el = project.elementById(id);
            return [id, { x: el?.x ?? 0, y: el?.y ?? 0 }];
          }),
        );
        editor.startDragElementAt(clicked.id, pointer.x, pointer.y, clicked.x, clicked.y);
      } else if (editor.tool === "select" && !clicked && !shiftHeld) {
        editor.clearSelection();
      } else {
        switch (editor.tool) {
          case "slot":
          case "texture":
          case "text":
            void project.addElement(editor.tool, gui.x, gui.y);
            break;
          case "button":
            void project.addElement("button", gui.x, gui.y);
            editor.tool = "select";
            break;
          case "toggle_button":
            void project.addElement("toggle_button", gui.x, gui.y);
            editor.tool = "select";
            break;
        }
      }
    };

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      if (e.ctrlKey || e.metaKey) {
        // Zoom
        const oldZoom = editor.zoom;
        if (e.deltaY < 0) editor.zoomIn();
        else editor.zoomOut();

        // Zoom toward cursor position
        if (editor.zoom !== oldZoom) {
          const ratio = editor.zoom / oldZoom;
          editor.panX = e.offsetX - ratio * (e.offsetX - editor.panX);
          editor.panY = e.offsetY - ratio * (e.offsetY - editor.panY);
        }
      } else {
        // Pan
        editor.panX -= e.deltaX;
        editor.panY -= e.deltaY;
      }
      this.updateTransform();
    };

    const isEditableTarget = (target: EventTarget | null): boolean => {
      if (!(target instanceof HTMLElement)) return false;
      return target instanceof HTMLInputElement
        || target instanceof HTMLSelectElement
        || target instanceof HTMLTextAreaElement
        || target.isContentEditable
        || target.closest("[contenteditable='true']") !== null
        || target.closest('[role="dialog"]') !== null;
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.code !== "Space" || isEditableTarget(e.target)) return;
      this.spacePanning = true;
      e.preventDefault();
      if (this.app.canvas instanceof HTMLCanvasElement && !this.isPanning) {
        this.app.canvas.style.cursor = "grab";
      }
    };

    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code !== "Space") return;
      this.spacePanning = false;
      e.preventDefault();
      if (this.app.canvas instanceof HTMLCanvasElement && !this.isPanning) {
        this.app.canvas.style.cursor = "crosshair";
      }
    };

    stage.addEventListener("pointermove", onPointerMove as never);
    stage.addEventListener("pointerup", onPointerUp as never);
    stage.addEventListener("pointerupoutside", onPointerUp as never);
    stage.addEventListener("pointerdown", onPointerDown as never);
    this.app.canvas.addEventListener("wheel", onWheel, { passive: false });
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);

    this.cleanupFns.push(() => {
      stage.removeEventListener("pointermove", onPointerMove as never);
      stage.removeEventListener("pointerup", onPointerUp as never);
      stage.removeEventListener("pointerupoutside", onPointerUp as never);
      stage.removeEventListener("pointerdown", onPointerDown as never);
      this.app.canvas.removeEventListener("wheel", onWheel);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    });
  }

  private hitTest(el: Element, gx: number, gy: number): boolean {
    const bounds = this.elementBounds(el);
    return gx >= bounds.x && gx <= bounds.x + bounds.w && gy >= bounds.y && gy <= bounds.y + bounds.h;
  }

  private hitTestHandle(el: Element, gx: number, gy: number): "tl" | "tr" | "bl" | "br" | null {
    const bounds = this.elementBounds(el);
    const w = bounds.w;
    const h = bounds.h;
    const HS = 5; // handle size in gui pixels (scaled by zoom)

    const handles: [number, number, "tl" | "tr" | "bl" | "br"][] = [
      [bounds.x - 1, bounds.y - 1, "tl"],
      [bounds.x + w - HS + 1, bounds.y - 1, "tr"],
      [bounds.x - 1, bounds.y + h - HS + 1, "bl"],
      [bounds.x + w - HS + 1, bounds.y + h - HS + 1, "br"],
    ];

    for (const [hx, hy, corner] of handles) {
      if (gx >= hx && gx <= hx + HS && gy >= hy && gy <= hy + HS) {
        return corner;
      }
    }
    return null;
  }

  private elementBounds(el: Element): { x: number; y: number; w: number; h: number } {
    if (el.type === "text") {
      return {
        x: el.x,
        y: el.y,
        ...this.textBounds(el),
      };
    }

    return {
      x: el.x,
      y: el.y,
      w: el.width ?? el.size ?? 18,
      h: el.height ?? el.size ?? 18,
    };
  }

  private textBounds(el: Element): { w: number; h: number } {
    const content = el.content ?? "Text";
    if (typeof document === "undefined") {
      return { w: Math.max(1, content.length * 5 + 2), h: 10 };
    }

    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (!ctx) return { w: Math.max(1, content.length * 5 + 2), h: 10 };

    ctx.font = "8px monospace";
    return {
      w: Math.max(1, Math.ceil(ctx.measureText(content).width) + 2),
      h: 10,
    };
  }

  private screenToGui(sx: number, sy: number): { x: number; y: number } {
    return {
      x: Math.floor((sx - editor.panX) / editor.zoom),
      y: Math.floor((sy - editor.panY) / editor.zoom),
    };
  }

  private pointerCanvasPosition(e: CanvasPointerEvent): { x: number; y: number } {
    if (e.global && Number.isFinite(e.global.x) && Number.isFinite(e.global.y)) {
      return { x: e.global.x, y: e.global.y };
    }

    if (Number.isFinite(e.offsetX) && Number.isFinite(e.offsetY)) {
      return { x: e.offsetX, y: e.offsetY };
    }

    const rect = this.app.canvas.getBoundingClientRect();
    return {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    };
  }

  render() {
    if (!this.ready || this.disposed) return;
    this.app.renderer.background.color = canvasPalette().background;
    this.updateTransform();
    this.syncGlyphTextureCache();
    this.drawGrid();
    this.drawElements();
    this.drawSelection();
    this.requestFrame();
  }

  private drawGrid() {
    this.gridContainer.removeChildren();
    if (!editor.showGrid) return;

    const g = new Graphics();
    const { width: gw, height: gh } = project.guiSize;
    const majorGridSize = Math.max(1, preferences.values.majorGridSize);
    const minorGridSize = Math.max(1, preferences.values.minorGridSize);
    const palette = canvasPalette();

    // Background fill for GUI area
    g.rect(0, 0, gw, gh);
    g.fill({ color: palette.guiFill, alpha: palette.guiAlpha });

    // Draw GUI border
    g.rect(-1, -1, gw + 2, gh + 2);
    g.stroke({ width: 1, color: palette.border, alpha: palette.borderAlpha });

    // Minor grid
    const minorAlpha = palette.minorAlpha;
    if (minorAlpha > 0) {
      for (let x = 0; x <= gw; x += minorGridSize) {
        g.moveTo(x, 0);
        g.lineTo(x, gh);
        g.stroke({ width: 1, color: palette.line, alpha: minorAlpha });
      }
      for (let y = 0; y <= gh; y += minorGridSize) {
        g.moveTo(0, y);
        g.lineTo(gw, y);
        g.stroke({ width: 1, color: palette.line, alpha: minorAlpha });
      }
    }

    // Major grid (always visible at zoom >= 1)
    const majorAlpha = palette.majorAlpha;
    for (let x = 0; x <= gw; x += majorGridSize) {
      g.moveTo(x, 0);
      g.lineTo(x, gh);
      g.stroke({ width: 1, color: palette.line, alpha: majorAlpha });
    }
    for (let y = 0; y <= gh; y += majorGridSize) {
      g.moveTo(0, y);
      g.lineTo(gw, y);
      g.stroke({ width: 1, color: palette.line, alpha: majorAlpha });
    }

    this.gridContainer.addChild(g);
  }

  private drawElements() {
    this.elementsContainer.removeChildren();

    // Sort by layer priority: Background first, then Overlay, then Animatable
    const sorted = [...project.elements].sort((a, b) => {
      const aLayer = LAYER_ORDER[a.layer ?? "background"] ?? 0;
      const bLayer = LAYER_ORDER[b.layer ?? "background"] ?? 0;
      return aLayer - bLayer;
    });

    for (const el of sorted) {
      let g: Container | null = null;
      try {
        g = this.drawElement(el);
      } catch (error) {
        console.error("Failed to draw element", el, error);
      }
      if (g) this.elementsContainer.addChild(g);
    }
  }

  private drawElement(el: Element): Container | null {
    switch (el.type) {
      case "slot":
      case "virtual_slot_cell":
        return this.drawSlot(el);
      case "texture":
        return this.drawTexture(el);
      case "progress":
        return this.drawProgress(el);
      case "text":
        return this.drawText(el);
      case "fluid_tank":
        return this.drawFluidTank(el);
      case "energy_bar":
        return this.drawEnergyBar(el);
      case "scrollbar":
        return this.drawScrollbar(el);
      case "button":
      case "toggle_button":
        return this.drawButton(el);
      default:
        return null;
    }
  }

  private drawSlot(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    const s = el.size ?? 18;
    g.rect(el.x, el.y, s, s);
    g.fill({ color: 0x8b8b8b });
    g.stroke({ width: 1, color: 0x373737 });
    // Inner highlight
    g.rect(el.x + 1, el.y + 1, s - 2, s - 2);
    g.stroke({ width: 1, color: 0xffffff, alpha: 0.3 });
    container.addChild(g);
    return container;
  }

  private drawTexture(el: Element): Container | null {
    if (isGeneratedTexturePath(el.asset)) {
      return this.drawGeneratedTextureFallback(el);
    }

    // Try to render as actual texture
    if (el.asset) {
      const dataUrl = assetDataUrls.get(el.asset);
      if (dataUrl) {
        const container = new Container();
        const baseTexture = Texture.from(dataUrl);
        const texture = this.textureWithUv(baseTexture, el);
        if (!texture) return null;
        const sprite = new Sprite(texture);
        sprite.x = el.x;
        sprite.y = el.y;
        if (el.width) sprite.width = el.width;
        if (el.height) sprite.height = el.height;
        container.addChild(sprite);
        return container;
      }
    }

    // Fallback placeholder (checkerboard)
    const container = new Container();
    const g = new Graphics();
    const w = el.width ?? 16;
    const h = el.height ?? 16;
    const cs = 4;
    for (let px = el.x; px < el.x + w; px += cs) {
      for (let py = el.y; py < el.y + h; py += cs) {
        const even = ((px - el.x) / cs + (py - el.y) / cs) % 2 === 0;
        g.rect(px, py, cs, cs);
        g.fill({ color: even ? 0x555577 : 0x444466 });
      }
    }
    g.rect(el.x, el.y, w, h);
    g.stroke({ width: 1, color: 0x8888cc, alpha: 0.6 });
    container.addChild(g);
    return container;
  }

  private drawGeneratedTextureFallback(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    const w = el.width ?? 16;
    const h = el.height ?? 16;

    if (el.asset === "textures/generated/gui_panel.png" || el.asset === LEGACY_BACKGROUND_TEXTURE) {
      g.rect(el.x, el.y, w, h);
      g.fill({ color: 0xb8b8b8 });
      g.rect(el.x, el.y, w, 1);
      g.fill({ color: 0xffffff });
      g.rect(el.x, el.y, 1, h);
      g.fill({ color: 0xffffff });
      g.rect(el.x, el.y + h - 1, w, 1);
      g.fill({ color: 0x555555 });
      g.rect(el.x + w - 1, el.y, 1, h);
      g.fill({ color: 0x555555 });
    } else if (el.asset === "textures/generated/slot.png") {
      g.rect(el.x, el.y, w, h);
      g.fill({ color: 0x8b8b8b });
      g.rect(el.x, el.y, w, 1);
      g.fill({ color: 0x373737 });
      g.rect(el.x, el.y, 1, h);
      g.fill({ color: 0x373737 });
      g.rect(el.x, el.y + h - 1, w, 1);
      g.fill({ color: 0xffffff, alpha: 0.8 });
      g.rect(el.x + w - 1, el.y, 1, h);
      g.fill({ color: 0xffffff, alpha: 0.8 });
    } else if (el.asset === "textures/generated/button.png") {
      this.drawButtonGraphics(g, el.x, el.y, w, h);
    } else if (el.asset === "textures/generated/scrollbar.png") {
      this.drawScrollbarGraphics(g, el.x, el.y, w, h);
    } else {
      g.rect(el.x, el.y, w, h);
      g.fill({ color: 0x6f6f6f });
      g.rect(el.x, el.y, w, h);
      g.stroke({ width: 1, color: 0x373737 });
    }

    container.addChild(g);
    return container;
  }

  private textureWithUv(baseTexture: Texture, el: Element): Texture | null {
    if (!el.uv || el.uv.width <= 0 || el.uv.height <= 0) {
      return baseTexture;
    }

    const sourceWidth = baseTexture.source.width;
    const sourceHeight = baseTexture.source.height;
    if (sourceWidth <= 0 || sourceHeight <= 0) {
      return baseTexture;
    }
    const x = Math.max(0, Math.min(el.uv.x, sourceWidth));
    const y = Math.max(0, Math.min(el.uv.y, sourceHeight));
    const width = Math.min(el.uv.width, sourceWidth - x);
    const height = Math.min(el.uv.height, sourceHeight - y);
    if (width <= 0 || height <= 0) {
      return null;
    }

    return new Texture({
      source: baseTexture.source,
      frame: new Rectangle(x, y, width, height),
    });
  }

  private drawButton(el: Element): Container {
    const container = new Container();
    const background = el.asset ? this.drawTexture(el) : this.drawButtonBackground(el);
    if (background) container.addChild(background);

    const label = this.drawButtonLabel(el);
    if (label) container.addChild(label);
    return container;
  }

  private drawButtonBackground(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    const w = el.width ?? el.size ?? 40;
    const h = el.height ?? el.size ?? 20;
    this.drawButtonGraphics(g, el.x, el.y, w, h);
    container.addChild(g);
    return container;
  }

  private drawButtonGraphics(g: Graphics, x: number, y: number, w: number, h: number) {
    g.rect(x, y, w, h);
    g.fill({ color: 0x9a9a9a });
    g.rect(x, y, w, 1);
    g.fill({ color: 0x373737 });
    g.rect(x, y, 1, h);
    g.fill({ color: 0x373737 });
    if (h > 1) {
      g.rect(x, y + h - 1, w, 1);
      g.fill({ color: 0x555555 });
    }
    if (w > 1) {
      g.rect(x + w - 1, y, 1, h);
      g.fill({ color: 0x555555 });
    }
    if (w > 2 && h > 2) {
      g.rect(x + 1, y + 1, w - 2, 1);
      g.fill({ color: 0xffffff, alpha: 0.9 });
      g.rect(x + 1, y + 1, 1, h - 2);
      g.fill({ color: 0xffffff, alpha: 0.9 });
      g.rect(x + 1, y + h - 2, w - 2, 1);
      g.fill({ color: 0x6b6b6b });
      g.rect(x + w - 2, y + 1, 1, h - 2);
      g.fill({ color: 0x6b6b6b });
    }
  }

  private drawButtonLabel(el: Element): Container | null {
    const content = el.content?.trim();
    if (!content) return null;

    const w = el.width ?? el.size ?? 40;
    const h = el.height ?? el.size ?? 20;
    const labelElement: Element = {
      ...el,
      type: "text",
      content,
      x: 0,
      y: 0,
      color: el.color ?? 0x404040,
      shadow: el.shadow ?? false,
    };

    const glyphMetrics = this.glyphTextMetrics(labelElement);
    if (glyphMetrics) {
      const glyphLabel = this.drawGlyphText({
        ...labelElement,
        x: Math.floor(el.x + (w - glyphMetrics.width) / 2 - glyphMetrics.minX),
        y: Math.floor(el.y + (h - glyphMetrics.height) / 2 - glyphMetrics.minY),
      });
      if (glyphLabel) return glyphLabel;
    }

    const texture = this.textTexture(labelElement);
    if (!texture) return null;

    const sprite = new Sprite(texture);
    sprite.x = Math.floor(el.x + (w - texture.width) / 2);
    sprite.y = Math.floor(el.y + (h - texture.height) / 2);

    const container = new Container();
    container.addChild(sprite);
    return container;
  }

  private glyphTextMetrics(el: Element): { minX: number; minY: number; width: number; height: number } | null {
    const fontId = el.font ?? "minecraft:default";
    const renderData = project.fontRenderData.get(fontId);
    if (!renderData || !renderData.glyph_map) return null;

    const text = el.content ?? "{text}";
    const glyphs = renderData.glyph_map;
    const lineAscent = this.textLineAscent(renderData, text);
    let cursorX = 0;
    let minX = Number.POSITIVE_INFINITY;
    let minY = Number.POSITIVE_INFINITY;
    let maxX = Number.NEGATIVE_INFINITY;
    let maxY = Number.NEGATIVE_INFINITY;

    for (const ch of Array.from(text)) {
      const glyph = glyphs[ch];
      if (!glyph) return null;

      const advance = this.glyphAdvance(glyph);
      if (glyph.width > 0 && glyph.height > 0) {
        const x = cursorX + (glyph.bearing_x ?? 0);
        const y = this.glyphYOffset(renderData, glyph, lineAscent);
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x + glyph.width);
        maxY = Math.max(maxY, y + glyph.height);
      }
      cursorX += advance;
    }

    if (!Number.isFinite(minX) || !Number.isFinite(minY)) {
      return { minX: 0, minY: 0, width: Math.max(1, cursorX), height: 8 };
    }

    const shadowOffset = el.shadow ? 1 : 0;
    return {
      minX,
      minY,
      width: Math.max(1, maxX - minX + shadowOffset),
      height: Math.max(1, maxY - minY + shadowOffset),
    };
  }

  private drawProgress(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    const w = el.width ?? 22;
    const h = el.height ?? 15;
    // Background
    g.rect(el.x, el.y, w, h);
    g.fill({ color: 0x333333 });
    g.stroke({ width: 1, color: 0x555555 });
    // Arrow indicator
    const arrowColor = 0xe9a23b;
    if (el.direction === "left_to_right" || !el.direction) {
      // Arrow pointing right
      const midY = el.y + h / 2;
      g.moveTo(el.x + 3, midY - 3);
      g.lineTo(el.x + w - 3, midY);
      g.lineTo(el.x + 3, midY + 3);
      g.closePath();
      g.fill({ color: arrowColor, alpha: 0.5 });
    } else if (el.direction === "bottom_to_top") {
      const midX = el.x + w / 2;
      g.moveTo(midX - 3, el.y + h - 3);
      g.lineTo(midX, el.y + 3);
      g.lineTo(midX + 3, el.y + h - 3);
      g.closePath();
      g.fill({ color: arrowColor, alpha: 0.5 });
    }
    container.addChild(g);
    return container;
  }

  private drawText(el: Element): Container {
    const glyphText = this.drawGlyphText(el);
    return glyphText ?? this.drawCanvasText(el);
  }

  private drawCanvasText(el: Element): Container {
    const container = new Container();
    const texture = this.textTexture(el);
    if (!texture) return container;

    const sprite = new Sprite(texture);
    sprite.x = el.x;
    sprite.y = el.y;
    container.addChild(sprite);
    return container;
  }

  private textTexture(el: Element): Texture | null {
    const content = el.content ?? "Text";
    const color = el.color ?? 0x404040;
    const shadow = el.shadow ?? false;
    const cacheKey = `${content}|${color}|${shadow}`;
    const cached = this.textTextureCache.get(cacheKey);
    if (cached) return cached;

    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (!ctx) return null;

    const { w, h } = this.textBounds(el);
    canvas.width = w;
    canvas.height = h;

    ctx.imageSmoothingEnabled = false;
    ctx.font = "8px monospace";
    ctx.textBaseline = "top";
    if (shadow) {
      ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
      ctx.fillText(content, 1, 1);
    }
    ctx.fillStyle = `#${color.toString(16).padStart(6, "0")}`;
    ctx.fillText(content, 0, 0);

    const texture = Texture.from(canvas);
    this.textTextureCache.set(cacheKey, texture);
    return texture;
  }

  private drawGlyphText(el: Element): Container | null {
    const fontId = el.font ?? "minecraft:default";
    const renderData = project.fontRenderData.get(fontId);
    if (!renderData || !renderData.glyph_map) return null;

    const text = el.content ?? "{text}";
    const glyphs = renderData.glyph_map;
    const lineAscent = this.textLineAscent(renderData, text);
    const container = new Container();

    if (el.shadow) {
      const shadow = this.buildGlyphLine(renderData, glyphs, text, el.x + 1, el.y + 1, 0x000000, 0.5, lineAscent);
      if (!shadow) return null;
      container.addChild(shadow);
    }

    const main = this.buildGlyphLine(renderData, glyphs, text, el.x, el.y, el.color ?? 0x404040, 1, lineAscent);
    if (!main) return null;
    container.addChild(main);
    return container;
  }

  private buildGlyphLine(
    renderData: FontRenderData,
    glyphs: Record<string, GlyphInfo>,
    text: string,
    x: number,
    y: number,
    tint: number,
    alpha: number,
    lineAscent: number,
  ): Container | null {
    const container = new Container();
    let cursorX = x;

    for (const ch of Array.from(text)) {
      const glyph = glyphs[ch];
      if (!glyph) return null;

      const advance = this.glyphAdvance(glyph);
      if (glyph.width <= 0 || glyph.height <= 0) {
        cursorX += advance;
        continue;
      }

      const texture = this.glyphTexture(renderData, ch, glyph);
      if (!texture) return null;

      const sprite = new Sprite(texture);
      sprite.x = cursorX + (glyph.bearing_x ?? 0);
      sprite.y = y + this.glyphYOffset(renderData, glyph, lineAscent);
      sprite.tint = tint;
      sprite.alpha = alpha;
      container.addChild(sprite);
      cursorX += advance;
    }

    return container;
  }

  private glyphAdvance(glyph: GlyphInfo): number {
    if (glyph.width <= 0 || glyph.height <= 0) {
      return glyph.advance && glyph.advance > 0 ? glyph.advance : 4;
    }
    return glyph.advance && glyph.advance > 0 ? glyph.advance : Math.max(glyph.width, 1);
  }

  private glyphTexture(renderData: FontRenderData, ch: string, glyph: GlyphInfo): Texture | null {
    const source = this.glyphSource(renderData, ch);
    const dataUrl = source?.dataUrl;
    if (!dataUrl) return null;

    const cacheKey = [
      renderData.id,
      renderData.source_type,
      source.identity,
      ch,
      glyph.x,
      glyph.y,
      glyph.width,
      glyph.height,
    ].join("|");
    const cached = this.glyphTextureCache.get(cacheKey);
    if (cached) return cached;

    const baseTexture = this.fontSourceTexture(source.identity, dataUrl);
    if (!baseTexture) return null;

    if (baseTexture.source.width <= 0 || baseTexture.source.height <= 0) return null;

    if (glyph.x >= baseTexture.source.width || glyph.y >= baseTexture.source.height) return null;

    const x = Math.max(0, glyph.x);
    const y = Math.max(0, glyph.y);
    const width = Math.max(1, Math.min(glyph.width, baseTexture.source.width - x));
    const height = Math.max(1, Math.min(glyph.height, baseTexture.source.height - y));

    const texture = new Texture({
      source: baseTexture.source,
      frame: new Rectangle(x, y, width, height),
    });
    this.glyphTextureCache.set(cacheKey, texture);
    return texture;
  }

  private fontSourceTexture(identity: string, dataUrl: string): Texture | null {
    const cached = this.fontSourceTextureCache.get(identity);
    if (cached) return cached;

    if (Assets.cache.has(dataUrl)) {
      const texture = Assets.get<Texture>(dataUrl);
      if (texture.source.width > 0 && texture.source.height > 0) {
        this.fontSourceTextureCache.set(identity, texture);
        return texture;
      }
    }

    if (!this.loadingFontSources.has(identity)) {
      this.loadingFontSources.add(identity);
      Assets.load<Texture>(dataUrl)
        .then(loaded => {
          if (this.disposed || !this.ready || !this.loadingFontSources.has(identity)) return;
          if (loaded.source.width <= 0 || loaded.source.height <= 0) return;

          this.fontSourceTextureCache.set(identity, loaded);
          this.glyphTextureCache.clear();
          this.render();
        })
        .catch(() => {
          // Glyph rendering will continue falling back to canvas text until a later render retries.
        })
        .finally(() => {
          this.loadingFontSources.delete(identity);
        });
    }

    return null;
  }

  private providerForGlyph(providers: MinecraftFontProviderRenderData[], ch: string): MinecraftFontProviderRenderData | undefined {
    return providers.find(provider => provider.chars.some(row => Array.from(row).includes(ch)));
  }

  private glyphSource(renderData: FontRenderData, ch: string): { dataUrl: string; identity: string } | null {
    if (renderData.source_type === "ttf") {
      return { dataUrl: renderData.atlas_data_url, identity: renderData.atlas_data_url };
    }

    const provider = this.providerForGlyph(renderData.providers, ch);
    if (!provider) return null;
    return {
      dataUrl: provider.image_data_url,
      identity: `${provider.file}:${provider.image_width}x${provider.image_height}:${provider.image_data_url}`,
    };
  }

  private syncGlyphTextureCache() {
    if (this.glyphTextureCacheVersion === project.fontRenderDataVersion) return;
    this.glyphTextureCache.clear();
    this.fontSourceTextureCache.clear();
    this.loadingFontSources.clear();
    this.glyphTextureCacheVersion = project.fontRenderDataVersion;
  }

  private textLineAscent(renderData: FontRenderData, text: string): number {
    if (renderData.source_type === "minecraft") return 0;

    let ascent = renderData.font_size;
    for (const ch of Array.from(text)) {
      const glyph = renderData.glyph_map[ch];
      if (glyph) ascent = Math.max(ascent, glyph.ascent);
    }
    return ascent;
  }

  private glyphYOffset(renderData: FontRenderData, glyph: GlyphInfo, lineAscent: number): number {
    if (renderData.source_type === "minecraft") return glyph.bearing_y ?? 0;
    return lineAscent + (glyph.bearing_y ?? -glyph.ascent);
  }

  private drawFluidTank(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    const w = el.width ?? 18;
    const h = el.height ?? 60;
    g.rect(el.x, el.y, w, h);
    g.fill({ color: 0x1a1a2e });
    g.stroke({ width: 1, color: 0x555555 });
    // Fill indicator (half full for preview)
    g.rect(el.x + 2, el.y + h * 0.3, w - 4, h * 0.7 - 2);
    g.fill({ color: 0x3b82e9, alpha: 0.5 });
    container.addChild(g);
    return container;
  }

  private drawEnergyBar(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    const w = el.width ?? 14;
    const h = el.height ?? 50;
    g.rect(el.x, el.y, w, h);
    g.fill({ color: 0x1a1a2e });
    g.stroke({ width: 1, color: 0x555555 });
    // Energy indicator
    g.rect(el.x + 2, el.y + h * 0.4, w - 4, h * 0.6 - 2);
    g.fill({ color: 0xe94560, alpha: 0.5 });
    container.addChild(g);
    return container;
  }

  private drawScrollbar(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    this.drawScrollbarGraphics(g, el.x, el.y, el.width ?? 12, el.height ?? 54);
    container.addChild(g);
    return container;
  }

  private drawScrollbarGraphics(g: Graphics, x: number, y: number, width: number, height: number) {
    const w = Math.max(5, width);
    const h = Math.max(9, height);
    g.rect(x, y, w, h);
    g.fill({ color: 0x6f6f6f });
    g.rect(x, y, w, h);
    g.stroke({ width: 1, color: 0x2f2f2f });

    const thumbW = Math.max(1, w - 4);
    const thumbH = Math.min(Math.max(5, Math.floor((h - 4) / 3)), Math.max(1, h - 4));
    g.rect(x + 2, y + 2, thumbW, thumbH);
    g.fill({ color: 0xb8b8b8 });
    g.rect(x + 2, y + 2, thumbW, thumbH);
    g.stroke({ width: 1, color: 0xffffff, alpha: 0.35 });
  }

  private drawSelection() {
    this.selectionGraphics.clear();

    for (const selId of editor.selectedIds) {
      const el = project.elementById(selId);
      if (!el) continue;

      const bounds = this.elementBounds(el);
      const w = bounds.w;
      const h = bounds.h;
      const g = this.selectionGraphics;

      const isPrimary = selId === editor.selectedElementId;
      const tint = isPrimary ? SELECTED_TINT : 0x888800;

      g.rect(bounds.x - 1, bounds.y - 1, w + 2, h + 2);
      g.stroke({ width: 1, color: tint });

      // Corner handles only on primary selection
      if (isPrimary) {
        const hs = Math.max(3, Math.round(8 / editor.zoom));
        const corners: [number, number][] = [
          [bounds.x - 1, bounds.y - 1],
          [bounds.x + w - hs + 1, bounds.y - 1],
          [bounds.x - 1, bounds.y + h - hs + 1],
          [bounds.x + w - hs + 1, bounds.y + h - hs + 1],
        ];
        for (const [cx, cy] of corners) {
          g.rect(cx, cy, hs, hs);
          g.fill({ color: tint });
        }
      }
    }
  }

  private updateCursorLabel(gx: number, gy: number) {
    if (gx < 0 || gy < 0) {
      this.cursorLabel.text = "";
    } else {
      this.cursorLabel.text = `${gx}, ${gy}`;
    }
    this.cursorLabel.x = 4;
    this.cursorLabel.y = this.app.renderer.height / (window.devicePixelRatio || 1) - 16;
  }

  updateTransform() {
    if (!this.ready || this.disposed) return;
    const scale = editor.zoom;
    this.elementsContainer.scale.set(scale);
    this.gridContainer.scale.set(scale);
    this.overlayContainer.scale.set(scale);

    this.elementsContainer.position.set(editor.panX, editor.panY);
    this.gridContainer.position.set(editor.panX, editor.panY);
    this.overlayContainer.position.set(editor.panX, editor.panY);
    this.requestFrame();
  }

  private requestFrame() {
    if (!this.ready || this.disposed) return;
    if (this.renderFrame !== null) return;
    this.renderFrame = requestAnimationFrame(() => {
      this.renderFrame = null;
      if (!this.ready || this.disposed) return;
      this.app.render();
    });
  }

  destroy() {
    if (this.disposed) return;
    this.disposed = true;
    this.ready = false;

    if (this.renderFrame !== null) {
      cancelAnimationFrame(this.renderFrame);
      this.renderFrame = null;
    }
    this.loadingFontSources.clear();
    this.textTextureCache.forEach(texture => texture.destroy(true));
    this.textTextureCache.clear();
    this.cleanupFns.forEach(fn => fn());
    this.cleanupFns = [];
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;

    if (this.pixiInitialized) {
      this.destroyPixiApp();
    }
  }

  private destroyPixiApp() {
    if (this.appDestroyed || !this.pixiInitialized) return;
    this.appDestroyed = true;
    this.app.destroy(true, { children: true });
  }
}
