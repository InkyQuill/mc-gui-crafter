import type {
  ActiveProjectPayload,
  Animation,
  AppConfig,
  AssetMetadata,
  AttachedRegion,
  AttachedRegionAnchor,
  AttachedRegionState,
  Element,
  EditorLayoutConfig,
  FontAsset,
  FontRenderData,
  GlyphInfo,
  Group,
  MinecraftSource,
  ModTarget,
  NineSlice,
  CodegenMode,
  ProjectData,
  ProjectExportSettings,
  ProjectSessionSummary,
  ProjectSummary,
  SaveProjectResult,
  SemanticGroup,
  WindowConfig,
} from "./types";

let tauriInvoke: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;

function hasTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function getInvoke() {
  if (tauriInvoke) return tauriInvoke;
  if (!hasTauriRuntime()) {
    tauriInvoke = mockInvoke;
    return tauriInvoke;
  }

  try {
    const tauri = await import("@tauri-apps/api/core");
    tauriInvoke = tauri.invoke;
    return tauriInvoke;
  } catch {
    tauriInvoke = mockInvoke;
    return tauriInvoke;
  }
}

interface MockSession {
  id: string;
  project: ProjectData;
  revision: number;
  can_undo: boolean;
  can_redo: boolean;
  undoStack: ProjectData[];
  redoStack: ProjectData[];
}

export interface ElementMoveRequest {
  id: string;
  x: number;
  y: number;
}

const mockSessions: MockSession[] = [];
const mockAssetDataUrls = new Map<string, Map<string, string>>();
const mockAssetMetadata = new Map<string, Map<string, AssetMetadata>>();
const mockExistingExportFiles = new Set<string>();
const EMPTY_SHA256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
let mockActiveProjectId: string | null = null;
let mockNextProjectId = 1;
let mockAppConfig: AppConfig = {
  mcp_port: 47381,
  editor_layout: {
    version: 1,
    right_dock_width: 520,
    properties_width: 300,
    browser_tab: "layers",
  },
  window: {
    width: 1280,
    height: 800,
    x: null,
    y: null,
  },
};

export function setMockExistingExportFiles(paths: string[]): void {
  mockExistingExportFiles.clear();
  for (const path of paths) mockExistingExportFiles.add(path);
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function mockSession(projectId?: unknown): MockSession {
  const id = typeof projectId === "string" ? projectId : mockActiveProjectId;
  const session = mockSessions.find(s => s.id === id);
  if (!session) throw "No project open";
  return session;
}

function mockSummary(session: MockSession): ProjectSessionSummary {
  return {
    id: session.id,
    name: session.project.name,
    path: session.project.project_path,
    active: session.id === mockActiveProjectId,
    is_dirty: session.project.is_dirty ?? false,
    revision: session.revision,
    element_count: session.project.elements.length,
    can_undo: session.can_undo,
    can_redo: session.can_redo,
  };
}

function mockProjectResult(session: MockSession): ProjectSummary {
  return {
    project_id: session.id,
    name: session.project.name,
    gui_size: session.project.gui_size,
    mod_target: session.project.mod_target,
    path: session.project.project_path,
    element_count: session.project.elements.length,
    is_dirty: session.project.is_dirty ?? false,
    revision: session.revision,
    session: mockSummary(session),
  };
}

function createMockSession(project: ProjectData): ProjectSummary {
  const id = `mock_project_${mockNextProjectId++}`;
  const session: MockSession = {
    id,
    project: clone(project),
    revision: 0,
    can_undo: false,
    can_redo: false,
    undoStack: [],
    redoStack: [],
  };
  mockSessions.push(session);
  mockAssetDataUrls.set(id, new Map());
  mockAssetMetadata.set(id, new Map(Object.entries(project.asset_metadata ?? {})));
  mockActiveProjectId = id;
  return mockProjectResult(session);
}

function clampMockLayout(layout: EditorLayoutConfig): EditorLayoutConfig {
  const right = layout.right_dock_width >= 360 && layout.right_dock_width <= 900
    ? Math.round(layout.right_dock_width)
    : 520;
  const maxProperties = Math.max(240, right - 160);
  const properties = layout.properties_width >= 240 && layout.properties_width <= maxProperties
    ? Math.round(layout.properties_width)
    : Math.min(300, maxProperties);
  return {
    version: 1,
    right_dock_width: right,
    properties_width: properties,
    browser_tab: layout.browser_tab === "assets" ? "assets" : "layers",
  };
}

function clampMockWindow(window: WindowConfig): WindowConfig {
  if (window.width < 900 || window.height < 600) {
    return { width: 1280, height: 800, x: null, y: null };
  }
  const hasValidPosition =
    typeof window.x === "number" &&
    typeof window.y === "number" &&
    Math.abs(window.x) < 20000 &&
    Math.abs(window.y) < 20000;
  return {
    width: Math.round(window.width),
    height: Math.round(window.height),
    x: hasValidPosition ? Math.round(window.x!) : null,
    y: hasValidPosition ? Math.round(window.y!) : null,
  };
}

function updateMockHistoryFlags(session: MockSession) {
  session.can_undo = session.undoStack.length > 0;
  session.can_redo = session.redoStack.length > 0;
}

function markMockChanged(session: MockSession, previous: ProjectData) {
  session.undoStack.push(previous);
  session.redoStack = [];
  session.project.is_dirty = true;
  session.revision += 1;
  updateMockHistoryFlags(session);
}

function refreshMockGroupPositions(session: MockSession, movedIds: Iterable<string>) {
  const moved = new Set(movedIds);
  if (moved.size === 0) return;

  for (const group of session.project.groups) {
    if (!group.elements.some(id => moved.has(id))) continue;

    const elements = group.elements
      .map(id => session.project.elements.find(element => element.id === id))
      .filter(element => element !== undefined);
    if (elements.length === 0) continue;

    group.x = Math.min(...elements.map(element => element.x));
    group.y = Math.min(...elements.map(element => element.y));
  }
}

function mockAssetsForSession(session: MockSession): Map<string, string> {
  let assets = mockAssetDataUrls.get(session.id);
  if (!assets) {
    assets = new Map();
    mockAssetDataUrls.set(session.id, assets);
  }
  return assets;
}

function mockAssetMetadataForSession(session: MockSession): Map<string, AssetMetadata> {
  let metadata = mockAssetMetadata.get(session.id);
  if (!metadata) {
    metadata = new Map(Object.entries(session.project.asset_metadata ?? {}));
    mockAssetMetadata.set(session.id, metadata);
  }
  return metadata;
}

function syncMockAssetMetadataFromProject(session: MockSession): void {
  mockAssetMetadata.set(session.id, new Map(Object.entries(session.project.asset_metadata ?? {})));
}

function dataUrlPayloadBytes(dataUrl: string): number {
  const payload = dataUrl.startsWith("data:image/png;base64,") ? dataUrl.slice("data:image/png;base64,".length) : "";
  if (!payload) return 0;
  const padding = payload.endsWith("==") ? 2 : payload.endsWith("=") ? 1 : 0;
  return Math.max(0, Math.floor(payload.length * 3 / 4) - padding);
}

async function mockSha256(dataUrl: string): Promise<string> {
  const payload = dataUrl.startsWith("data:image/png;base64,") ? dataUrl.slice("data:image/png;base64,".length) : "";
  if (!payload) return EMPTY_SHA256;
  if (typeof crypto === "undefined" || !crypto.subtle || typeof Uint8Array === "undefined" || typeof atob === "undefined") {
    return "0".repeat(64);
  }
  const binary = atob(payload);
  const data = Uint8Array.from(binary, char => char.charCodeAt(0));
  const hash = await crypto.subtle.digest("SHA-256", data);
  return [...new Uint8Array(hash)].map(byte => byte.toString(16).padStart(2, "0")).join("");
}

async function mockAssetResult(
  name: string,
  dataUrl: string,
  metadata?: AssetMetadata,
  includeDataUrl = false,
): Promise<AssetImportResult> {
  const decoded = dataUrl
    ? await dataUrlDimensions(dataUrl)
    : { width: metadata?.width ?? 16, height: metadata?.height ?? 16 };
  return {
    name,
    width: decoded.width,
    height: decoded.height,
    bytes: dataUrlPayloadBytes(dataUrl),
    sha256: await mockSha256(dataUrl),
    ...(includeDataUrl ? { data_url: dataUrl } : {}),
    nine_slice: metadata?.nine_slice ?? null,
  };
}

async function dataUrlDimensions(dataUrl: string): Promise<{ width: number; height: number }> {
  if (!dataUrl) return { width: 16, height: 16 };
  if (typeof Image === "undefined") return { width: 16, height: 16 };
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve({ width: image.naturalWidth || 16, height: image.naturalHeight || 16 });
    image.onerror = () => reject("Failed to decode PNG");
    image.src = dataUrl;
  });
}

