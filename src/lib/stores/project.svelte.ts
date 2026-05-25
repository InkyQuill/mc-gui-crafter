import type {
  ActiveProjectPayload,
  Animation,
  AssetMetadata,
  AttachedRegion,
  Element,
  ElementType,
  FontAsset,
  FontRenderData,
  Group,
  ModTarget,
  ProjectExportSettings,
  ProjectSessionSummary,
  SemanticGroup,
  Size,
  VisualBounds,
} from "../types";
import * as api from "../api";
import { SvelteMap } from "svelte/reactivity";

let nextId = 1;
function uid(): string {
  return `el_${nextId++}_${Date.now().toString(36)}`;
}

// Global map of asset name -> data URL for rendering
export const assetDataUrls = new SvelteMap<string, string>();
export const assetDimensions = new SvelteMap<string, Size>();

const DEFAULT_EXPORT_SETTINGS: ProjectExportSettings = {
  codegen_mode: "simple",
  generate_runtime_helpers: true,
  generate_semantic_registry: false,
};

export class ProjectStore {
  sessions = $state<ProjectSessionSummary[]>([]);
  activeProjectId = $state<string | null>(null);
  name = $state("Untitled GUI");
  guiSize = $state<Size>({ width: 176, height: 166 });
  modTarget = $state<ModTarget>("forge");
  elements = $state<Element[]>([]);
  groups = $state<Group[]>([]);
  animations = $state<Animation[]>([]);
  assets = $state<string[]>([]);
  assetMetadata = $state<Record<string, AssetMetadata>>({});
  fonts = $state<FontAsset[]>([]);
  semanticGroups = $state<SemanticGroup[]>([]);
  attachedRegions = $state<AttachedRegion[]>([]);
  exportSettings = $state<ProjectExportSettings>({ ...DEFAULT_EXPORT_SETTINGS });
  fontRenderData = new SvelteMap<string, FontRenderData>();
  fontRenderDataVersion = $state(0);
  revision = $state(0);
  renderVersion = $state(0);

  projectPath = $state<string | null>(null);
  isDirty = $state(false);
  isOpen = $state(false);

  get elementCount(): number {
    return this.elements.length;
  }

  get activeSession(): ProjectSessionSummary | undefined {
    return this.sessions.find(session => session.id === this.activeProjectId);
  }

  get summary() {
    return {
      project_id: this.activeProjectId ?? "",
      name: this.name,
      gui_size: this.guiSize,
      mod_target: this.modTarget,
      element_count: this.elementCount,
      is_dirty: this.isDirty,
      revision: this.revision,
      path: this.projectPath,
      session: this.activeSession,
    };
  }

  get visualBounds(): VisualBounds {
    let minX = 0;
    let minY = 0;
    let maxX = this.guiSize.width;
    let maxY = this.guiSize.height;

    for (const el of this.elements) {
      if (el.visible === false) continue;
      const size = this.elementVisualSize(el);
      minX = Math.min(minX, el.x);
      minY = Math.min(minY, el.y);
      maxX = Math.max(maxX, el.x + size.width);
      maxY = Math.max(maxY, el.y + size.height);
    }

    for (const region of this.attachedRegions) {
      if (region.visible === false) continue;
      minX = Math.min(minX, region.x);
      minY = Math.min(minY, region.y);
      maxX = Math.max(maxX, region.x + region.width);
      maxY = Math.max(maxY, region.y + region.height);
    }

    return {
      x: minX,
      y: minY,
      width: Math.max(1, maxX - minX),
      height: Math.max(1, maxY - minY),
    };
  }

  elementById(id: string): Element | undefined {
    return this.elements.find(e => e.id === id);
  }

  attachedRegionById(id: string): AttachedRegion | undefined {
    return this.attachedRegions.find(region => region.id === id);
  }

  elementsForAttachedRegion(id: string): Element[] {
    return this.elements.filter(element => element.attached_region === id);
  }

  groupById(id: string): Group | undefined {
    return this.groups.find(group => group.id === id);
  }

  groupForElement(id: string): Group | undefined {
    return this.groups.find(group => group.elements.includes(id));
  }

