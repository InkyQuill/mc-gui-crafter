import { Application, Container, Graphics, Rectangle, Text, TextStyle, Sprite, Texture } from "pixi.js";
import type { Element } from "../types";
import { project, assetDataUrls } from "../stores/project.svelte";
import { editor } from "../stores/editor.svelte";

const GRID_MAJOR = 18;
const GRID_MINOR = 2;

const SELECTED_TINT = 0xffff00;

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
    const rect = this.containerEl.getBoundingClientRect();
    editor.canvasWidth = rect.width;
    editor.canvasHeight = rect.height;

    await this.app.init({
      width: rect.width,
      height: rect.height,
      backgroundColor: 0x12121f,
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
    });

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
    this.render();
  }

  private setupResizeObserver() {
    this.resizeObserver = new ResizeObserver(() => {
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

    const onPointerMove = (e: PointerEvent) => {
      const gui = this.screenToGui(e.offsetX, e.offsetY);
      editor.mouseGuiX = gui.x;
      editor.mouseGuiY = gui.y;
      this.updateCursorLabel(gui.x, gui.y);

      // Update cursor for resize handles
      let cursor = "crosshair";
      if (editor.isResizing) {
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

      if (editor.isResizing && editor.resizeElementId && editor.resizeCorner) {
        const dx = (e.offsetX - editor.resizeStartScreenX) / editor.zoom;
        const dy = (e.offsetY - editor.resizeStartScreenY) / editor.zoom;
        const ox = editor.resizeOrigX;
        const oy = editor.resizeOrigY;
        const ow = editor.resizeOrigW;
        const oh = editor.resizeOrigH;

        let nx: number, ny: number, nw: number, nh: number;
        switch (editor.resizeCorner) {
          case "tl":
            nx = Math.round(ox + dx);
            ny = Math.round(oy + dy);
            nw = Math.max(4, ow - (nx - ox));
            nh = Math.max(4, oh - (ny - oy));
            break;
          case "tr":
            nx = ox;
            ny = Math.round(oy + dy);
            nw = Math.max(4, ow + dx);
            nh = Math.max(4, oh - (ny - oy));
            break;
          case "bl":
            nx = Math.round(ox + dx);
            ny = oy;
            nw = Math.max(4, ow - (nx - ox));
            nh = Math.max(4, oh + dy);
            break;
          case "br":
            nx = ox;
            ny = oy;
            nw = Math.max(4, Math.round(ow + dx));
            nh = Math.max(4, Math.round(oh + dy));
            break;
        }

        project.resizeElement(editor.resizeElementId, nx, ny, nw, nh, false);
        return;
      }

      if (editor.isDragging && editor.dragElementId) {
        const dx = (e.offsetX - editor.dragStartX) / editor.zoom;
        const dy = (e.offsetY - editor.dragStartY) / editor.zoom;

        if (editor.selectedIds.size > 1) {
          // Multi-drag: move all selected elements by the delta
          for (const id of editor.selectedIds) {
            const start = this.dragStartPositions.get(id);
            if (start) {
              const newX = Math.round(start.x + dx);
              const newY = Math.round(start.y + dy);
              project.moveElement(id, newX, newY, false);
            }
          }
        } else {
          const newX = Math.round(editor.dragOrigX + dx);
          const newY = Math.round(editor.dragOrigY + dy);
          project.moveElement(editor.dragElementId, newX, newY, false);
        }
      }
    };

    const onPointerUp = () => {
      if (editor.isResizing && editor.resizeElementId) {
        const el = project.elementById(editor.resizeElementId);
        if (el) {
          const w = el.width ?? el.size ?? 18;
          const h = el.height ?? el.size ?? 18;
          project.resizeElement(editor.resizeElementId, el.x, el.y, w, h, true);
        }
      }
      if (editor.isDragging && editor.dragElementId) {
        if (editor.selectedIds.size > 1) {
          project.commitMovedElements(editor.selectedIds);
        } else {
          const el = project.elementById(editor.dragElementId);
          if (el) {
            project.moveElement(editor.dragElementId, el.x, el.y, true);
          }
        }
      }
      this.dragStartPositions.clear();
      editor.isDragging = false;
      editor.dragElementId = null;
      editor.isResizing = false;
      editor.resizeElementId = null;
      editor.resizeCorner = null;
    };

    const onPointerDown = (e: PointerEvent) => {
      const gui = this.screenToGui(e.offsetX, e.offsetY);
      const shiftHeld = e.shiftKey;

      // Check resize handles on selected element first
      if (editor.tool === "select" && editor.selectedElementId && !shiftHeld) {
        const selEl = project.elementById(editor.selectedElementId);
        if (selEl) {
          const corner = this.hitTestHandle(selEl, gui.x, gui.y);
          if (corner) {
            const bounds = project.getElementBounds(editor.selectedElementId)!;
            editor.startResize(editor.selectedElementId, corner, e.offsetX, e.offsetY, bounds.x, bounds.y, bounds.w, bounds.h);
            return;
          }
        }
      }

      // Find clicked element (reverse order = top-most first)
      const clicked = [...project.elements].reverse().find(el => this.hitTest(el, gui.x, gui.y));

      if (clicked && editor.tool === "select") {
        const keepMultiSelection = !shiftHeld && editor.selectedIds.size > 1 && editor.selectedIds.has(clicked.id);
        if (!keepMultiSelection) {
          editor.selectElement(clicked.id, shiftHeld);
        }
        this.dragStartPositions = new Map(
          [...editor.selectedIds].map(id => {
            const el = project.elementById(id);
            return [id, { x: el?.x ?? 0, y: el?.y ?? 0 }];
          }),
        );
        editor.startDragElement(clicked.id, e.offsetX, e.offsetY, this.containerEl);
      } else if (editor.tool === "select" && !clicked && !shiftHeld) {
        editor.clearSelection();
      } else if (editor.tool === "slot" || editor.tool === "texture" || editor.tool === "text") {
        // Place new element
        project.addElement(editor.tool, gui.x, gui.y);
        editor.tool = "select";
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
    };

    stage.addEventListener("pointermove", onPointerMove as never);
    stage.addEventListener("pointerup", onPointerUp as never);
    stage.addEventListener("pointerupoutside", onPointerUp as never);
    stage.addEventListener("pointerdown", onPointerDown as never);
    this.app.canvas.addEventListener("wheel", onWheel, { passive: false });

    this.cleanupFns.push(() => {
      stage.removeEventListener("pointermove", onPointerMove as never);
      stage.removeEventListener("pointerup", onPointerUp as never);
      stage.removeEventListener("pointerupoutside", onPointerUp as never);
      stage.removeEventListener("pointerdown", onPointerDown as never);
      this.app.canvas.removeEventListener("wheel", onWheel);
    });
  }

  private hitTest(el: Element, gx: number, gy: number): boolean {
    const w = el.width ?? el.size ?? 18;
    const h = el.height ?? el.size ?? 18;
    return gx >= el.x && gx <= el.x + w && gy >= el.y && gy <= el.y + h;
  }

  private hitTestHandle(el: Element, gx: number, gy: number): "tl" | "tr" | "bl" | "br" | null {
    const w = el.width ?? el.size ?? 18;
    const h = el.height ?? el.size ?? 18;
    const HS = 5; // handle size in gui pixels (scaled by zoom)

    const handles: [number, number, "tl" | "tr" | "bl" | "br"][] = [
      [el.x - 1, el.y - 1, "tl"],
      [el.x + w - HS + 1, el.y - 1, "tr"],
      [el.x - 1, el.y + h - HS + 1, "bl"],
      [el.x + w - HS + 1, el.y + h - HS + 1, "br"],
    ];

    for (const [hx, hy, corner] of handles) {
      if (gx >= hx && gx <= hx + HS && gy >= hy && gy <= hy + HS) {
        return corner;
      }
    }
    return null;
  }

  private screenToGui(sx: number, sy: number): { x: number; y: number } {
    return {
      x: Math.floor((sx - editor.panX) / editor.zoom),
      y: Math.floor((sy - editor.panY) / editor.zoom),
    };
  }

  render() {
    this.drawGrid();
    this.drawElements();
    this.drawSelection();
  }

  private drawGrid() {
    this.gridContainer.removeChildren();
    if (!editor.showGrid) return;

    const g = new Graphics();
    const { width: gw, height: gh } = project.guiSize;

    // Background fill for GUI area
    g.rect(0, 0, gw, gh);
    g.fill({ color: 0xc6c6c6, alpha: 0.3 });

    // Draw GUI border
    g.rect(-1, -1, gw + 2, gh + 2);
    g.stroke({ width: 1, color: 0xffffff, alpha: 0.5 });

    // Minor grid
    const minorAlpha = editor.zoom >= 2 ? 0.1 : 0;
    if (minorAlpha > 0) {
      for (let x = 0; x <= gw; x += GRID_MINOR) {
        g.moveTo(x, 0);
        g.lineTo(x, gh);
        g.stroke({ width: 1, color: 0xffffff, alpha: minorAlpha });
      }
      for (let y = 0; y <= gh; y += GRID_MINOR) {
        g.moveTo(0, y);
        g.lineTo(gw, y);
        g.stroke({ width: 1, color: 0xffffff, alpha: minorAlpha });
      }
    }

    // Major grid (always visible at zoom >= 1)
    const majorAlpha = 0.25;
    for (let x = 0; x <= gw; x += GRID_MAJOR) {
      g.moveTo(x, 0);
      g.lineTo(x, gh);
      g.stroke({ width: 1, color: 0xffffff, alpha: majorAlpha });
    }
    for (let y = 0; y <= gh; y += GRID_MAJOR) {
      g.moveTo(0, y);
      g.lineTo(gw, y);
      g.stroke({ width: 1, color: 0xffffff, alpha: majorAlpha });
    }

    this.gridContainer.addChild(g);
  }

  private drawElements() {
    this.elementsContainer.removeChildren();

    for (const el of project.elements) {
      const g = this.drawElement(el);
      if (g) this.elementsContainer.addChild(g);
    }
  }

  private drawElement(el: Element): Container | null {
    switch (el.type) {
      case "slot":
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
    // Try to render as actual texture
    if (el.asset) {
      const dataUrl = assetDataUrls.get(el.asset);
      if (dataUrl) {
        const container = new Container();
        const baseTexture = Texture.from(dataUrl);
        const texture = this.textureWithUv(baseTexture, el);
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

  private textureWithUv(baseTexture: Texture, el: Element): Texture {
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
    const width = Math.max(1, Math.min(el.uv.width, sourceWidth - x));
    const height = Math.max(1, Math.min(el.uv.height, sourceHeight - y));

    return new Texture({
      source: baseTexture.source,
      frame: new Rectangle(x, y, width, height),
    });
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
    const container = new Container();
    const text = new Text({
      text: el.content ?? "{text}",
      style: new TextStyle({
        fontSize: 8,
        fill: el.color ?? 0x404040,
        fontFamily: "monospace",
        dropShadow: el.shadow ? { alpha: 0.5, blur: 0, distance: 1, color: 0x000000 } : undefined,
      }),
    });
    text.x = el.x;
    text.y = el.y;
    container.addChild(text);
    return container;
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

  private drawSelection() {
    this.selectionGraphics.clear();

    for (const selId of editor.selectedIds) {
      const el = project.elementById(selId);
      if (!el) continue;

      const w = el.width ?? el.size ?? 18;
      const h = el.height ?? el.size ?? 18;
      const g = this.selectionGraphics;

      const isPrimary = selId === editor.selectedElementId;
      const tint = isPrimary ? SELECTED_TINT : 0x888800;

      g.rect(el.x - 1, el.y - 1, w + 2, h + 2);
      g.stroke({ width: 1, color: tint });

      // Corner handles only on primary selection
      if (isPrimary) {
        const hs = Math.max(3, Math.round(8 / editor.zoom));
        const corners: [number, number][] = [
          [el.x - 1, el.y - 1],
          [el.x + w - hs + 1, el.y - 1],
          [el.x - 1, el.y + h - hs + 1],
          [el.x + w - hs + 1, el.y + h - hs + 1],
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
    const scale = editor.zoom;
    this.elementsContainer.scale.set(scale);
    this.gridContainer.scale.set(scale);
    this.overlayContainer.scale.set(scale);

    this.elementsContainer.position.set(editor.panX, editor.panY);
    this.gridContainer.position.set(editor.panX, editor.panY);
    this.overlayContainer.position.set(editor.panX, editor.panY);
  }

  destroy() {
    this.cleanupFns.forEach(fn => fn());
    this.resizeObserver?.disconnect();
    this.app.destroy(true, { children: true });
  }
}