const attachedRegionAnchors = new Set<AttachedRegionAnchor>(["left", "right", "top", "bottom", "free"]);
const attachedRegionStates = new Set<AttachedRegionState>(["static", "toggleable"]);
const requiredAttachedRegionFields = ["anchor", "x", "y", "width", "height", "state"] as const;
const maxU32 = 0xffffffff;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function assertIntegerField(value: unknown, field: string): asserts value is number {
  if (typeof value !== "number" || !Number.isInteger(value)) {
    throw `Invalid attached region update: ${field} must be an integer`;
  }
}

function assertPositiveU32Field(value: unknown, field: string): asserts value is number {
  assertIntegerField(value, field);
  if (value <= 0 || value > maxU32) {
    throw `Invalid attached region update: ${field} is out of range`;
  }
}

function applyMockAttachedRegionChanges(current: AttachedRegion, changes: unknown): AttachedRegion {
  if (!isRecord(changes)) throw "Attached region changes must be an object";
  if ("id" in changes && changes.id !== current.id) throw "Attached region id cannot be changed";

  const next = { ...current };
  for (const [key, value] of Object.entries(changes)) {
    if (key === "id") continue;
    (next as Record<string, unknown>)[key] = value;
  }

  for (const field of requiredAttachedRegionFields) {
    if (next[field] === null || next[field] === undefined) {
      throw `Invalid attached region update: ${field} is required`;
    }
  }
  if (!attachedRegionAnchors.has(next.anchor)) {
    throw `Invalid attached region update: invalid anchor ${String(next.anchor)}`;
  }
  if (!attachedRegionStates.has(next.state)) {
    throw `Invalid attached region update: invalid state ${String(next.state)}`;
  }
  assertIntegerField(next.x, "x");
  assertIntegerField(next.y, "y");
  assertPositiveU32Field(next.width, "width");
  assertPositiveU32Field(next.height, "height");
  if (next.visible !== undefined && typeof next.visible !== "boolean") {
    throw "Invalid attached region update: visible must be a boolean";
  }
  if (next.kind !== null && next.kind !== undefined && typeof next.kind !== "string") {
    throw "Invalid attached region update: kind must be a string or null";
  }
  if (next.semantic_group !== null && next.semantic_group !== undefined && typeof next.semantic_group !== "string") {
    throw "Invalid attached region update: semantic_group must be a string or null";
  }

  return next;
}

const javaKeywords = new Set([
  "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char",
  "class", "const", "continue", "default", "do", "double", "else", "enum",
  "extends", "final", "finally", "float", "for", "goto", "if", "implements",
  "import", "instanceof", "int", "interface", "long", "native", "new",
  "package", "private", "protected", "public", "return", "short", "static",
  "strictfp", "super", "switch", "synchronized", "this", "throw", "throws",
  "transient", "try", "void", "volatile", "while",
]);

function trimInvalidResourceEdges(value: string, fallback: string): string {
  const trimmed = value.replace(/^[_-]+|[_-]+$/g, "");
  return trimmed || fallback;
}

function sanitizeMockResource(value: string, fallback: string): string {
  let out = "";
  for (const char of value.trim()) {
    const lower = char.toLowerCase();
    if (/^[a-z0-9_-]$/.test(lower)) out += lower;
    else if (/^\s$/.test(lower) || lower === ".") out += "_";
  }
  return trimInvalidResourceEdges(out, fallback);
}

function sanitizeMockClassName(value: string): string {
  let out = "";
  let capitalizeNext = true;
  for (const char of value.trim()) {
    if (/^[a-zA-Z0-9]$/.test(char)) {
      out += capitalizeNext ? char.toUpperCase() : char;
      capitalizeNext = false;
    } else {
      capitalizeNext = true;
    }
  }
  if (!out) out = "GeneratedGui";
  if (/^[0-9]/.test(out)) out = `G${out}`;
  if (javaKeywords.has(out)) out = `${out}Gui`;
  return out;
}

function sanitizeMockPackage(value: string, modId: string): string {
  const segments = value
    .split(".")
    .map(segment => {
      let out = "";
      for (const char of segment.trim()) {
        const lower = char.toLowerCase();
        if (/^[a-z0-9_]$/.test(lower)) out += lower;
      }
      if (!out) return null;
      if (/^[0-9]/.test(out)) out = `_${out}`;
      if (javaKeywords.has(out)) out = `${out}_`;
      return out;
    })
    .filter((segment): segment is string => Boolean(segment));
  return segments.length > 0 ? segments.join(".") : `com.example.${modId.replace(/-/g, "_")}`;
}

