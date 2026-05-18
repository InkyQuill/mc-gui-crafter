export type Layer = "background" | "overlay" | "animatable";

export type ElementType = "texture" | "slot" | "progress" | "text" | "fluid_tank" | "energy_bar";

export type FillDirection = "left_to_right" | "right_to_left" | "bottom_to_top" | "top_to_bottom";

export type ModTarget = "forge" | "fabric" | "neoforge";

export interface Size {
  width: number;
  height: number;
}

export interface UvRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Element {
  id: string;
  type: ElementType;
  x: number;
  y: number;
  width?: number;
  height?: number;
  size?: number;
  asset?: string;
  direction?: FillDirection;
  content?: string;
  font?: string;
  color?: number;
  shadow?: boolean;
  animation?: string;
  visible?: boolean;
  uv?: UvRect | null;
  layer?: Layer;
}

export interface Group {
  id: string;
  x: number;
  y: number;
  elements: string[];
}

export interface Animation {
  id: string;
  type: "fill" | "cycle" | "pulse" | "toggle";
  data_key: string;
  texture?: string;
  direction?: FillDirection;
  frame_count?: number;
  fps?: number;
  min_value?: number;
  max_value?: number;
  triggers_on?: string;
}

export interface ProjectSummary {
  project_id: string;
  name: string;
  gui_size: Size;
  mod_target: ModTarget;
  element_count: number;
  is_dirty: boolean;
  revision: number;
  path?: string | null;
  session: ProjectSessionSummary;
}

export interface GlyphInfo {
  x: number;
  y: number;
  width: number;
  height: number;
  ascent: number;
}

export interface FontAsset {
  id: string;
  source: { type: "minecraft" | "ttf"; font_size?: number; glyph_map?: Record<string, GlyphInfo> };
}

export interface MinecraftSource {
  name: string;
  path: string;
  source_type: "prismlauncher" | "gradle_dev" | "vanilla";
}

export interface ProjectData {
  name: string;
  gui_size: Size;
  mod_target: ModTarget;
  elements: Element[];
  groups: Group[];
  animations: Animation[];
  assets: string[];
  project_path?: string | null;
  is_dirty?: boolean;
  fonts?: FontAsset[];
}

export interface ProjectSessionSummary {
  id: string;
  name: string;
  path?: string | null;
  active: boolean;
  is_dirty: boolean;
  revision: number;
  element_count: number;
  can_undo: boolean;
  can_redo: boolean;
}

export interface ActiveProjectPayload {
  summary: ProjectSessionSummary;
  project: ProjectData;
}

export interface SaveProjectResult {
  project_id: string;
  status: "saved";
  path?: string | null;
  is_dirty: boolean;
}

export interface AssetInfo {
  name: string;
  width: number;
  height: number;
  data_url: string;
}
