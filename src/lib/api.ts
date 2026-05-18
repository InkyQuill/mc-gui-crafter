import type {
  ActiveProjectPayload,
  Animation,
  Element,
  FontAsset,
  GlyphInfo,
  Group,
  MinecraftSource,
  ModTarget,
  ProjectData,
  ProjectSessionSummary,
  ProjectSummary,
  SaveProjectResult,
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

const mockSessions: MockSession[] = [];
const mockAssetDataUrls = new Map<string, Map<string, string>>();
const mockExistingExportFiles = new Set<string>();
let mockActiveProjectId: string | null = null;
let mockNextProjectId = 1;

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
  mockActiveProjectId = id;
  return mockProjectResult(session);
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

function mockAssetsForSession(session: MockSession): Map<string, string> {
  let assets = mockAssetDataUrls.get(session.id);
  if (!assets) {
    assets = new Map();
    mockAssetDataUrls.set(session.id, assets);
  }
  return assets;
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
    warnings: files
      .filter(path => mockExistingExportFiles.has(path))
      .map(path => `Target file already exists and will be overwritten: ${path}`),
    errors,
  };
}

async function mockInvoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "project_new":
      return createMockSession({
        name: (args?.name as string) || "Untitled",
        gui_size: { width: (args?.width as number) || 176, height: (args?.height as number) || 166 },
        mod_target: (args?.mod_target as ModTarget) || "forge",
        elements: [],
        groups: [],
        animations: [],
        assets: [],
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
      session.project.is_dirty = true;
      session.revision += 1;
      updateMockHistoryFlags(session);
      return mockSummary(session);
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
        markMockChanged(session, previous);
      }
      return clone(el);
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
      if (!session.project.assets.includes(name)) {
        const previous = clone(session.project);
        session.project.assets.push(name);
        markMockChanged(session, previous);
      }
      if (dataUrl) mockAssetsForSession(session).set(name, dataUrl);
      const dimensions = await dataUrlDimensions(dataUrl);
      return { name, width: dimensions.width, height: dimensions.height, data_url: dataUrl };
    }
    case "asset_update": {
      const session = mockSession(args?.project_id);
      const name = String(args?.name ?? "");
      const dataUrl = String(args?.data_url ?? "");
      if (!dataUrl.startsWith("data:image/png;base64,")) throw "Invalid asset data URL: expected data:image/png;base64,...";
      if (!session.project.assets.includes(name)) throw `Asset not found: ${name}`;
      const dimensions = await dataUrlDimensions(dataUrl);
      const assets = mockAssetsForSession(session);
      if (assets.get(name) !== dataUrl) {
        const previous = clone(session.project);
        assets.set(name, dataUrl);
        markMockChanged(session, previous);
      }
      return { name, width: dimensions.width, height: dimensions.height, data_url: dataUrl };
    }
    case "asset_list": {
      const session = mockSession(args?.project_id);
      const assets = mockAssetsForSession(session);
      return Promise.all(session.project.assets.map(async name => {
        const dataUrl = assets.get(name) ?? "";
        const dimensions = await dataUrlDimensions(dataUrl);
        return { name, width: dimensions.width, height: dimensions.height, data_url: dataUrl };
      }));
    }
    case "asset_remove": {
      const session = mockSession(args?.project_id);
      const before = session.project.assets.length;
      const previous = clone(session.project);
      session.project.assets = session.project.assets.filter(name => name !== args?.name);
      mockAssetsForSession(session).delete(String(args?.name ?? ""));
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

export async function elementAdd(element: Element, projectId?: string): Promise<Element> {
  const invoke = await getInvoke();
  return invoke("element_add", { element, project_id: projectId }) as Promise<Element>;
}

export async function elementMove(id: string, x: number, y: number, projectId?: string): Promise<Element> {
  const invoke = await getInvoke();
  return invoke("element_move", { id, x, y, project_id: projectId }) as Promise<Element>;
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
  data_url: string;
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

export async function projectExportPreview(
  target: ModTarget,
  modId: string,
  packageName: string,
  className: string,
  outputDir: string,
  projectId?: string,
): Promise<ExportPreview> {
  const invoke = await getInvoke();
  return invoke("project_export_preview", {
    target,
    mod_id: modId,
    package: packageName,
    class_name: className,
    output_dir: outputDir,
    project_id: projectId,
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

export async function projectExport(
  target: ModTarget,
  modId: string,
  packageName: string,
  className: string,
  outputDir: string,
  projectId?: string,
): Promise<string[]> {
  const invoke = await getInvoke();
  return invoke("project_export", {
    target,
    mod_id: modId,
    package: packageName,
    class_name: className,
    output_dir: outputDir,
    project_id: projectId,
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