function joinMockPath(...parts: string[]): string {
  const [first, ...rest] = parts;
  return [first.replace(/\/+$/g, ""), ...rest.map(part => part.replace(/^\/+|\/+$/g, ""))]
    .filter(Boolean)
    .join("/");
}

function mockExportPreview(args?: Record<string, unknown>): ExportPreview {
  const target = String(args?.target ?? "forge").trim().toLowerCase();
  if (!["forge", "fabric", "neoforge"].includes(target)) throw `Unsupported export target: ${target}`;
  const outputDir = String(args?.output_dir ?? "").trim();
  if (!outputDir) throw "Export output directory cannot be empty";

  const session = mockSession(args?.project_id);
  const modId = sanitizeMockResource(String(args?.mod_id ?? ""), "mcgui_export");
  const className = sanitizeMockClassName(String(args?.class_name ?? ""));
  const packageName = sanitizeMockPackage(String(args?.package ?? ""), modId);
  const packagePath = packageName.replace(/\./g, "/");
  const resourceName = sanitizeMockResource(String(args?.class_name ?? ""), "gui");
  const settings = mockExportSettings(session.project, args);
  const assetBase = joinMockPath(outputDir, "src/main/resources/assets", modId);
  const javaBase = joinMockPath(outputDir, "src/main/java", packagePath);
  const metadata = target === "fabric"
    ? "src/main/resources/fabric.mod.json"
    : target === "neoforge"
      ? "src/main/resources/META-INF/neoforge.mods.toml"
      : "src/main/resources/META-INF/mods.toml";
  const referencedAssets = new Set<string>();
  for (const element of session.project.elements) {
    if (element.type === "texture" && element.asset) referencedAssets.add(element.asset);
  }
  for (const animation of session.project.animations) {
    if (animation.texture) referencedAssets.add(animation.texture);
  }
  const assets = mockAssetsForSession(session);
  const errors = [...referencedAssets]
    .filter(asset => !assets.has(asset))
    .map(asset => `Texture asset referenced by project is missing: ${asset}`);

  const files = [
    joinMockPath(outputDir, "settings.gradle"),
    joinMockPath(outputDir, "build.gradle"),
    joinMockPath(outputDir, "gradle.properties"),
    joinMockPath(assetBase, `textures/gui/${resourceName}_gui.png`),
    ...[...referencedAssets].filter(asset => assets.has(asset)).map(asset => joinMockPath(assetBase, asset)),
    joinMockPath(assetBase, `gui/${resourceName}_layout.json`),
    joinMockPath(javaBase, "GuiLayout.java"),
    ...(settings.generate_runtime_helpers ? [joinMockPath(javaBase, "GuiRuntime.java")] : []),
    ...(settings.codegen_mode === "modular" && settings.generate_semantic_registry
      ? [joinMockPath(javaBase, "SemanticRegistry.java")]
      : []),
    joinMockPath(javaBase, `${className}Screen.java`),
    joinMockPath(javaBase, `${className}Client.java`),
    joinMockPath(outputDir, metadata),
    joinMockPath(outputDir, "README.txt"),
  ];

  return {
    target: target as ModTarget,
    mod_id: modId,
    package: packageName,
    class_name: className,
    output_dir: outputDir,
    files,
    warnings: args?.overwrite === true
      ? []
      : files
        .filter(path => mockExistingExportFiles.has(path))
        .map(path => `Target file already exists and will be overwritten: ${path}`),
    errors,
  };
}

function mockExportSettings(project: ProjectData, args?: Record<string, unknown>): ProjectExportSettings {
  const settings: ProjectExportSettings = {
    codegen_mode: project.export_settings?.codegen_mode ?? "simple",
    generate_runtime_helpers: project.export_settings?.generate_runtime_helpers ?? true,
    generate_semantic_registry: project.export_settings?.generate_semantic_registry ?? false,
  };

  if (args?.codegen_mode === "simple" || args?.codegen_mode === "modular") {
    settings.codegen_mode = args.codegen_mode;
  }
  if (typeof args?.generate_runtime_helpers === "boolean") {
    settings.generate_runtime_helpers = args.generate_runtime_helpers;
  }
  if (typeof args?.generate_semantic_registry === "boolean") {
    settings.generate_semantic_registry = args.generate_semantic_registry;
  }
  if (settings.codegen_mode === "simple") settings.generate_semantic_registry = false;
  if (settings.codegen_mode === "modular") settings.generate_semantic_registry = true;
  return settings;
}