  movementIdsForElement(id: string): string[] {
    const group = this.groupForElement(id);
    const regionId = this.elementById(id)?.attached_region;
    const regionIds = regionId ? this.elementsForAttachedRegion(regionId).map(element => element.id) : [];
    const ids = group ? group.elements : regionIds.length > 0 ? regionIds : [id];
    return ids.filter((elementId, index) => ids.indexOf(elementId) === index && this.elementById(elementId));
  }

  movementIdsForElements(ids: Iterable<string>): string[] {
    const movementIds: string[] = [];
    for (const id of ids) {
      for (const movementId of this.movementIdsForElement(id)) {
        if (!movementIds.includes(movementId)) movementIds.push(movementId);
      }
    }
    return movementIds;
  }

  async newProject(name: string, width: number, height: number, target: ModTarget, template?: string) {
    const summary = await api.projectNew(name, width, height, target, template);
    this.activeProjectId = summary.project_id;
    await this.refreshSessions();
    await this.hydrateActiveProject();
    nextId = 1;
    this.startAutoSave();
  }

  async openProject(path: string) {
    const summary = await api.projectOpen(path);
    this.activeProjectId = summary.project_id;
    await this.refreshSessions();
    await this.hydrateActiveProject();
    nextId = this.elements.length + 1;
    ProjectStore.addRecentProject(path);
    this.startAutoSave();
  }

  async saveProject() {
    if (!this.activeProjectId) return;
    const result = await api.projectSave(this.activeProjectId);
    this.projectPath = result.path ?? this.projectPath;
    this.isDirty = result.is_dirty;
    await this.refreshSessions();
    if (this.projectPath) ProjectStore.addRecentProject(this.projectPath);
  }

  async saveProjectAs(path: string) {
    if (!this.activeProjectId) return;
    const result = await api.projectSaveAs(path, this.activeProjectId);
    this.projectPath = result.path ?? path;
    this.isDirty = result.is_dirty;
    await this.refreshSessions();
    ProjectStore.addRecentProject(this.projectPath);
    this.startAutoSave();
  }

  async switchProject(projectId: string) {
    if (projectId === this.activeProjectId) return;
    await api.projectSetActive(projectId);
    this.activeProjectId = projectId;
    await this.refreshSessions();
    await this.hydrateActiveProject();
  }

  async closeProject(projectId: string) {
    await api.projectClose(projectId);
    await this.refreshSessions();
    if (this.sessions.length === 0) {
      this.clearActiveProject();
      return;
    }
    await this.hydrateActiveProject();
  }

  async addElement(type: ElementType, x: number, y: number, overrides?: Partial<Element>): Promise<Element> {
    const element: Element = {
      id: uid(),
      type,
      x,
      y,
      ...overrides,
    };

    if (type === "slot" && !element.size) {
      element.size = 18;
    }

    if (type === "text") {
      element.content ??= "Text";
      element.font ??= "minecraft:default";
      element.color ??= 0x404040;
      element.shadow ??= false;
    }

    if (type === "progress") {
      element.width ??= 22;
      element.height ??= 15;
      element.asset ??= "textures/generated/progress_arrow.png";
      element.layer ??= "animatable";
      element.direction ??= "left_to_right";
    }

    if (type === "button") {
      element.width ??= 52;
      element.height ??= 20;
      element.asset ??= "textures/generated/button.png";
      element.layer ??= "background";
      element.content ??= "Button";
    }

    if (type === "toggle_button") {
      element.width ??= 20;
      element.height ??= 20;
      element.asset ??= "textures/generated/button.png";
      element.layer ??= "background";
      element.content ??= "Toggle";
    }

    const result = await api.elementAdd(element, this.activeProjectId ?? undefined);
    this.elements = [...this.elements, result];
    this.isDirty = true;
    await this.refreshSessions();
    this.bumpRenderVersion();

    return result;
  }

  async moveElement(id: string, x: number, y: number, recordUndo = true) {
    const old = this.elementById(id);
    if (!old) return;
    if (!recordUndo && old.x === x && old.y === y) return;

    this._moveLocal(id, x, y);
    this.isDirty = true;
    this.bumpRenderVersion();

    if (recordUndo) {
      await api.elementMove(id, x, y, this.activeProjectId ?? undefined);
      await this.refreshSessions();
      await this.hydrateActiveProject();
    }
  }

  async moveElementOrGroup(id: string, x: number, y: number, recordUndo = true) {
    const old = this.elementById(id);
    if (!old) return;
    await this.moveElementByDeltaForGroup(id, x - old.x, y - old.y, recordUndo);
  }

