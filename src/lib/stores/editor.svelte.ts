import { preferences } from "./preferences.svelte";
import type { Size } from "../types";

export type EditorTool = "select" | "pan" | "slot" | "texture" | "text" | "button" | "toggle_button";

export function snap(value: number, snapSize: number): number {
  if (snapSize <= 1) return Math.round(value);
  return Math.round(value / snapSize) * snapSize;
}

class EditorStore {
  selectedElementId = $state<string | null>(null);
  selectedIds = $state<Set<string>>(new Set());
  zoom = $state(2);
  activeTool = $state<EditorTool>("select");
  toolRevision = $state(0);
  selectionRevision = $state(0);

  // Mouse position in GUI pixel coordinates (relative to GUI top-left)
  mouseGuiX = $state(0);
  mouseGuiY = $state(0);

  // Canvas pan offset in screen pixels
  panX = $state(0);
  panY = $state(0);

  // Canvas container dimensions
  canvasWidth = $state(800);
  canvasHeight = $state(600);

  get tool() {
    return this.activeTool;
  }

  set tool(value: EditorTool) {
    if (this.activeTool === value) return;
    this.activeTool = value;
    this.toolRevision += 1;
  }

  get showGrid() {
    return preferences.values.showGrid;
  }

  set showGrid(value: boolean) {
    preferences.update({ showGrid: value });
  }

  get snapToGrid() {
    return preferences.values.snapToGrid;
  }

  set snapToGrid(value: boolean) {
    preferences.update({ snapToGrid: value });
  }

  snapCoordinate(value: number): number {
    if (!preferences.values.snapToGrid) return Math.round(value);
    return snap(value, preferences.values.snapSize);
  }

  // Dragging state
  isDragging = $state(false);
  dragElementId = $state<string | null>(null);
  dragStartX = $state(0);
  dragStartY = $state(0);
  dragOrigX = $state(0);
  dragOrigY = $state(0);

  // Resizing state
  isResizing = $state(false);
  resizeElementId = $state<string | null>(null);
  resizeCorner = $state<"tl" | "tr" | "bl" | "br" | null>(null);
  resizeStartScreenX = $state(0);
  resizeStartScreenY = $state(0);
  resizeOrigX = $state(0);
  resizeOrigY = $state(0);
  resizeOrigW = $state(0);
  resizeOrigH = $state(0);

  selectElement(id: string | null, additive = false) {
    if (additive && id) {
      const next = new Set(this.selectedIds);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      this.selectedIds = next;
      this.selectedElementId = next.size > 0 ? [...next][0] : null;
    } else {
      this.selectedElementId = id;
      this.selectedIds = id ? new Set([id]) : new Set();
    }
    this.selectionRevision += 1;
  }

  clearSelection() {
    this.selectedElementId = null;
    this.selectedIds = new Set();
    this.selectionRevision += 1;
  }

  isSelected(id: string): boolean {
    return this.selectedIds.has(id);
  }

  zoomIn() {
    this.zoom = Math.min(8, this.zoom + 1);
  }

  zoomOut() {
    this.zoom = Math.max(1, this.zoom - 1);
  }

  resetView(guiSize: Size = { width: 176, height: 166 }) {
    this.zoom = 2;
    this.centerView(guiSize);
  }

  centerView(guiSize: Size) {
    this.panX = Math.round((this.canvasWidth - guiSize.width * this.zoom) / 2);
    this.panY = Math.round((this.canvasHeight - guiSize.height * this.zoom) / 2);
  }

  screenToGui(screenX: number, screenY: number, canvasEl: HTMLElement): { x: number; y: number } {
    const rect = canvasEl.getBoundingClientRect();
    const scaledX = (screenX - rect.left - this.panX) / this.zoom;
    const scaledY = (screenY - rect.top - this.panY) / this.zoom;
    return {
      x: Math.floor(scaledX),
      y: Math.floor(scaledY),
    };
  }

  startDragElementAt(id: string, canvasX: number, canvasY: number, elementX: number, elementY: number) {
    this.isDragging = true;
    this.dragElementId = id;
    this.dragStartX = canvasX;
    this.dragStartY = canvasY;
    this.dragOrigX = elementX;
    this.dragOrigY = elementY;
  }

  startResize(id: string, corner: "tl" | "tr" | "bl" | "br", screenX: number, screenY: number, origX: number, origY: number, origW: number, origH: number) {
    this.isResizing = true;
    this.resizeElementId = id;
    this.resizeCorner = corner;
    this.resizeStartScreenX = screenX;
    this.resizeStartScreenY = screenY;
    this.resizeOrigX = origX;
    this.resizeOrigY = origY;
    this.resizeOrigW = origW;
    this.resizeOrigH = origH;
  }
}

export const editor = new EditorStore();