async function mockInvoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "app_config_get":
      return clone(mockAppConfig);
    case "editor_layout_save": {
      mockAppConfig = {
        ...mockAppConfig,
        editor_layout: clampMockLayout(args?.layout as EditorLayoutConfig),
      };
      return clone(mockAppConfig);
    }
    case "app_window_save": {
      mockAppConfig = {
        ...mockAppConfig,
        window: clampMockWindow(args?.window as WindowConfig),
      };
      return clone(mockAppConfig);
    }
    case "ui_layout_reset":
      mockAppConfig = {
        ...mockAppConfig,
        editor_layout: {
          version: 1,
          right_dock_width: 520,
          properties_width: 300,
          browser_tab: "layers",
        },
        window: {
          width: 1280,
          height: 800,
          x: null,
          y: null,
        },
      };
      return clone(mockAppConfig);
    case "project_new":
      return createMockSession({
        name: (args?.name as string) || "Untitled",
        gui_size: { width: (args?.width as number) || 176, height: (args?.height as number) || 166 },
        mod_target: (args?.mod_target as ModTarget) || "forge",
        elements: [{
          id: "background",
          type: "texture",
          x: 0,
          y: 0,
          width: (args?.width as number) || 176,
          height: (args?.height as number) || 166,
          asset: "textures/generated/gui_panel.png",
          visible: true,
          layer: "background",
        }],
        groups: [],
        animations: [],
        assets: ["textures/generated/gui_panel.png"],
        is_dirty: true,
      });
    case "project_open":
      return createMockSession({
        name: "Opened Project",
        gui_size: { width: 176, height: 166 },
        mod_target: "forge",
        elements: [],
        groups: [],
        animations: [],
        assets: [],
        project_path: args?.path as string,
        is_dirty: false,
      });
    case "project_save": {
      const session = mockSession(args?.project_id);
      session.project.is_dirty = false;
      return { project_id: session.id, status: "saved", path: session.project.project_path, is_dirty: false };
    }
    case "project_save_as": {
      const session = mockSession(args?.project_id);
      session.project.project_path = args?.path as string;
      session.project.is_dirty = false;
      return { project_id: session.id, status: "saved", path: session.project.project_path, is_dirty: false };
    }
    case "project_close": {
      const index = mockSessions.findIndex(s => s.id === args?.project_id);
      if (index === -1) throw "Project session not found";
      const [closed] = mockSessions.splice(index, 1);
      mockAssetDataUrls.delete(closed.id);
      mockAssetMetadata.delete(closed.id);
      if (mockActiveProjectId === closed.id) {
        mockActiveProjectId = mockSessions.length > 0 ? mockSessions[mockSessions.length - 1].id : null;
      }
      return mockSummary(closed);
    }
    case "project_set_active": {
      const session = mockSessions.find(s => s.id === args?.project_id);
      if (!session) throw "Project session not found";
      mockActiveProjectId = session.id;
      return mockSummary(session);
    }
    case "project_list_sessions":
      return mockSessions.map(mockSummary);
    case "project_get_active": {
      const session = mockSession();
      return { summary: mockSummary(session), project: clone(session.project) };
    }
    case "project_summary": {
      const session = mockSession(args?.project_id);
      return mockProjectResult(session);
    }
    case "project_undo": {
      const session = mockSession(args?.project_id);
      const previous = session.undoStack.pop();
      if (!previous) throw "Nothing to undo";
      session.redoStack.push(clone(session.project));
      session.project = previous;
      syncMockAssetMetadataFromProject(session);
      session.project.is_dirty = true;
      session.revision += 1;
      updateMockHistoryFlags(session);
      return mockSummary(session);
    }
    case "project_redo": {
      const session = mockSession(args?.project_id);
      const next = session.redoStack.pop();
      if (!next) throw "Nothing to redo";
      session.undoStack.push(clone(session.project));
      session.project = next;
      syncMockAssetMetadataFromProject(session);
      session.project.is_dirty = true;
      session.revision += 1;
      updateMockHistoryFlags(session);
      return mockSummary(session);
    }
    case "project_export_settings_update": {
      const session = mockSession(args?.projectId);
      const settings = clone(args?.settings as ProjectExportSettings);
      if (settings.codegen_mode === "simple") settings.generate_semantic_registry = false;
      if (settings.codegen_mode === "modular") settings.generate_semantic_registry = true;
      if (JSON.stringify(session.project.export_settings) !== JSON.stringify(settings)) {
        const previous = clone(session.project);
        session.project.export_settings = settings;
        markMockChanged(session, previous);
      }
      return clone(session.project.export_settings);
    }
    case "project_semantic_groups_update": {
      const session = mockSession(args?.projectId);
      const groups = clone((args?.groups as SemanticGroup[]) ?? []);
      if (JSON.stringify(session.project.semantic_groups) !== JSON.stringify(groups)) {
        const previous = clone(session.project);
        session.project.semantic_groups = groups;
        markMockChanged(session, previous);
      }
      return clone(session.project.semantic_groups ?? []);
    }
    case "attached_region_create": {
      const session = mockSession(args?.project_id);
      const region = clone(args?.region as AttachedRegion);
      if ((session.project.attached_regions ?? []).some(existing => existing.id === region.id)) {
        throw `Attached region already exists: ${region.id}`;
      }
      const previous = clone(session.project);
      session.project.attached_regions = [...(session.project.attached_regions ?? []), region];
      markMockChanged(session, previous);
      return clone(region);
    }
    case "attached_region_update": {
      const session = mockSession(args?.project_id);
      const id = String(args?.id);
      const regions = session.project.attached_regions ?? [];
      const index = regions.findIndex(region => region.id === id);
      if (index === -1) throw `Attached region not found: ${id}`;

      const current = regions[index];
      const updated = applyMockAttachedRegionChanges(current, args?.changes);
      if (JSON.stringify(updated) !== JSON.stringify(current)) {
        const previous = clone(session.project);
        session.project.attached_regions = regions.map(region => region.id === id ? clone(updated) : region);
        markMockChanged(session, previous);
      }
      return clone(updated);
    }
    case "attached_region_remove": {
      const session = mockSession(args?.project_id);
      const id = String(args?.id);
      if (!(session.project.attached_regions ?? []).some(region => region.id === id)) return false;

      const previous = clone(session.project);
      session.project.attached_regions = (session.project.attached_regions ?? []).filter(region => region.id !== id);
      for (const element of session.project.elements) {
        if (element.attached_region === id) element.attached_region = null;
      }
      markMockChanged(session, previous);
      return true;
    }
    case "attached_region_list": {
      const session = mockSession(args?.project_id);
      return clone(session.project.attached_regions ?? []);
    }
    case "attached_region_move_with_elements": {
      const session = mockSession(args?.project_id);
      const id = String(args?.id);
      const x = Number(args?.x);
      const y = Number(args?.y);
      const region = (session.project.attached_regions ?? []).find(existing => existing.id === id);
      if (!region) throw `Attached region not found: ${id}`;
      if (region.x === x && region.y === y) return clone(region);

      const dx = x - region.x;
      const dy = y - region.y;
      const previous = clone(session.project);
      const movedChildIds: string[] = [];
      const updated: AttachedRegion = { ...region, x, y };
      session.project.attached_regions = (session.project.attached_regions ?? []).map(existing =>
        existing.id === id ? updated : existing,
      );
      for (const element of session.project.elements) {
        if (element.attached_region !== id) continue;
        element.x += dx;
        element.y += dy;
        movedChildIds.push(element.id);
      }
      refreshMockGroupPositions(session, movedChildIds);
      markMockChanged(session, previous);
      return clone(updated);
    }
    case "element_add": {
      const session = mockSession(args?.project_id);
      const previous = clone(session.project);
      const element = args?.element as Element;
      const added = { visible: true, ...clone(element) };
      session.project.elements.push(added);
      markMockChanged(session, previous);
      return clone(added);
    }
    case "element_move": {
      const session = mockSession(args?.project_id);
      const el = session.project.elements.find(e => e.id === args?.id);
      if (!el) throw "Element not found";
      const x = args?.x as number;
      const y = args?.y as number;
      if (el.x !== x || el.y !== y) {
        const previous = clone(session.project);
        el.x = x;
        el.y = y;
        refreshMockGroupPositions(session, [el.id]);
        markMockChanged(session, previous);
      }
      return clone(el);
    }
    case "element_move_many": {
      const session = mockSession(args?.project_id);
      const moves = ((args?.moves as ElementMoveRequest[] | undefined) ?? []).map(move => ({
        id: move.id,
        x: move.x,
        y: move.y,
      }));
      if (moves.length === 0) return [];

      const seen = new Set<string>();
      const elements = moves.map(move => {
        if (seen.has(move.id)) throw `Duplicate element move: ${move.id}`;
        seen.add(move.id);
        const el = session.project.elements.find(element => element.id === move.id);
        if (!el) throw `Element not found: ${move.id}`;
        return el;
      });

      if (moves.some((move, index) => elements[index].x !== move.x || elements[index].y !== move.y)) {
        const previous = clone(session.project);
        moves.forEach((move, index) => {
          elements[index].x = move.x;
          elements[index].y = move.y;
        });
        refreshMockGroupPositions(session, moves.map(move => move.id));
        markMockChanged(session, previous);
      }

      return moves.map(move => clone(session.project.elements.find(element => element.id === move.id)!));
    }
    case "element_update": {
      const session = mockSession(args?.project_id);
      const el = session.project.elements.find(e => e.id === args?.id);
      if (!el) throw "Element not found";
      const next = { ...el, ...(args?.changes as Partial<Element>) };
      if (JSON.stringify(next) !== JSON.stringify(el)) {
        const previous = clone(session.project);
        Object.assign(el, clone(next));
        markMockChanged(session, previous);
      }
      return clone(el);
    }
    case "element_resize": {
      const session = mockSession(args?.project_id);
      const el = session.project.elements.find(e => e.id === args?.id);
      if (!el) throw "Element not found";
      const x = args?.x as number;
      const y = args?.y as number;
      const width = args?.width as number;
      const height = args?.height as number;
      const previous = clone(session.project);
      el.x = x;
      el.y = y;
      if (el.type === "slot") {
        el.size = Math.max(8, width, height);
      } else {
        el.width = Math.max(4, width);
        el.height = Math.max(4, height);
      }
      if (JSON.stringify(previous.elements.find(e => e.id === el.id)) !== JSON.stringify(el)) {
        markMockChanged(session, previous);
      }
      return clone(el);
    }
    case "element_reorder": {
      const session = mockSession(args?.project_id);
      const index = session.project.elements.findIndex(e => e.id === args?.id);
      if (index === -1) throw "Element not found";
      const target = Math.max(0, Math.min(args?.index as number, session.project.elements.length - 1));
      if (index !== target) {
        const previous = clone(session.project);
        const [element] = session.project.elements.splice(index, 1);
        session.project.elements.splice(target, 0, element);
        markMockChanged(session, previous);
      }
      return mockSummary(session);
    }
    case "element_remove": {
      const session = mockSession(args?.project_id);
      const before = session.project.elements.length;
      const previous = clone(session.project);
      session.project.elements = session.project.elements.filter(e => e.id !== args?.id);
      for (const group of session.project.groups) {
        group.elements = group.elements.filter(id => id !== args?.id);
      }
      session.project.groups = session.project.groups.filter(group => group.elements.length >= 2);
      if (session.project.elements.length !== before) markMockChanged(session, previous);
      return session.project.elements.length !== before;
    }
    case "element_list": {
      const session = mockSession(args?.project_id);
      return clone(session.project.elements);
    }
    case "group_create": {
      const session = mockSession(args?.project_id);
      const elementIds = [...new Set((args?.element_ids as string[]) ?? [])];
      if (elementIds.length < 2) throw "At least two elements are required to create a group";
      for (const id of elementIds) {
        if (!session.project.elements.some(element => element.id === id)) throw `Element not found: ${id}`;
      }
      const groupId = (args?.group_id as string | undefined) || `group_${Date.now().toString(36)}`;
      if (session.project.groups.some(group => group.id === groupId)) throw "Group already exists";
      const previous = clone(session.project);
      for (const group of session.project.groups) {
        group.elements = group.elements.filter(id => !elementIds.includes(id));
      }
      session.project.groups = session.project.groups.filter(group => group.elements.length >= 2);
      const groupedElements = elementIds.map(id => session.project.elements.find(element => element.id === id)!);
      const group: Group = {
        id: groupId,
        x: Math.min(...groupedElements.map(element => element.x)),
        y: Math.min(...groupedElements.map(element => element.y)),
        elements: elementIds,
      };
      session.project.groups.push(group);
      markMockChanged(session, previous);
      return clone(group);
    }
    case "group_ungroup": {
      const session = mockSession(args?.project_id);
      const before = session.project.groups.length;
      const previous = clone(session.project);
      session.project.groups = session.project.groups.filter(group => group.id !== args?.group_id);
      if (session.project.groups.length !== before) markMockChanged(session, previous);
      return session.project.groups.length !== before;
    }
    case "group_list": {
      const session = mockSession(args?.project_id);
      return clone(session.project.groups);
    }
    case "animation_create": {
      const session = mockSession(args?.project_id);
      const animation = clone(args?.animation as Animation);
      if (session.project.animations.some(a => a.id === animation.id)) throw "Animation already exists";
      const previous = clone(session.project);
      session.project.animations.push(animation);
      markMockChanged(session, previous);
      return clone(animation);
    }
    case "animation_update": {
      const session = mockSession(args?.project_id);
      const animation = session.project.animations.find(a => a.id === args?.id);
      if (!animation) throw "Animation not found";
      const next = { ...animation, ...(args?.changes as Partial<Animation>) };
      if (JSON.stringify(next) !== JSON.stringify(animation)) {
        const previous = clone(session.project);
        Object.assign(animation, clone(next));
        markMockChanged(session, previous);
      }
      return clone(animation);
    }
    case "animation_remove": {
      const session = mockSession(args?.project_id);
      const before = session.project.animations.length;
      const previous = clone(session.project);
      session.project.animations = session.project.animations.filter(a => a.id !== args?.id);
      for (const element of session.project.elements) {
        if (element.animation === args?.id) element.animation = undefined;
      }
      if (JSON.stringify(previous) !== JSON.stringify(session.project) && session.project.animations.length !== before) {
        markMockChanged(session, previous);
      }
      return session.project.animations.length !== before;
    }
    case "animation_bind": {
      const session = mockSession(args?.project_id);
      if (!session.project.animations.some(a => a.id === args?.animation_id)) throw "Animation not found";
      const element = session.project.elements.find(e => e.id === args?.element_id);
      if (!element) throw "Element not found";
      if (element.animation !== args?.animation_id) {
        const previous = clone(session.project);
        element.animation = args?.animation_id as string;
        markMockChanged(session, previous);
      }
      return clone(element);
    }
    case "animation_unbind": {
      const session = mockSession(args?.project_id);
      const element = session.project.elements.find(e => e.id === args?.element_id);
      if (!element) throw "Element not found";
      if (element.animation !== undefined) {
        const previous = clone(session.project);
        element.animation = undefined;
        markMockChanged(session, previous);
      }
      return clone(element);
    }
    case "asset_import": {
      const session = mockSession(args?.project_id);
      const name = `textures/${String(args?.file_path ?? "texture").split("/").pop()?.replace(/\.[^.]+$/, "") || "texture"}.png`;
      const dataUrl = typeof args?.data_url === "string" ? args.data_url : "";
      const metadata = mockAssetMetadataForSession(session).get(name);
      if (!session.project.assets.includes(name)) {
        const previous = clone(session.project);
        session.project.assets.push(name);
        markMockChanged(session, previous);
      }
      if (dataUrl) mockAssetsForSession(session).set(name, dataUrl);
      return mockAssetResult(name, dataUrl, metadata, true);
    }
    case "asset_update": {
      const session = mockSession(args?.project_id);
      const name = String(args?.name ?? "");
      const dataUrl = String(args?.data_url ?? "");
      if (!dataUrl.startsWith("data:image/png;base64,")) throw "Invalid asset data URL: expected data:image/png;base64,...";
      if (!session.project.assets.includes(name)) throw `Asset not found: ${name}`;
      const assets = mockAssetsForSession(session);
      if (assets.get(name) !== dataUrl) {
        const previous = clone(session.project);
        assets.set(name, dataUrl);
        markMockChanged(session, previous);
      }
      return mockAssetResult(name, dataUrl, mockAssetMetadataForSession(session).get(name), true);
    }
    case "asset_list": {
      const session = mockSession(args?.project_id);
      const assets = mockAssetsForSession(session);
      const metadata = mockAssetMetadataForSession(session);
      return Promise.all(session.project.assets.map(async name => {
        const dataUrl = assets.get(name) ?? "";
        return mockAssetResult(name, dataUrl, metadata.get(name));
      }));
    }
    case "asset_metadata_update": {
      const session = mockSession(args?.project_id);
      const name = String(args?.name ?? "");
      if (!session.project.assets.includes(name)) throw `Asset not found: ${name}`;
      const metadata = clone(args?.metadata as AssetMetadata);
      const metadataMap = mockAssetMetadataForSession(session);
      const current = metadataMap.get(name);
      if (current !== undefined && JSON.stringify(current) === JSON.stringify(metadata)) {
        return clone(current);
      }
      const previous = clone(session.project);
      metadataMap.set(name, metadata);
      session.project.asset_metadata = Object.fromEntries(metadataMap.entries());
      markMockChanged(session, previous);
      return clone(metadata);
    }
    case "asset_remove": {
      const session = mockSession(args?.project_id);
      const before = session.project.assets.length;
      const previous = clone(session.project);
      session.project.assets = session.project.assets.filter(name => name !== args?.name);
      mockAssetsForSession(session).delete(String(args?.name ?? ""));
      mockAssetMetadataForSession(session).delete(String(args?.name ?? ""));
      session.project.asset_metadata = Object.fromEntries(mockAssetMetadataForSession(session).entries());
      if (session.project.assets.length !== before) markMockChanged(session, previous);
      return session.project.assets.length !== before;
    }
    case "asset_get_data_url": {
      const session = mockSession(args?.project_id);
      const dataUrl = mockAssetsForSession(session).get(String(args?.name ?? ""));
      if (dataUrl === undefined) throw `Asset not found: ${String(args?.name ?? "")}`;
      return dataUrl;
    }
    case "list_minecraft_sources":
      return [];
    case "font_list":
      return [{ id: "minecraft:default", source: { type: "minecraft" } }];
    case "font_glyph_map":
      return {};
    case "font_render_data": {
      const mockChars = " ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
      const glyphMap = Object.fromEntries(Array.from(mockChars).map(ch => [
        ch,
        ch === " "
          ? { x: 0, y: 0, width: 0, height: 0, ascent: 0, advance: 4 }
          : { x: 0, y: 0, width: 1, height: 1, ascent: 1, advance: 5, bearing_x: 0, bearing_y: 0 },
      ]));
      return {
        id: String(args?.font_id ?? "minecraft:default"),
        source_type: "minecraft",
        providers: [{
          file: "minecraft:font/ascii.png",
          ascent: 7,
          chars: [mockChars],
          image_width: 1,
          image_height: 1,
          image_data_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/lP9Y9QAAAABJRU5ErkJggg==",
        }],
        glyph_map: glyphMap,
      };
    }
    case "font_import":
      throw "Mock: font import not supported in browser mode";
    case "project_export_preview":
      return mockExportPreview(args);
    case "project_export": {
      const preview = mockExportPreview(args);
      if (preview.errors.length > 0) throw preview.errors.join("\n");
      return preview.files;
    }
    case "mcp_status":
      return null;
    case "template_list":
      return [
        { name: "empty", description: "Blank canvas of configurable size", default_width: 176, default_height: 166, element_count: 0 },
        { name: "furnace", description: "Furnace: input, fuel, progress arrow, output, player inventory", default_width: 176, default_height: 166, element_count: 6 },
        { name: "crafting_3x3", description: "3x3 crafting grid with output slot", default_width: 176, default_height: 166, element_count: 11 },
        { name: "chest_9x3", description: "Standard chest inventory (9x3 grid)", default_width: 176, default_height: 166, element_count: 28 },
        { name: "chest_9x6", description: "Double chest inventory (9x6 grid)", default_width: 176, default_height: 222, element_count: 55 },
        { name: "advanced_machine", description: "Advanced machine: input, fuel, output, progress arrow, 2 fluid tanks, energy bar", default_width: 176, default_height: 166, element_count: 9 },
        { name: "fluid_tank", description: "Fluid tank: input/output slots, fluid fill gauge, capacity text", default_width: 176, default_height: 166, element_count: 6 },
        { name: "brewing_stand", description: "Brewing stand: 3 bottles, ingredient, blaze powder, progress bubbles, fuel gauge", default_width: 176, default_height: 166, element_count: 12 },
        { name: "anvil", description: "Anvil: 2 input slots, output, level cost text, repair progress", default_width: 176, default_height: 166, element_count: 7 },
        { name: "custom_grid", description: "Custom N×M grid with optional output, progress, and inventory", default_width: 176, default_height: 166, element_count: 39 },
      ];
    default:
      throw `Unknown command: ${cmd}`;
  }
}