  async moveElementByDeltaForGroup(id: string, dx: number, dy: number, recordUndo = true) {
    const moves = this.movementIdsForElement(id)
      .map(elementId => {
        const el = this.elementById(elementId);
        return el ? { id: elementId, x: el.x + dx, y: el.y + dy } : null;
      })
      .filter(move => move !== null);

    await this.moveElements(moves, recordUndo);
  }

  async moveElements(moves: api.ElementMoveRequest[], recordUndo = true) {
    const uniqueMoves = new Map<string, api.ElementMoveRequest>();
    for (const move of moves) {
      if (this.elementById(move.id)) uniqueMoves.set(move.id, move);
    }

    const changed = [...uniqueMoves.values()].filter(move => {
      const el = this.elementById(move.id);
      return el && (el.x !== move.x || el.y !== move.y);
    });
    if (changed.length === 0) return;

    for (const move of changed) {
      this._moveLocal(move.id, move.x, move.y);
    }
    this.isDirty = true;
    this.bumpRenderVersion();

    if (recordUndo) {
      await this.commitElementMoves(changed);
    }
  }

  async commitMovedElements(ids: Iterable<string>) {
    const moves = [...ids]
      .map(id => {
        const el = this.elementById(id);
        return el ? { id, x: el.x, y: el.y } : null;
      })
      .filter(move => move !== null);

    await this.commitElementMoves(moves);
  }

  private async commitElementMoves(moves: api.ElementMoveRequest[]) {
    if (moves.length === 0) return;

    if (moves.length === 1) {
      const [move] = moves;
      await api.elementMove(move.id, move.x, move.y, this.activeProjectId ?? undefined);
    } else {
      await api.elementMoveMany(moves, this.activeProjectId ?? undefined);
    }

    await this.refreshSessions();
    await this.hydrateActiveProject();
  }

  private _moveLocal(id: string, x: number, y: number) {
    const el = this.elements.find(e => e.id === id);
    if (el) {
      el.x = x;
      el.y = y;
      this.refreshGroupPositionsForElements([id]);
    }
  }

  private refreshGroupPositionsForElements(ids: Iterable<string>) {
    const moved = new Set(ids);
    if (moved.size === 0) return;

    for (const group of this.groups) {
      if (!group.elements.some(id => moved.has(id))) continue;

      const elements = group.elements
        .map(id => this.elementById(id))
        .filter(element => element !== undefined);
      if (elements.length === 0) continue;

      group.x = Math.min(...elements.map(element => element.x));
      group.y = Math.min(...elements.map(element => element.y));
    }
  }

  async removeElement(id: string) {
    const el = this.elementById(id);
    if (!el) return;

    await api.elementRemove(id, this.activeProjectId ?? undefined);
    this.elements = this.elements.filter(e => e.id !== id);
    this.groups = this.groups
      .map(group => ({ ...group, elements: group.elements.filter(elementId => elementId !== id) }))
      .filter(group => group.elements.length >= 2);
    this.isDirty = true;
    await this.refreshSessions();
    this.bumpRenderVersion();
  }

  async updateElement(id: string, changes: Partial<Element>) {
    const el = this.elements.find(e => e.id === id);
    if (!el) return;

    const updated = await api.elementUpdate(id, changes, this.activeProjectId ?? undefined);
    Object.assign(el, updated);
    await this.refreshSessions();
    await this.hydrateActiveProject();
  }

  async updateExportSettings(changes: Partial<ProjectExportSettings>) {
    const next: ProjectExportSettings = {
      ...this.exportSettings,
      ...changes,
    };
    if (next.codegen_mode === "simple") {
      next.generate_semantic_registry = false;
    }
    if (next.codegen_mode === "modular") {
      next.generate_semantic_registry = true;
    }
    const updated = await api.projectExportSettingsUpdate(next, this.activeProjectId ?? undefined);
    this.exportSettings = updated;
    await this.refreshSessions();
    await this.hydrateActiveProject();
  }

  async updateSemanticGroups(groups: SemanticGroup[]) {
    const updated = await api.projectSemanticGroupsUpdate(groups, this.activeProjectId ?? undefined);
    this.semanticGroups = updated;
    await this.refreshSessions();
    await this.hydrateActiveProject();
  }

