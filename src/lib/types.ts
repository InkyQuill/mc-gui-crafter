export type Layer = "background" | "overlay" | "animatable";

export type ElementType =
  | "texture"
  | "slot"
  | "progress"
  | "text"
  | "fluid_tank"
  | "energy_bar"
  | "scrollbar"
  | "button"
  | "toggle_button"
  | "text_input"
  | "tab"
  | "panel"
  | "virtual_slot_cell";

export type FillDirection = "left_to_right" | "right_to_left" | "bottom_to_top" | "top_to_bottom";

export type ModTarget = "forge" | "fabric" | "neoforge";

export type SlotRole =
  | "machine"
  | "player_inventory"
  | "hotbar"
  | "scrollable_inventory"
  | "virtual_storage"
  | "upgrade"
  | "upgrade_settings"
  | "filter"
  | "ghost"
  | "offhand";

export type SemanticGroupKind =
  | "fixed_slots"
  | "virtual_slot_grid"
  | "player_inventory"
  | "hotbar"
  | "upgrade_slots"
  | "upgrade_panel"
  | "search_field"
  | "control_buttons";

export interface SemanticGroup {
  id: string;
  kind: SemanticGroupKind;
  columns?: number;
  visible_rows?: number;
  total_rows?: number;
  slot_count?: number;
  data_source?: string;
  scroll_binding?: string;
  dynamic_height?: boolean;
}

export type CodegenMode = "simple" | "modular";

export interface ProjectExportSettings {
  codegen_mode: CodegenMode;
  generate_runtime_helpers: boolean;
  generate_semantic_registry: boolean;
}

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
  icon?: string | null;
  icon_uv?: UvRect | null;
  tooltip?: string | null;
  direction?: FillDirection;
  content?: string;
  font?: string;
  color?: number;
  shadow?: boolean;
  animation?: string;
  visible?: boolean;
  uv?: UvRect | null;
  layer?: Layer;
  slot_role?: SlotRole | null;
  slot_index?: number | null;
  inventory_group?: string | null;
  scroll_binding?: string | null;
  scroll_min?: number;
  scroll_max?: number;
  visible_rows?: number | null;
  total_rows?: number | null;
  columns?: number | null;
  target_group?: string | null;
  binding?: string | null;
  dock?: string;
  open_width?: number;
  open_height?: number;
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
  advance?: number;
  bearing_x?: number;
  bearing_y?: number;
}

export interface FontAsset {
  id: string;
  source: { type: "minecraft" | "ttf"; font_size?: number; glyph_map?: Record<string, GlyphInfo> };
}

export interface MinecraftFontProviderRenderData {
  file: string;
  ascent: number;
  chars: string[];
  image_width: number;
  image_height: number;
  image_data_url: string;
}

export interface MinecraftFontRenderData {
  id: string;
  source_type: "minecraft";
  providers: MinecraftFontProviderRenderData[];
  glyph_map: Record<string, GlyphInfo>;
}

export interface TtfFontRenderData {
  id: string;
  source_type: "ttf";
  font_size: number;
  atlas_data_url: string;
  glyph_map: Record<string, GlyphInfo>;
}

export type FontRenderData = MinecraftFontRenderData | TtfFontRenderData;

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
  semantic_groups?: SemanticGroup[];
  export_settings?: ProjectExportSettings;
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