export async function projectNew(name: string, width: number, height: number, modTarget: string, template?: string): Promise<ProjectSummary> {
  const invoke = await getInvoke();
  return invoke("project_new", { name, width, height, mod_target: modTarget, template }) as Promise<ProjectSummary>;
}

export async function appConfigGet(): Promise<AppConfig> {
  const invoke = await getInvoke();
  return invoke("app_config_get") as Promise<AppConfig>;
}

export async function editorLayoutSave(layout: EditorLayoutConfig): Promise<AppConfig> {
  const invoke = await getInvoke();
  return invoke("editor_layout_save", { layout }) as Promise<AppConfig>;
}

export async function appWindowSave(window: WindowConfig): Promise<AppConfig> {
  const invoke = await getInvoke();
  return invoke("app_window_save", { window }) as Promise<AppConfig>;
}

export async function uiLayoutReset(): Promise<AppConfig> {
  const invoke = await getInvoke();
  return invoke("ui_layout_reset") as Promise<AppConfig>;
}

export async function projectOpen(path: string): Promise<ProjectSummary> {
  const invoke = await getInvoke();
  return invoke("project_open", { path }) as Promise<ProjectSummary>;
}

export async function projectSave(projectId?: string): Promise<SaveProjectResult> {
  const invoke = await getInvoke();
  return invoke("project_save", { project_id: projectId }) as Promise<SaveProjectResult>;
}