  async createAttachedRegion(region: AttachedRegion): Promise<AttachedRegion> {
    const created = await api.attachedRegionCreate(region, this.activeProjectId ?? undefined);
    this.attachedRegions = [...this.attachedRegions, created];
    this.isDirty = true;
    await this.refreshSessions();
    this.bumpRenderVersion();
    return created;
  }

  async updateAttachedRegion(id: string, changes: Partial<AttachedRegion>): Promise<AttachedRegion> {
    const before = this.attachedRegionById(id);
    const updated = await api.attachedRegionUpdate(id, changes, this.activeProjectId ?? undefined);
    this.attachedRegions = this.attachedRegions.map(region => region.id === id ? updated : region);
    if (!before || JSON.stringify(before) !== JSON.stringify(updated)) {
      this.isDirty = true;
    }
    await this.refreshSessions();
    this.bumpRenderVersion();
    return updated;
  }

  async removeAttachedRegion(id: string): Promise<void> {
    const removed = await api.attachedRegionRemove(id, this.activeProjectId ?? undefined);
    if (!removed) return;

    this.attachedRegions = this.attachedRegions.filter(region => region.id !== id);
    this.elements = this.elements.map(element =>
      element.attached_region === id ? { ...element, attached_region: null } : element,
    );
    this.isDirty = true;
    await this.refreshSessions();
    this.bumpRenderVersion();
  }

  async moveAttachedRegionWithElements(id: string, x: number, y: number): Promise<void> {
    const old = this.attachedRegionById(id);
    if (!old) return;

    const updated = await api.attachedRegionMoveWithElements(id, x, y, this.activeProjectId ?? undefined);
    const dx = updated.x - old.x;
    const dy = updated.y - old.y;
    this.attachedRegions = this.attachedRegions.map(region => region.id === id ? updated : region);
    const movedIds = this.moveAttachedRegionElementsLocal(id, dx, dy);
    if (dx !== 0 || dy !== 0 || JSON.stringify(old) !== JSON.stringify(updated)) {
      this.isDirty = true;
    }
    this.refreshGroupPositionsForElements(movedIds);
    await this.refreshSessions();
    this.bumpRenderVersion();
  }

  previewMoveAttachedRegionWithElements(id: string, x: number, y: number): void {
    const old = this.attachedRegionById(id);
    if (!old) return;

    const dx = x - old.x;
    const dy = y - old.y;
    this.attachedRegions = this.attachedRegions.map(region => region.id === id ? { ...region, x, y } : region);
    const movedIds = this.moveAttachedRegionElementsLocal(id, dx, dy);
    this.refreshGroupPositionsForElements(movedIds);
    this.bumpRenderVersion();
  }

  async createGroup(elementIds: Iterable<string>) {
    const elementIdList = [...elementIds];
    const ids = elementIdList.filter((id, index) => elementIdList.indexOf(id) === index && this.elementById(id));
    if (ids.length < 2) return null;

    const group = await api.groupCreate(ids, undefined, this.activeProjectId ?? undefined);
    await this.refreshSessions();
    await this.hydrateActiveProject();
    return group;
  }

  async ungroup(groupId: string) {
    const removed = await api.groupUngroup(groupId, this.activeProjectId ?? undefined);
    if (removed) {
      await this.refreshSessions();
      await this.hydrateActiveProject();
    }
    return removed;
  }

  async ungroupElements(elementIds: Iterable<string>) {
    const groupIds: string[] = [];
    for (const id of elementIds) {
      const group = this.groupForElement(id);
      if (group && !groupIds.includes(group.id)) groupIds.push(group.id);
    }
    for (const groupId of groupIds) {
      await this.ungroup(groupId);
    }
  }

  async undo() {
    if (!this.activeProjectId || !this.canUndo) return;
    await api.projectUndo(this.activeProjectId);
    await this.refreshSessions();
    await this.hydrateActiveProject();
  }

  async redo() {
    if (!this.activeProjectId || !this.canRedo) return;
    await api.projectRedo(this.activeProjectId);
    await this.refreshSessions();
    await this.hydrateActiveProject();
  }

  get canUndo(): boolean { return this.activeSession?.can_undo ?? false; }
  get canRedo(): boolean { return this.activeSession?.can_redo ?? false; }

  async moveElementUp(id: string) {
    const idx = this.elements.findIndex(e => e.id === id);
    if (idx >= 0 && idx < this.elements.length - 1) {
      await api.elementReorder(id, idx + 1, this.activeProjectId ?? undefined);
      await this.refreshSessions();
      await this.hydrateActiveProject();
    }
  }

  async moveElementDown(id: string) {
    const idx = this.elements.findIndex(e => e.id === id);
    if (idx > 0) {
      await api.elementReorder(id, idx - 1, this.activeProjectId ?? undefined);
      await this.refreshSessions();
      await this.hydrateActiveProject();
    }
  }

  getElementBounds(id: string): { x: number; y: number; w: number; h: number } | null {
    const el = this.elementById(id);
    if (!el) return null;
    const w = el.width ?? el.size ?? 18;
    const h = el.height ?? el.size ?? 18;
    return { x: el.x, y: el.y, w, h };
  }

  async resizeElement(id: string, x: number, y: number, w: number, h: number, recordUndo = true) {
    const el = this.elementById(id);
    if (!el) return;

    this._resizeLocal(id, x, y, w, h, undefined);
    this.isDirty = true;
    this.bumpRenderVersion();
    if (recordUndo) {
      await api.elementResize(id, x, y, w, h, this.activeProjectId ?? undefined);
      await this.refreshSessions();
      await this.hydrateActiveProject();
    }
  }

  _resizeLocal(id: string, x: number, y: number, w?: number, h?: number, size?: number) {
    const el = this.elements.find(e => e.id === id);
    if (!el) return;
    el.x = x;
    el.y = y;
    if (el.type === "slot") {
      const nextSize = size ?? (w !== undefined || h !== undefined ? Math.max(w ?? 0, h ?? 0) : undefined);
      if (nextSize !== undefined) el.size = Math.max(8, nextSize);
    } else {
      if (w !== undefined) el.width = Math.max(4, w);
      if (h !== undefined) el.height = Math.max(4, h);
    }
  }

  // -- Animation management --
  async addAnimation(name: string, type: Animation["type"], dataKey: string) {
    const anim: Animation = {
      id: name.trim() || `anim_${Date.now().toString(36)}`,
      type,
      data_key: dataKey,
    };
    const created = await api.animationCreate(anim, this.activeProjectId ?? undefined);
    await this.refreshSessions();
    await this.hydrateActiveProject();
    return created;
  }

  async removeAnimation(id: string) {
    await api.animationRemove(id, this.activeProjectId ?? undefined);
    await this.refreshSessions();
    await this.hydrateActiveProject();
  }

  async updateAnimation(id: string, changes: Partial<Animation>) {
    const anim = this.animations.find(a => a.id === id);
    if (anim) {
      Object.assign(anim, await api.animationUpdate(id, changes, this.activeProjectId ?? undefined));
      await this.refreshSessions();
      await this.hydrateActiveProject();
    }
  }

  async bindAnimationToElement(elementId: string, animationId: string | undefined) {
    const el = this.elements.find(e => e.id === elementId);
    if (el) {
      const updated = animationId
        ? await api.animationBind(elementId, animationId, this.activeProjectId ?? undefined)
        : await api.animationUnbind(elementId, this.activeProjectId ?? undefined);
      Object.assign(el, updated);
      await this.refreshSessions();
      await this.hydrateActiveProject();
    }
  }

  // -- Auto-save --
  private autoSaveTimer: ReturnType<typeof setInterval> | null = null;
  private fontRenderDataRequest = 0;

  async syncFromBackend() {
    await this.refreshSessions();
    if (this.activeProjectId) {
      await this.hydrateActiveProject();
    }
  }

  async refreshSessions() {
    try {
      this.sessions = await api.projectListSessions();
      this.activeProjectId = this.sessions.find(session => session.active)?.id ?? null;
    } catch { /* ignore errors during sync */ }
  }

  async importFont(filePath: string) {
    const projectId = this.activeProjectId;
    const requestId = this.nextFontRenderDataRequest();
    const font = await api.fontImport(filePath, projectId ?? undefined);
    if (!this.isCurrentFontRenderDataRequest(projectId, requestId)) return font;

    const existing = this.fonts.findIndex(f => f.id === font.id);
    if (existing >= 0) this.fonts = this.fonts.map(existingFont => existingFont.id === font.id ? font : existingFont);
    else this.fonts = [...this.fonts, font];
    await this.loadFontRenderData(font.id, projectId, requestId);
    if (!this.isCurrentFontRenderDataRequest(projectId, requestId)) return font;

    this.isDirty = true;
    await this.refreshSessions();
    this.bumpRenderVersion();
    return font;
  }