export async function projectSaveAs(path: string, projectId?: string): Promise<SaveProjectResult> {
  const invoke = await getInvoke();
  return invoke("project_save_as", { project_id: projectId, path }) as Promise<SaveProjectResult>;
}

export async function projectClose(projectId: string): Promise<ProjectSessionSummary> {
  const invoke = await getInvoke();
  return invoke("project_close", { project_id: projectId }) as Promise<ProjectSessionSummary>;
}

export async function projectSetActive(projectId: string): Promise<ProjectSessionSummary> {
  const invoke = await getInvoke();
  return invoke("project_set_active", { project_id: projectId }) as Promise<ProjectSessionSummary>;
}

export async function projectListSessions(): Promise<ProjectSessionSummary[]> {
  const invoke = await getInvoke();
  return invoke("project_list_sessions") as Promise<ProjectSessionSummary[]>;
}

export async function projectGetActive(): Promise<ActiveProjectPayload> {
  const invoke = await getInvoke();
  return invoke("project_get_active") as Promise<ActiveProjectPayload>;
}

export async function projectSummary(projectId?: string): Promise<ProjectSummary> {
  const invoke = await getInvoke();
  return invoke("project_summary", { project_id: projectId }) as Promise<ProjectSummary>;
}

export async function projectUndo(projectId?: string): Promise<ProjectSessionSummary> {
  const invoke = await getInvoke();
  return invoke("project_undo", { project_id: projectId }) as Promise<ProjectSessionSummary>;
}

export async function projectRedo(projectId?: string): Promise<ProjectSessionSummary> {
  const invoke = await getInvoke();
  return invoke("project_redo", { project_id: projectId }) as Promise<ProjectSessionSummary>;
}

export async function projectExportSettingsUpdate(settings: ProjectExportSettings, projectId?: string): Promise<ProjectExportSettings> {
  const invoke = await getInvoke();
  return invoke("project_export_settings_update", { projectId, settings }) as Promise<ProjectExportSettings>;
}

export async function projectSemanticGroupsUpdate(groups: SemanticGroup[], projectId?: string): Promise<SemanticGroup[]> {
  const invoke = await getInvoke();
  return invoke("project_semantic_groups_update", { projectId, groups }) as Promise<SemanticGroup[]>;
}

export async function attachedRegionCreate(region: AttachedRegion, projectId?: string): Promise<AttachedRegion> {
  const invoke = await getInvoke();
  return invoke("attached_region_create", { region, project_id: projectId }) as Promise<AttachedRegion>;
}

export async function attachedRegionUpdate(id: string, changes: Partial<AttachedRegion>, projectId?: string): Promise<AttachedRegion> {
  const invoke = await getInvoke();
  return invoke("attached_region_update", { id, changes, project_id: projectId }) as Promise<AttachedRegion>;
}

export async function attachedRegionRemove(id: string, projectId?: string): Promise<boolean> {
  const invoke = await getInvoke();
  return invoke("attached_region_remove", { id, project_id: projectId }) as Promise<boolean>;
}

export async function attachedRegionList(projectId?: string): Promise<AttachedRegion[]> {
  const invoke = await getInvoke();
  return invoke("attached_region_list", { project_id: projectId }) as Promise<AttachedRegion[]>;
}

export async function attachedRegionMoveWithElements(id: string, x: number, y: number, projectId?: string): Promise<AttachedRegion> {
  const invoke = await getInvoke();
  return invoke("attached_region_move_with_elements", { id, x, y, project_id: projectId }) as Promise<AttachedRegion>;
}

export async function elementAdd(element: Element, projectId?: string): Promise<Element> {
  const invoke = await getInvoke();
  return invoke("element_add", { element, project_id: projectId }) as Promise<Element>;
}

export async function elementMove(id: string, x: number, y: number, projectId?: string): Promise<Element> {
  const invoke = await getInvoke();
  return invoke("element_move", { id, x, y, project_id: projectId }) as Promise<Element>;
}

export async function elementMoveMany(moves: ElementMoveRequest[], projectId?: string): Promise<Element[]> {
  const invoke = await getInvoke();
  return invoke("element_move_many", { moves, project_id: projectId }) as Promise<Element[]>;
}

export async function elementUpdate(id: string, changes: Partial<Element>, projectId?: string): Promise<Element> {
  const invoke = await getInvoke();
  return invoke("element_update", { id, changes, project_id: projectId }) as Promise<Element>;
}

export async function elementResize(id: string, x: number, y: number, width: number, height: number, projectId?: string): Promise<Element> {
  const invoke = await getInvoke();
  return invoke("element_resize", { id, x, y, width, height, project_id: projectId }) as Promise<Element>;
}

export async function elementReorder(id: string, index: number, projectId?: string): Promise<ProjectSessionSummary> {
  const invoke = await getInvoke();
  return invoke("element_reorder", { id, index, project_id: projectId }) as Promise<ProjectSessionSummary>;
}

export async function elementRemove(id: string, projectId?: string): Promise<boolean> {
  const invoke = await getInvoke();
  return invoke("element_remove", { id, project_id: projectId }) as Promise<boolean>;
}

export async function elementList(projectId?: string): Promise<Element[]> {
  const invoke = await getInvoke();
  return invoke("element_list", { project_id: projectId }) as Promise<Element[]>;
}

export async function groupCreate(elementIds: string[], groupId?: string, projectId?: string): Promise<Group> {
  const invoke = await getInvoke();
  return invoke("group_create", { element_ids: elementIds, group_id: groupId, project_id: projectId }) as Promise<Group>;
}

export async function groupUngroup(groupId: string, projectId?: string): Promise<boolean> {
  const invoke = await getInvoke();
  return invoke("group_ungroup", { group_id: groupId, project_id: projectId }) as Promise<boolean>;
}