  async refreshFonts() {
    const projectId = this.activeProjectId;
    const requestId = this.nextFontRenderDataRequest();
    try {
      const fonts = await api.fontList(projectId ?? undefined);
      if (!this.isCurrentFontRenderDataRequest(projectId, requestId)) return;

      this.fonts = fonts;
      await this.syncFontRenderData(fonts.map(font => font.id), projectId, requestId);
    } catch { /* fonts may not be available */ }
  }

  async hydrateActiveProject() {
    try {
      const payload = await api.projectGetActive();
      this.applyActivePayload(payload);
      await this.loadActiveAssets();
      await this.refreshFonts();
    } catch {
      this.clearActiveProject();
    }
  }

  private applyActivePayload(payload: ActiveProjectPayload) {
    const project = payload.project;
    this.activeProjectId = payload.summary.id;
    this.name = project.name;
    this.guiSize = project.gui_size;
    this.modTarget = project.mod_target;
    this.elements = project.elements;
    this.groups = project.groups;
    this.animations = project.animations;
    this.assets = project.assets;
    this.assetMetadata = project.asset_metadata ?? {};
    this.fonts = project.fonts ?? [];
    this.semanticGroups = project.semantic_groups ?? [];
    this.attachedRegions = project.attached_regions ?? [];
    this.exportSettings = project.export_settings ?? { ...DEFAULT_EXPORT_SETTINGS };
    this.invalidateFontRenderData();
    this.projectPath = payload.summary.path ?? project.project_path ?? null;
    this.isDirty = payload.summary.is_dirty;
    this.revision = payload.summary.revision;
    this.isOpen = true;
    this.bumpRenderVersion();
  }

  private async loadActiveAssets() {
    try {
      const assets = await api.assetList(this.activeProjectId ?? undefined);
      this.assets = assets.map(a => a.name);
      assetDataUrls.clear();
      assetDimensions.clear();
      const nextMetadata = { ...this.assetMetadata };
      for (const a of assets) {
        if (a.data_url) {
          assetDataUrls.set(a.name, a.data_url);
        }
        if (a.width > 0 && a.height > 0) {
          assetDimensions.set(a.name, { width: a.width, height: a.height });
        }
        nextMetadata[a.name] = {
          ...nextMetadata[a.name],
          width: a.width,
          height: a.height,
          nine_slice: a.nine_slice ?? nextMetadata[a.name]?.nine_slice ?? null,
        };
      }
      this.assetMetadata = nextMetadata;
      this.bumpRenderVersion();
    } catch { /* assets may not be available */ }
  }

  async ensureAssetDataUrl(name: string): Promise<string | undefined> {
    const cached = assetDataUrls.get(name);
    if (cached) return cached;
    if (!this.activeProjectId) return undefined;
    const dataUrl = await api.assetGetDataUrl(name, this.activeProjectId);
    assetDataUrls.set(name, dataUrl);
    this.bumpRenderVersion();
    return dataUrl;
  }

  async updateAssetMetadata(name: string, metadata: AssetMetadata): Promise<AssetMetadata> {
    const updated = await api.assetMetadataUpdate(name, metadata, this.activeProjectId ?? undefined);
    this.assetMetadata = { ...this.assetMetadata, [name]: updated };
    this.isDirty = true;
    await this.refreshSessions();
    this.bumpRenderVersion();
    return updated;
  }

  private async syncFontRenderData(fontIds: string[], projectId: string | null, requestId: number) {
    const next: [string, FontRenderData][] = [];
    await Promise.all(fontIds.map(async fontId => {
      try {
        const renderData = await api.fontRenderData(fontId, projectId ?? undefined);
        if (this.isCurrentFontRenderDataRequest(projectId, requestId)) {
          next.push([fontId, renderData]);
        }
      } catch {
        // Keep font list usable even if a single render payload is unavailable.
      }
    }));
    if (!this.isCurrentFontRenderDataRequest(projectId, requestId)) return;

    this.fontRenderData.clear();
    for (const [fontId, renderData] of next) {
      this.fontRenderData.set(fontId, renderData);
    }
    this.bumpFontRenderDataVersion();
    this.bumpRenderVersion();
  }