export async function groupList(projectId?: string): Promise<Group[]> {
  const invoke = await getInvoke();
  return invoke("group_list", { project_id: projectId }) as Promise<Group[]>;
}

export async function animationCreate(animation: Animation, projectId?: string): Promise<Animation> {
  const invoke = await getInvoke();
  return invoke("animation_create", { animation, project_id: projectId }) as Promise<Animation>;
}

export async function animationUpdate(id: string, changes: Partial<Animation>, projectId?: string): Promise<Animation> {
  const invoke = await getInvoke();
  return invoke("animation_update", { id, changes, project_id: projectId }) as Promise<Animation>;
}

export async function animationRemove(id: string, projectId?: string): Promise<boolean> {
  const invoke = await getInvoke();
  return invoke("animation_remove", { id, project_id: projectId }) as Promise<boolean>;
}

export async function animationBind(elementId: string, animationId: string, projectId?: string): Promise<Element> {
  const invoke = await getInvoke();
  return invoke("animation_bind", { element_id: elementId, animation_id: animationId, project_id: projectId }) as Promise<Element>;
}

export async function animationUnbind(elementId: string, projectId?: string): Promise<Element> {
  const invoke = await getInvoke();
  return invoke("animation_unbind", { element_id: elementId, project_id: projectId }) as Promise<Element>;
}

export interface TemplateInfo {
  name: string;
  description: string;
  default_width: number;
  default_height: number;
  element_count: number;
}

export async function templateList(): Promise<TemplateInfo[]> {
  const invoke = await getInvoke();
  return invoke("template_list") as Promise<TemplateInfo[]>;
}

export interface McpServerStatus {
  address: string;
}

export async function mcpStatus(): Promise<McpServerStatus | null> {
  const invoke = await getInvoke();
  return invoke("mcp_status") as Promise<McpServerStatus | null>;
}

export interface AssetImportResult {
  name: string;
  width: number;
  height: number;
  bytes: number;
  sha256: string;
  data_url?: string;
  nine_slice?: NineSlice | null;
}

export interface ExportPreview {
  target: ModTarget;
  mod_id: string;
  package: string;
  class_name: string;
  output_dir: string;
  files: string[];
  warnings: string[];
  errors: string[];
}

export interface ExportSettingsOverride {
  codegen_mode: CodegenMode;
  generate_runtime_helpers: boolean;
  generate_semantic_registry: boolean;
  overwrite?: boolean;
}

export async function assetImport(filePath: string, projectId?: string, dataUrl?: string): Promise<AssetImportResult> {
  const invoke = await getInvoke();
  return invoke("asset_import", { file_path: filePath, project_id: projectId, data_url: dataUrl }) as Promise<AssetImportResult>;
}

export async function assetUpdate(name: string, dataUrl: string, projectId?: string): Promise<AssetImportResult> {
  const invoke = await getInvoke();
  return invoke("asset_update", { name, data_url: dataUrl, project_id: projectId }) as Promise<AssetImportResult>;
}

export async function assetList(projectId?: string): Promise<AssetImportResult[]> {
  const invoke = await getInvoke();
  return invoke("asset_list", { project_id: projectId }) as Promise<AssetImportResult[]>;
}

export async function assetRemove(name: string, projectId?: string): Promise<boolean> {
  const invoke = await getInvoke();
  return invoke("asset_remove", { name, project_id: projectId }) as Promise<boolean>;
}

export async function assetGetDataUrl(name: string, projectId?: string): Promise<string> {
  const invoke = await getInvoke();
  return invoke("asset_get_data_url", { name, project_id: projectId }) as Promise<string>;
}

export async function assetMetadataUpdate(name: string, metadata: AssetMetadata, projectId?: string): Promise<AssetMetadata> {
  const invoke = await getInvoke();
  return invoke("asset_metadata_update", { name, metadata, project_id: projectId }) as Promise<AssetMetadata>;
}

export async function projectExportPreview(
  target: ModTarget,
  modId: string,
  packageName: string,
  className: string,
  outputDir: string,
  projectId?: string,
  settingsOverride?: ExportSettingsOverride,
): Promise<ExportPreview> {
  const invoke = await getInvoke();
  return invoke("project_export_preview", {
    target,
    mod_id: modId,
    package: packageName,
    class_name: className,
    output_dir: outputDir,
    project_id: projectId,
    ...settingsOverride,
  }) as Promise<ExportPreview>;
}

export async function listMinecraftSources(): Promise<MinecraftSource[]> {
  const invoke = await getInvoke();
  return invoke("list_minecraft_sources") as Promise<MinecraftSource[]>;
}

export async function fontImport(filePath: string, projectId?: string): Promise<FontAsset> {
  const invoke = await getInvoke();
  return invoke("font_import", { file_path: filePath, project_id: projectId }) as Promise<FontAsset>;
}

export async function fontList(projectId?: string): Promise<FontAsset[]> {
  const invoke = await getInvoke();
  return invoke("font_list", { project_id: projectId }) as Promise<FontAsset[]>;
}

export async function fontGlyphMap(fontId: string, projectId?: string): Promise<Record<string, GlyphInfo>> {
  const invoke = await getInvoke();
  return invoke("font_glyph_map", { font_id: fontId, project_id: projectId }) as Promise<Record<string, GlyphInfo>>;
}

export async function fontRenderData(fontId: string, projectId?: string): Promise<FontRenderData> {
  const invoke = await getInvoke();
  return invoke("font_render_data", { font_id: fontId, project_id: projectId }) as Promise<FontRenderData>;
}

export async function projectExport(
  target: ModTarget,
  modId: string,
  packageName: string,
  className: string,
  outputDir: string,
  projectId?: string,
  settingsOverride?: ExportSettingsOverride,
): Promise<string[]> {
  const invoke = await getInvoke();
  return invoke("project_export", {
    target,
    mod_id: modId,
    package: packageName,
    class_name: className,
    output_dir: outputDir,
    project_id: projectId,
    ...settingsOverride,
  }) as Promise<string[]>;
}

export async function showOpenDialog(): Promise<string | null> {
  try {
    const dialog = await import("@tauri-apps/plugin-dialog");
    const result = await dialog.open({
      filters: [{ name: "MCGUI Project", extensions: ["mcgui"] }],
      multiple: false,
    });
    return result as string | null;
  } catch {
    return prompt("Enter path to open:") || null;
  }
}

export async function showSaveDialog(): Promise<string | null> {
  try {
    const dialog = await import("@tauri-apps/plugin-dialog");
    const result = await dialog.save({
      filters: [{ name: "MCGUI Project", extensions: ["mcgui"] }],
    });
    return result as string | null;
  } catch {
    return prompt("Enter path to save:") || null;
  }
}