  private async loadFontRenderData(fontId: string, projectId: string | null, requestId: number) {
    try {
      const renderData = await api.fontRenderData(fontId, projectId ?? undefined);
      if (!this.isCurrentFontRenderDataRequest(projectId, requestId)) return;

      this.fontRenderData.set(fontId, renderData);
      this.bumpFontRenderDataVersion();
    } catch { /* font render data may not be available */ }
  }

  private nextFontRenderDataRequest(): number {
    this.fontRenderDataRequest += 1;
    return this.fontRenderDataRequest;
  }

  private isCurrentFontRenderDataRequest(projectId: string | null, requestId: number): boolean {
    return this.fontRenderDataRequest === requestId && this.activeProjectId === projectId && this.isOpen;
  }

  private invalidateFontRenderData() {
    this.nextFontRenderDataRequest();
    this.fontRenderData.clear();
    this.bumpFontRenderDataVersion();
  }

  private bumpFontRenderDataVersion() {
    this.fontRenderDataVersion += 1;
  }

  private clearActiveProject() {
    this.activeProjectId = null;
    this.name = "Untitled GUI";
    this.guiSize = { width: 176, height: 166 };
    this.modTarget = "forge";
    this.elements = [];
    this.groups = [];
    this.animations = [];
    this.assets = [];
    this.assetMetadata = {};
    this.fonts = [];
    this.semanticGroups = [];
    this.attachedRegions = [];
    this.exportSettings = { ...DEFAULT_EXPORT_SETTINGS };
    this.invalidateFontRenderData();
    this.projectPath = null;
    this.isDirty = false;
    this.revision = 0;
    this.isOpen = false;
    assetDataUrls.clear();
    assetDimensions.clear();
    this.bumpRenderVersion();
  }
  private bumpRenderVersion() {
    this.renderVersion += 1;
  }

  private elementVisualSize(element: Element): { width: number; height: number } {
    const defaultSize = (() => {
      switch (element.type) {
        case "slot":
        case "virtual_slot_cell":
          return { width: 18, height: 18 };
        case "button":
        case "toggle_button":
          return { width: 20, height: 20 };
        case "scrollbar":
          return { width: 12, height: 54 };
        default:
          return { width: 16, height: 16 };
      }
    })();

    const assetSize = element.type === "texture" && element.asset
      ? assetDimensions.get(element.asset)
      : undefined;

    return {
      width: element.width ?? element.size ?? assetSize?.width ?? defaultSize.width,
      height: element.height ?? element.size ?? assetSize?.height ?? defaultSize.height,
    };
  }

  private moveAttachedRegionElementsLocal(id: string, dx: number, dy: number): string[] {
    if (dx === 0 && dy === 0) return [];

    const movedIds: string[] = [];
    this.elements = this.elements.map(element => {
      if (element.attached_region !== id) return element;
      movedIds.push(element.id);
      return { ...element, x: element.x + dx, y: element.y + dy };
    });
    return movedIds;
  }

  startAutoSave(intervalMs = 60000) {
    this.stopAutoSave();
    this.autoSaveTimer = setInterval(() => {
      if (this.isDirty && this.projectPath) {
        this.saveProject();
      }
    }, intervalMs);
  }
  stopAutoSave() {
    if (this.autoSaveTimer) { clearInterval(this.autoSaveTimer); this.autoSaveTimer = null; }
  }

  // -- Recent projects --
  static getRecentProjects(): string[] {
    try {
      const parsed = JSON.parse(localStorage.getItem("mcgui_recent") || "[]");
      return Array.isArray(parsed) ? parsed.filter(path => typeof path === "string") : [];
    } catch { return []; }
  }

  static addRecentProject(path: string) {
    const recent = ProjectStore.getRecentProjects().filter(p => p !== path);
    recent.unshift(path);
    localStorage.setItem("mcgui_recent", JSON.stringify(recent.slice(0, 10)));
  }

  static removeRecentProject(path: string) {
    const recent = ProjectStore.getRecentProjects().filter(p => p !== path);
    localStorage.setItem("mcgui_recent", JSON.stringify(recent));
  }

  removeRecentProject(path: string) {
    ProjectStore.removeRecentProject(path);
  }
}

export const project = new ProjectStore();
