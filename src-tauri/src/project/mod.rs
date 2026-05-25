use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

macro_rules! iterable_enum {
    (
        $(#[$enum_meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )+
        }

        impl $name {
            pub fn variants() -> impl Iterator<Item = Self> {
                [$(Self::$variant),+].into_iter()
            }
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum ModTarget {
        #[serde(alias = "Forge")]
        Forge,
        #[serde(alias = "Fabric")]
        Fabric,
        #[serde(rename = "neoforge", alias = "NeoForge", alias = "neo_forge")]
        NeoForge,
    }
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum ElementType {
        #[serde(alias = "Texture")]
        Texture,
        #[serde(alias = "Slot")]
        Slot,
        #[serde(alias = "Progress")]
        Progress,
        #[serde(alias = "Text")]
        Text,
        #[serde(alias = "FluidTank")]
        FluidTank,
        #[serde(alias = "EnergyBar")]
        EnergyBar,
        #[serde(alias = "Scrollbar")]
        Scrollbar,
        #[serde(alias = "Button")]
        Button,
        #[serde(alias = "ToggleButton")]
        ToggleButton,
        #[serde(alias = "TextInput")]
        TextInput,
        #[serde(alias = "Tab")]
        Tab,
        #[serde(alias = "Panel")]
        Panel,
        #[serde(alias = "VirtualSlotCell")]
        VirtualSlotCell,
    }
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum SlotRole {
        #[serde(alias = "Machine")]
        Machine,
        #[serde(alias = "PlayerInventory")]
        PlayerInventory,
        #[serde(alias = "Hotbar")]
        Hotbar,
        #[serde(alias = "ScrollableInventory")]
        ScrollableInventory,
        #[serde(alias = "VirtualStorage")]
        VirtualStorage,
        #[serde(alias = "Upgrade")]
        Upgrade,
        #[serde(alias = "UpgradeSettings")]
        UpgradeSettings,
        #[serde(alias = "Filter")]
        Filter,
        #[serde(alias = "Ghost")]
        Ghost,
        #[serde(alias = "Offhand")]
        Offhand,
    }
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum SemanticGroupKind {
        FixedSlots,
        VirtualSlotGrid,
        PlayerInventory,
        Hotbar,
        UpgradeSlots,
        UpgradePanel,
        SearchField,
        ControlButtons,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticGroup {
    pub id: String,
    pub kind: SemanticGroupKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_rows: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_rows: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub member_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_binding: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dynamic_height: bool,
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    #[serde(rename_all = "snake_case")]
    pub enum CodegenMode {
        #[default]
        Simple,
        Modular,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectExportSettings {
    #[serde(default)]
    pub codegen_mode: CodegenMode,
    #[serde(default = "default_true")]
    pub generate_runtime_helpers: bool,
    #[serde(default)]
    pub generate_semantic_registry: bool,
}

impl Default for ProjectExportSettings {
    fn default() -> Self {
        Self {
            codegen_mode: CodegenMode::Simple,
            generate_runtime_helpers: true,
            generate_semantic_registry: false,
        }
    }
}

impl ProjectExportSettings {
    pub fn normalized(self) -> Self {
        self
    }
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum FillDirection {
        #[serde(alias = "LeftToRight")]
        LeftToRight,
        #[serde(alias = "RightToLeft")]
        RightToLeft,
        #[serde(alias = "BottomToTop")]
        BottomToTop,
        #[serde(alias = "TopToBottom")]
        TopToBottom,
    }
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    #[serde(rename_all = "snake_case")]
    pub enum Layer {
        #[default]
        Background,
        Overlay,
        Animatable,
    }
}

fn is_default_layer(layer: &Layer) -> bool {
    *layer == Layer::Background
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UvRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

iterable_enum! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum NineSliceMode {
        Tile,
        Stretch,
    }
}

fn default_nine_slice_mode() -> NineSliceMode {
    NineSliceMode::Tile
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NineSlice {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
    #[serde(default = "default_nine_slice_mode")]
    pub edge_mode: NineSliceMode,
    #[serde(default = "default_nine_slice_mode")]
    pub center_mode: NineSliceMode,
}

iterable_enum! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
    #[serde(rename_all = "snake_case")]
    pub enum TextureRenderMode {
        #[default]
        Plain,
        NineSlice,
    }
}

fn is_plain_render_mode(mode: &TextureRenderMode) -> bool {
    *mode == TextureRenderMode::Plain
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nine_slice: Option<NineSlice>,
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum AttachedRegionAnchor {
        #[serde(alias = "Left")]
        Left,
        #[serde(alias = "Right")]
        Right,
        #[serde(alias = "Top")]
        Top,
        #[serde(alias = "Bottom")]
        Bottom,
        #[serde(alias = "Free")]
        Free,
    }
}

iterable_enum! {
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum AttachedRegionState {
        #[serde(alias = "Static")]
        Static,
        #[serde(alias = "Toggleable")]
        Toggleable,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttachedRegion {
    pub id: String,
    pub anchor: AttachedRegionAnchor,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub state: AttachedRegionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_group: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_owned: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectState {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub initial: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_role: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectStateOverrides {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub elements: HashMap<String, ElementStateOverride>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub groups: HashMap<String, GroupStateOverride>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub attached_regions: HashMap<String, AttachedRegionStateOverride>,
}

impl ProjectStateOverrides {
    fn is_empty(&self) -> bool {
        self.elements.is_empty() && self.groups.is_empty() && self.attached_regions.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ElementStateOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_region: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
}

impl ElementStateOverride {
    fn is_empty(&self) -> bool {
        self.visible.is_none()
            && self.x.is_none()
            && self.y.is_none()
            && self.width.is_none()
            && self.height.is_none()
            && self.attached_region.is_none()
            && self.layer.is_none()
    }

    fn apply_patch(&mut self, patch: ElementStateOverridePatch) -> bool {
        let before = self.clone();
        if let Some(value) = patch.visible {
            self.visible = value;
        }
        if let Some(value) = patch.x {
            self.x = value;
        }
        if let Some(value) = patch.y {
            self.y = value;
        }
        if let Some(value) = patch.width {
            self.width = value;
        }
        if let Some(value) = patch.height {
            self.height = value;
        }
        if let Some(value) = patch.attached_region {
            self.attached_region = value;
        }
        if let Some(value) = patch.layer {
            self.layer = value;
        }
        *self != before
    }

    fn clear_field(&mut self, field: &str) -> Result<bool, String> {
        let before = self.clone();
        match field {
            "visible" => self.visible = None,
            "x" => self.x = None,
            "y" => self.y = None,
            "width" => self.width = None,
            "height" => self.height = None,
            "attached_region" => self.attached_region = None,
            "layer" => self.layer = None,
            _ => return Err(format!("unknown state override field '{field}'")),
        }
        Ok(*self != before)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AttachedRegionStateOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

impl AttachedRegionStateOverride {
    fn is_empty(&self) -> bool {
        self.visible.is_none()
            && self.x.is_none()
            && self.y.is_none()
            && self.width.is_none()
            && self.height.is_none()
    }

    fn apply_patch(&mut self, patch: AttachedRegionStateOverridePatch) -> bool {
        let before = self.clone();
        if let Some(value) = patch.visible {
            self.visible = value;
        }
        if let Some(value) = patch.x {
            self.x = value;
        }
        if let Some(value) = patch.y {
            self.y = value;
        }
        if let Some(value) = patch.width {
            self.width = value;
        }
        if let Some(value) = patch.height {
            self.height = value;
        }
        *self != before
    }

    fn clear_field(&mut self, field: &str) -> Result<bool, String> {
        let before = self.clone();
        match field {
            "visible" => self.visible = None,
            "x" => self.x = None,
            "y" => self.y = None,
            "width" => self.width = None,
            "height" => self.height = None,
            _ => return Err(format!("unknown state override field '{field}'")),
        }
        Ok(*self != before)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GroupStateOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

impl GroupStateOverride {
    fn is_empty(&self) -> bool {
        self.visible.is_none()
    }

    fn clear_field(&mut self, field: &str) -> Result<bool, String> {
        let before = self.clone();
        match field {
            "visible" => self.visible = None,
            _ => return Err(format!("unknown state override field '{field}'")),
        }
        Ok(*self != before)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ElementStateOverridePatch {
    pub visible: Option<Option<bool>>,
    pub x: Option<Option<i32>>,
    pub y: Option<Option<i32>>,
    pub width: Option<Option<u32>>,
    pub height: Option<Option<u32>>,
    pub attached_region: Option<Option<Option<String>>>,
    pub layer: Option<Option<Layer>>,
}

#[derive(Debug, Clone, Default)]
pub struct AttachedRegionStateOverridePatch {
    pub visible: Option<Option<bool>>,
    pub x: Option<Option<i32>>,
    pub y: Option<Option<i32>>,
    pub width: Option<Option<u32>>,
    pub height: Option<Option<u32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateOverrideTarget {
    Element(String),
    AttachedRegion(String),
    Group(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GlyphInfo {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub ascent: i32,
    #[serde(default)]
    pub advance: u32,
    #[serde(default)]
    pub bearing_x: i32,
    #[serde(default)]
    pub bearing_y: i32,
}

pub type GlyphMap = HashMap<char, GlyphInfo>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BitmapProvider {
    pub file: String,
    pub ascent: i32,
    pub chars: Vec<String>,
    #[serde(skip)]
    pub image_data: Vec<u8>,
    #[serde(skip)]
    pub image_width: u32,
    #[serde(skip)]
    pub image_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum FontSource {
    #[serde(rename = "minecraft")]
    Minecraft {
        providers: Vec<BitmapProvider>,
        glyph_map: GlyphMap,
    },
    #[serde(rename = "ttf")]
    Ttf {
        #[serde(default)]
        atlas_png: Vec<u8>,
        font_size: u32,
        glyph_map: GlyphMap,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FontAsset {
    pub id: String,
    pub source: FontSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Element {
    pub id: String,
    #[serde(rename = "type")]
    pub element_type: ElementType,
    pub x: i32,
    pub y: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_uv: Option<UvRect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<FillDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uv: Option<UvRect>,
    #[serde(default, skip_serializing_if = "is_plain_render_mode")]
    pub render_mode: TextureRenderMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nine_slice: Option<NineSlice>,
    #[serde(default, skip_serializing_if = "is_default_layer")]
    pub layer: Layer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_role: Option<SlotRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_min: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_max: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_rows: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_rows: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dock: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_region: Option<String>,
}

impl Element {
    pub fn render_size(&self) -> Size {
        let (default_width, default_height) = match self.element_type {
            ElementType::Slot | ElementType::VirtualSlotCell => (18, 18),
            ElementType::Button | ElementType::ToggleButton => (20, 20),
            ElementType::Scrollbar => (12, 54),
            // Texture compositing uses the source PNG dimensions when no explicit size is
            // provided, but visual bounds cannot decode project texture_data without I/O.
            ElementType::Texture => (16, 16),
            _ => (16, 16),
        };

        Size {
            width: self.width.or(self.size).unwrap_or(default_width),
            height: self.height.or(self.size).unwrap_or(default_height),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub elements: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_owned: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub name: String,
    pub gui_size: Size,
    pub mod_target: ModTarget,
    pub elements: Vec<Element>,
    pub groups: Vec<Group>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<ProjectState>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub state_overrides: HashMap<String, ProjectStateOverrides>,
    pub animations: Vec<crate::animation::Animation>,
    pub assets: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub asset_metadata: HashMap<String, AssetMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_groups: Vec<SemanticGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attached_regions: Vec<AttachedRegion>,
    #[serde(default)]
    pub export_settings: ProjectExportSettings,
    #[serde(skip)]
    pub project_path: Option<String>,
    #[serde(skip)]
    pub is_dirty: bool,
    #[serde(skip)]
    pub texture_data: HashMap<String, Vec<u8>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSession {
    pub id: String,
    pub project: Project,
    pub revision: u64,
    #[serde(skip)]
    pub undo_stack: Vec<Project>,
    #[serde(skip)]
    pub redo_stack: Vec<Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSessionSummary {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    pub active: bool,
    pub is_dirty: bool,
    pub revision: u64,
    pub element_count: usize,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Default)]
pub struct ProjectSessionManager {
    projects: Vec<ProjectSession>,
    active_project_id: Option<String>,
}

impl ProjectSessionManager {
    pub fn create_session(&mut self, project: Project) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.projects.push(ProjectSession {
            id: id.clone(),
            project,
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        });
        self.active_project_id = Some(id.clone());
        id
    }

    pub fn close_session(&mut self, project_id: &str) -> Result<ProjectSessionSummary, String> {
        let index = self
            .projects
            .iter()
            .position(|session| session.id == project_id)
            .ok_or("Project session not found")?;
        let was_active = self.active_project_id.as_deref() == Some(project_id);
        let session = self.projects.remove(index);

        if was_active {
            self.active_project_id = self.projects.last().map(|session| session.id.clone());
        }

        Ok(self.summary_for_session(&session))
    }

    pub fn set_active(&mut self, project_id: &str) -> Result<ProjectSessionSummary, String> {
        if !self.projects.iter().any(|session| session.id == project_id) {
            return Err("Project session not found".to_string());
        }
        self.active_project_id = Some(project_id.to_string());
        self.resolve(Some(project_id))
            .map(|session| self.summary_for(session))
    }

    pub fn active_session(&self) -> Result<&ProjectSession, String> {
        self.resolve(None)
    }

    pub fn list_sessions(&self) -> Vec<ProjectSessionSummary> {
        self.projects
            .iter()
            .map(|session| self.summary_for(session))
            .collect()
    }

    pub fn resolve(&self, project_id: Option<&str>) -> Result<&ProjectSession, String> {
        let id = self.resolve_id(project_id)?;
        self.projects
            .iter()
            .find(|session| session.id == id)
            .ok_or("Project session not found".to_string())
    }

    pub fn resolve_mut(&mut self, project_id: Option<&str>) -> Result<&mut ProjectSession, String> {
        let id = self.resolve_id(project_id)?;
        self.projects
            .iter_mut()
            .find(|session| session.id == id)
            .ok_or("Project session not found".to_string())
    }

    pub fn record_history(&mut self, project_id: Option<&str>) -> Result<(), String> {
        let session = self.resolve_mut(project_id)?;
        session.undo_stack.push(session.project.clone());
        session.redo_stack.clear();
        Ok(())
    }

    pub fn mark_changed(
        &mut self,
        project_id: Option<&str>,
    ) -> Result<ProjectSessionSummary, String> {
        let active_id = self.active_project_id.clone();
        let session = self.resolve_mut(project_id)?;
        session.revision += 1;
        session.project.is_dirty = true;
        Ok(summary_for_session_with_active(
            session,
            active_id.as_deref(),
        ))
    }

    pub fn undo(&mut self, project_id: Option<&str>) -> Result<ProjectSessionSummary, String> {
        let active_id = self.active_project_id.clone();
        let session = self.resolve_mut(project_id)?;
        let previous = session.undo_stack.pop().ok_or("Nothing to undo")?;
        let current = std::mem::replace(&mut session.project, previous);
        session.redo_stack.push(current);
        session.revision += 1;
        session.project.is_dirty = true;
        Ok(summary_for_session_with_active(
            session,
            active_id.as_deref(),
        ))
    }

    pub fn redo(&mut self, project_id: Option<&str>) -> Result<ProjectSessionSummary, String> {
        let active_id = self.active_project_id.clone();
        let session = self.resolve_mut(project_id)?;
        let next = session.redo_stack.pop().ok_or("Nothing to redo")?;
        let current = std::mem::replace(&mut session.project, next);
        session.undo_stack.push(current);
        session.revision += 1;
        session.project.is_dirty = true;
        Ok(summary_for_session_with_active(
            session,
            active_id.as_deref(),
        ))
    }

    fn resolve_id(&self, project_id: Option<&str>) -> Result<String, String> {
        if let Some(id) = project_id {
            return Ok(id.to_string());
        }

        self.active_project_id
            .clone()
            .ok_or("No project open".to_string())
    }

    fn summary_for(&self, session: &ProjectSession) -> ProjectSessionSummary {
        summary_for_session_with_active(session, self.active_project_id.as_deref())
    }

    fn summary_for_session(&self, session: &ProjectSession) -> ProjectSessionSummary {
        self.summary_for(session)
    }
}

fn summary_for_session_with_active(
    session: &ProjectSession,
    active_project_id: Option<&str>,
) -> ProjectSessionSummary {
    ProjectSessionSummary {
        id: session.id.clone(),
        name: session.project.name.clone(),
        path: session.project.project_path.clone(),
        active: active_project_id == Some(session.id.as_str()),
        is_dirty: session.project.is_dirty,
        revision: session.revision,
        element_count: session.project.elements.len(),
        can_undo: !session.undo_stack.is_empty(),
        can_redo: !session.redo_stack.is_empty(),
    }
}

impl Project {
    pub fn new(name: &str, width: u32, height: u32, target: ModTarget) -> Self {
        Self {
            name: name.to_string(),
            gui_size: Size { width, height },
            mod_target: target,
            elements: Vec::new(),
            groups: Vec::new(),
            states: Vec::new(),
            state_overrides: HashMap::new(),
            animations: Vec::new(),
            assets: Vec::new(),
            asset_metadata: HashMap::new(),
            semantic_groups: Vec::new(),
            attached_regions: Vec::new(),
            export_settings: ProjectExportSettings::default(),
            project_path: None,
            is_dirty: true,
            texture_data: HashMap::new(),
            fonts: Vec::new(),
        }
    }

    pub fn find_element(&self, id: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }

    pub fn find_element_mut(&mut self, id: &str) -> Option<&mut Element> {
        self.elements.iter_mut().find(|e| e.id == id)
    }

    pub fn find_attached_region(&self, id: &str) -> Option<&AttachedRegion> {
        self.attached_regions.iter().find(|region| region.id == id)
    }

    pub fn find_attached_region_mut(&mut self, id: &str) -> Option<&mut AttachedRegion> {
        self.attached_regions
            .iter_mut()
            .find(|region| region.id == id)
    }

    pub fn find_state(&self, id: &str) -> Option<&ProjectState> {
        self.states.iter().find(|state| state.id == id)
    }

    pub fn find_state_mut(&mut self, id: &str) -> Option<&mut ProjectState> {
        self.states.iter_mut().find(|state| state.id == id)
    }

    pub fn initial_state_id(&self) -> Option<&str> {
        self.states
            .iter()
            .find(|state| state.initial)
            .or_else(|| self.states.first())
            .map(|state| state.id.as_str())
    }

    pub fn validate_state_id_available(&self, id: &str) -> Result<(), String> {
        if id.trim().is_empty() {
            return Err("state id cannot be empty".into());
        }
        if self.states.iter().any(|state| state.id == id) {
            return Err(format!("state id '{id}' already exists"));
        }
        Ok(())
    }

    pub fn effective_for_state(&self, state_id: Option<&str>) -> Result<Project, String> {
        let Some(state_id) = state_id else {
            return Ok(self.clone());
        };
        if self.find_state(state_id).is_none() {
            return Err(format!("unknown state '{state_id}'"));
        }

        let mut effective = self.clone();
        let Some(overrides) = self.state_overrides.get(state_id) else {
            return Ok(effective);
        };

        for (element_id, override_value) in &overrides.elements {
            let Some(element) = effective.find_element_mut(element_id) else {
                continue;
            };
            if let Some(value) = override_value.visible {
                element.visible = value;
            }
            if let Some(value) = override_value.x {
                element.x = value;
            }
            if let Some(value) = override_value.y {
                element.y = value;
            }
            if let Some(value) = override_value.width {
                element.width = Some(value);
            }
            if let Some(value) = override_value.height {
                element.height = Some(value);
            }
            if let Some(value) = &override_value.attached_region {
                element.attached_region = value.clone();
            }
            if let Some(value) = override_value.layer.clone() {
                element.layer = value;
            }
        }

        for (region_id, override_value) in &overrides.attached_regions {
            let Some(region) = effective.find_attached_region_mut(region_id) else {
                continue;
            };
            if let Some(value) = override_value.visible {
                region.visible = value;
            }
            if let Some(value) = override_value.x {
                region.x = value;
            }
            if let Some(value) = override_value.y {
                region.y = value;
            }
            if let Some(value) = override_value.width {
                region.width = value;
            }
            if let Some(value) = override_value.height {
                region.height = value;
            }
        }

        Ok(effective)
    }

    pub fn update_element_state_override(
        &mut self,
        state_id: &str,
        element_id: &str,
        patch: ElementStateOverridePatch,
    ) -> Result<bool, String> {
        if self.find_state(state_id).is_none() {
            return Err(format!("unknown state '{state_id}'"));
        }
        if self.find_element(element_id).is_none() {
            return Err(format!("unknown element '{element_id}'"));
        }

        let before = self.state_overrides.get(state_id).cloned();
        let override_value = self
            .state_overrides
            .entry(state_id.to_string())
            .or_default()
            .elements
            .entry(element_id.to_string())
            .or_default();
        let changed = override_value.apply_patch(patch);
        if !changed {
            self.restore_state_overrides(state_id, before);
            return Ok(false);
        }

        self.prune_state_override_target(
            state_id,
            StateOverrideTarget::Element(element_id.to_string()),
        );
        self.is_dirty = true;
        Ok(true)
    }

    pub fn update_attached_region_state_override(
        &mut self,
        state_id: &str,
        region_id: &str,
        patch: AttachedRegionStateOverridePatch,
    ) -> Result<bool, String> {
        if self.find_state(state_id).is_none() {
            return Err(format!("unknown state '{state_id}'"));
        }
        if self.find_attached_region(region_id).is_none() {
            return Err(format!("unknown attached region '{region_id}'"));
        }

        let before = self.state_overrides.get(state_id).cloned();
        let override_value = self
            .state_overrides
            .entry(state_id.to_string())
            .or_default()
            .attached_regions
            .entry(region_id.to_string())
            .or_default();
        let changed = override_value.apply_patch(patch);
        if !changed {
            self.restore_state_overrides(state_id, before);
            return Ok(false);
        }

        self.prune_state_override_target(
            state_id,
            StateOverrideTarget::AttachedRegion(region_id.to_string()),
        );
        self.is_dirty = true;
        Ok(true)
    }

    pub fn clear_state_override_field(
        &mut self,
        state_id: &str,
        target: StateOverrideTarget,
        field: &str,
    ) -> Result<bool, String> {
        if self.find_state(state_id).is_none() {
            return Err(format!("unknown state '{state_id}'"));
        }

        let changed = match &target {
            StateOverrideTarget::Element(element_id) => {
                if self.find_element(element_id).is_none() {
                    return Err(format!("unknown element '{element_id}'"));
                }
                if let Some(override_value) = self
                    .state_overrides
                    .get_mut(state_id)
                    .and_then(|overrides| overrides.elements.get_mut(element_id))
                {
                    override_value.clear_field(field)?
                } else {
                    ElementStateOverride::default().clear_field(field)?
                }
            }
            StateOverrideTarget::AttachedRegion(region_id) => {
                if self.find_attached_region(region_id).is_none() {
                    return Err(format!("unknown attached region '{region_id}'"));
                }
                if let Some(override_value) = self
                    .state_overrides
                    .get_mut(state_id)
                    .and_then(|overrides| overrides.attached_regions.get_mut(region_id))
                {
                    override_value.clear_field(field)?
                } else {
                    AttachedRegionStateOverride::default().clear_field(field)?
                }
            }
            StateOverrideTarget::Group(group_id) => {
                if !self.groups.iter().any(|group| group.id == *group_id) {
                    return Err(format!("unknown group '{group_id}'"));
                }
                if let Some(override_value) = self
                    .state_overrides
                    .get_mut(state_id)
                    .and_then(|overrides| overrides.groups.get_mut(group_id))
                {
                    override_value.clear_field(field)?
                } else {
                    GroupStateOverride::default().clear_field(field)?
                }
            }
        };

        if changed {
            self.prune_state_override_target(state_id, target);
            self.is_dirty = true;
        }
        Ok(changed)
    }

    fn prune_state_override_target(&mut self, state_id: &str, target: StateOverrideTarget) {
        let Some(overrides) = self.state_overrides.get_mut(state_id) else {
            return;
        };

        match target {
            StateOverrideTarget::Element(element_id) => {
                if overrides
                    .elements
                    .get(&element_id)
                    .is_some_and(ElementStateOverride::is_empty)
                {
                    overrides.elements.remove(&element_id);
                }
            }
            StateOverrideTarget::AttachedRegion(region_id) => {
                if overrides
                    .attached_regions
                    .get(&region_id)
                    .is_some_and(AttachedRegionStateOverride::is_empty)
                {
                    overrides.attached_regions.remove(&region_id);
                }
            }
            StateOverrideTarget::Group(group_id) => {
                if overrides
                    .groups
                    .get(&group_id)
                    .is_some_and(GroupStateOverride::is_empty)
                {
                    overrides.groups.remove(&group_id);
                }
            }
        }

        if overrides.is_empty() {
            self.state_overrides.remove(state_id);
        }
    }

    fn restore_state_overrides(
        &mut self,
        state_id: &str,
        overrides: Option<ProjectStateOverrides>,
    ) {
        if let Some(overrides) = overrides {
            self.state_overrides.insert(state_id.to_string(), overrides);
        } else {
            self.state_overrides.remove(state_id);
        }
    }

    pub fn visual_bounds(&self) -> VisualBounds {
        let mut min_x = 0_i64;
        let mut min_y = 0_i64;
        let mut max_x = i64::from(self.gui_size.width);
        let mut max_y = i64::from(self.gui_size.height);

        for element in self.elements.iter().filter(|element| element.visible) {
            let x = i64::from(element.x);
            let y = i64::from(element.y);
            let size = self.element_visual_size(element);
            let width = i64::from(size.width);
            let height = i64::from(size.height);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + width);
            max_y = max_y.max(y + height);
        }

        for region in self.attached_regions.iter().filter(|region| region.visible) {
            let x = i64::from(region.x);
            let y = i64::from(region.y);
            let width = i64::from(region.width);
            let height = i64::from(region.height);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + width);
            max_y = max_y.max(y + height);
        }

        VisualBounds {
            x: min_x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            y: min_y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            width: u32::try_from((max_x - min_x).max(1)).unwrap_or(u32::MAX),
            height: u32::try_from((max_y - min_y).max(1)).unwrap_or(u32::MAX),
        }
    }

    fn element_visual_size(&self, element: &Element) -> Size {
        if element.element_type != ElementType::Texture {
            return element.render_size();
        }

        let Some(asset_name) = element.asset.as_deref() else {
            return element.render_size();
        };
        let Some(data) = self.texture_data.get(asset_name) else {
            return element.render_size();
        };
        let Ok(texture) = image::load_from_memory(data) else {
            return element.render_size();
        };

        Size {
            width: element.width.or(element.size).unwrap_or(texture.width()),
            height: element.height.or(element.size).unwrap_or(texture.height()),
        }
    }

    pub fn remove_element(&mut self, id: &str) -> Option<Element> {
        if let Some(pos) = self.elements.iter().position(|e| e.id == id) {
            self.is_dirty = true;
            let removed = self.elements.remove(pos);
            for group in &mut self.groups {
                group.elements.retain(|element_id| element_id != id);
            }
            self.groups.retain(|group| group.elements.len() >= 2);
            Some(removed)
        } else {
            None
        }
    }

    pub fn add_element(&mut self, element: Element) {
        self.is_dirty = true;
        self.elements.push(element);
    }

    pub fn group_elements(
        &mut self,
        group_id: String,
        element_ids: Vec<String>,
    ) -> Result<Group, String> {
        if self.groups.iter().any(|group| group.id == group_id) {
            return Err("Group already exists".to_string());
        }

        let mut unique_ids = Vec::new();
        for id in element_ids {
            if !unique_ids.iter().any(|existing| existing == &id) {
                unique_ids.push(id);
            }
        }
        if unique_ids.len() < 2 {
            return Err("At least two elements are required to create a group".to_string());
        }

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        for id in &unique_ids {
            let element = self
                .find_element(id)
                .ok_or_else(|| format!("Element not found: {id}"))?;
            min_x = min_x.min(element.x);
            min_y = min_y.min(element.y);
        }

        for group in &mut self.groups {
            group
                .elements
                .retain(|element_id| !unique_ids.iter().any(|id| id == element_id));
        }
        self.groups.retain(|group| group.elements.len() >= 2);

        let group = Group {
            id: group_id,
            x: min_x,
            y: min_y,
            elements: unique_ids,
            state_owned: Vec::new(),
        };
        self.groups.push(group.clone());
        self.is_dirty = true;
        Ok(group)
    }

    pub fn ungroup(&mut self, group_id: &str) -> bool {
        let old_len = self.groups.len();
        self.groups.retain(|group| group.id != group_id);
        let removed = self.groups.len() != old_len;
        if removed {
            self.is_dirty = true;
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn semantic_group_member_ids_round_trip_with_default_empty() {
        let json = serde_json::json!({
            "id": "controls",
            "kind": "control_buttons",
            "member_ids": ["settings_button", "lock_button"],
            "data_source": "pouch_settings"
        });

        let group: SemanticGroup = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(group.member_ids, vec!["settings_button", "lock_button"]);
        assert_eq!(
            serde_json::to_value(&group).unwrap()["member_ids"],
            json["member_ids"]
        );

        let without_members: SemanticGroup = serde_json::from_value(serde_json::json!({
            "id": "inventory",
            "kind": "player_inventory"
        }))
        .unwrap();
        assert!(without_members.member_ids.is_empty());
        assert!(serde_json::to_value(&without_members)
            .unwrap()
            .get("member_ids")
            .is_none());
    }

    fn sample_element_defaults() -> Element {
        Element {
            id: String::new(),
            element_type: ElementType::Slot,
            x: 0,
            y: 0,
            width: None,
            height: None,
            size: None,
            asset: None,
            icon: None,
            icon_uv: None,
            tooltip: None,
            direction: None,
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: None,
            visible: true,
            uv: None,
            render_mode: crate::project::TextureRenderMode::Plain,
            nine_slice: None,
            layer: Layer::Background,
            slot_role: None,
            slot_index: None,
            inventory_group: None,
            scroll_binding: None,
            scroll_min: None,
            scroll_max: None,
            visible_rows: None,
            total_rows: None,
            columns: None,
            target_group: None,
            binding: None,
            dock: None,
            open_width: None,
            open_height: None,
            attached_region: None,
        }
    }

    fn sample_element(id: &str) -> Element {
        Element {
            id: id.to_string(),
            direction: Some(FillDirection::LeftToRight),
            size: Some(18),
            uv: Some(UvRect {
                x: 1,
                y: 2,
                width: 16,
                height: 16,
            }),
            ..sample_element_defaults()
        }
    }

    fn base_element_for_test(id: &str, element_type: ElementType, x: i32, y: i32) -> Element {
        Element {
            id: id.to_string(),
            element_type,
            x,
            y,
            ..sample_element_defaults()
        }
    }

    fn test_attached_region(id: &str, x: i32, y: i32, width: u32, height: u32) -> AttachedRegion {
        AttachedRegion {
            id: id.to_string(),
            anchor: AttachedRegionAnchor::Right,
            x,
            y,
            width,
            height,
            state: AttachedRegionState::Static,
            kind: None,
            semantic_group: None,
            visible: true,
            state_owned: Vec::new(),
        }
    }

    #[test]
    fn project_round_trips_state_definitions_and_overrides() {
        let mut project = Project::new("State Variants", 176, 166, ModTarget::Forge);
        project.states.push(ProjectState {
            id: "collapsed".into(),
            label: "Collapsed".into(),
            description: Some("Base pouch layout".into()),
            initial: true,
            export_role: Some("collapsed".into()),
        });
        project.states.push(ProjectState {
            id: "expanded".into(),
            label: "Expanded".into(),
            description: Some("Drawer visible".into()),
            initial: false,
            export_role: Some("expanded".into()),
        });

        let mut overrides = ProjectStateOverrides::default();
        overrides.elements.insert(
            "settings_panel".into(),
            ElementStateOverride {
                visible: Some(true),
                x: Some(176),
                y: Some(0),
                width: Some(88),
                height: Some(166),
                attached_region: Some(Some("settings_drawer".into())),
                layer: Some(Layer::Overlay),
            },
        );
        overrides.attached_regions.insert(
            "settings_drawer".into(),
            AttachedRegionStateOverride {
                visible: Some(true),
                x: Some(176),
                y: Some(0),
                width: Some(88),
                height: Some(166),
            },
        );
        project.state_overrides.insert("expanded".into(), overrides);

        let value = serde_json::to_value(&project).unwrap();
        assert_eq!(value["states"][0]["id"], "collapsed");
        assert_eq!(
            value["state_overrides"]["expanded"]["elements"]["settings_panel"]["layer"],
            "overlay"
        );

        let loaded: Project = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.states.len(), 2);
        assert_eq!(
            loaded.state_overrides["expanded"].elements["settings_panel"].x,
            Some(176)
        );
    }

    #[test]
    fn effective_layout_applies_state_overrides_without_mutating_base() {
        let mut project = Project::new("State Variants", 176, 166, ModTarget::Forge);
        project.elements.push(base_element_for_test(
            "settings_panel",
            ElementType::Texture,
            0,
            0,
        ));
        project
            .attached_regions
            .push(test_attached_region("settings_drawer", 176, 0, 88, 166));
        project.states.push(ProjectState {
            id: "expanded".into(),
            label: "Expanded".into(),
            description: None,
            initial: true,
            export_role: Some("expanded".into()),
        });

        let mut overrides = ProjectStateOverrides::default();
        overrides.elements.insert(
            "settings_panel".into(),
            ElementStateOverride {
                visible: Some(true),
                x: Some(176),
                y: Some(8),
                width: None,
                height: None,
                attached_region: Some(Some("settings_drawer".into())),
                layer: Some(Layer::Overlay),
            },
        );
        overrides.attached_regions.insert(
            "settings_drawer".into(),
            AttachedRegionStateOverride {
                visible: Some(true),
                x: None,
                y: Some(8),
                width: None,
                height: None,
            },
        );
        project.state_overrides.insert("expanded".into(), overrides);

        let effective = project.effective_for_state(Some("expanded")).unwrap();
        let effective_element = effective.find_element("settings_panel").unwrap();
        assert_eq!(effective_element.x, 176);
        assert_eq!(effective_element.y, 8);
        assert_eq!(
            effective_element.attached_region.as_deref(),
            Some("settings_drawer")
        );
        assert_eq!(
            effective.find_attached_region("settings_drawer").unwrap().y,
            8
        );

        let base_element = project.find_element("settings_panel").unwrap();
        assert_eq!(base_element.x, 0);
        assert_eq!(base_element.y, 0);
        assert_eq!(base_element.attached_region, None);
    }

    #[test]
    fn clearing_state_override_field_restores_inherited_base_value() {
        let mut project = Project::new("State Variants", 176, 166, ModTarget::Forge);
        project
            .elements
            .push(base_element_for_test("panel", ElementType::Texture, 4, 6));
        project.states.push(ProjectState {
            id: "expanded".into(),
            label: "Expanded".into(),
            description: None,
            initial: true,
            export_role: None,
        });

        project
            .update_element_state_override(
                "expanded",
                "panel",
                ElementStateOverridePatch {
                    x: Some(Some(48)),
                    y: Some(Some(64)),
                    ..ElementStateOverridePatch::default()
                },
            )
            .unwrap();
        assert_eq!(
            project
                .effective_for_state(Some("expanded"))
                .unwrap()
                .find_element("panel")
                .unwrap()
                .x,
            48
        );

        project
            .clear_state_override_field(
                "expanded",
                StateOverrideTarget::Element("panel".into()),
                "x",
            )
            .unwrap();
        let effective = project.effective_for_state(Some("expanded")).unwrap();
        assert_eq!(effective.find_element("panel").unwrap().x, 4);
        assert_eq!(effective.find_element("panel").unwrap().y, 64);
    }

    #[test]
    fn no_op_state_override_updates_do_not_dirty_project_or_create_overrides() {
        let mut project = Project::new("State Variants", 176, 166, ModTarget::Forge);
        project
            .elements
            .push(base_element_for_test("panel", ElementType::Texture, 4, 6));
        project
            .attached_regions
            .push(test_attached_region("drawer", 176, 0, 88, 166));
        project.states.push(ProjectState {
            id: "expanded".into(),
            label: "Expanded".into(),
            description: None,
            initial: true,
            export_role: None,
        });
        project.is_dirty = false;

        let changed = project
            .update_element_state_override(
                "expanded",
                "panel",
                ElementStateOverridePatch::default(),
            )
            .unwrap();
        assert!(!changed);
        assert!(!project.is_dirty);
        assert!(project.state_overrides.is_empty());

        let changed = project
            .update_attached_region_state_override(
                "expanded",
                "drawer",
                AttachedRegionStateOverridePatch::default(),
            )
            .unwrap();
        assert!(!changed);
        assert!(!project.is_dirty);
        assert!(project.state_overrides.is_empty());

        assert!(project
            .update_element_state_override(
                "expanded",
                "panel",
                ElementStateOverridePatch {
                    x: Some(Some(48)),
                    ..ElementStateOverridePatch::default()
                },
            )
            .unwrap());
        project.is_dirty = false;

        let changed = project
            .update_element_state_override(
                "expanded",
                "panel",
                ElementStateOverridePatch {
                    x: Some(Some(48)),
                    ..ElementStateOverridePatch::default()
                },
            )
            .unwrap();
        assert!(!changed);
        assert!(!project.is_dirty);
        assert_eq!(
            project.state_overrides["expanded"].elements["panel"].x,
            Some(48)
        );
    }

    #[test]
    fn clearing_missing_or_empty_state_override_fields_is_not_dirty() {
        let mut project = Project::new("State Variants", 176, 166, ModTarget::Forge);
        project
            .elements
            .push(base_element_for_test("panel", ElementType::Texture, 4, 6));
        project.states.push(ProjectState {
            id: "expanded".into(),
            label: "Expanded".into(),
            description: None,
            initial: true,
            export_role: None,
        });
        project.is_dirty = false;

        let changed = project
            .clear_state_override_field(
                "expanded",
                StateOverrideTarget::Element("panel".into()),
                "x",
            )
            .unwrap();
        assert!(!changed);
        assert!(!project.is_dirty);
        assert!(project.state_overrides.is_empty());

        let mut overrides = ProjectStateOverrides::default();
        overrides
            .elements
            .insert("panel".into(), ElementStateOverride::default());
        project.state_overrides.insert("expanded".into(), overrides);
        project.is_dirty = false;

        let changed = project
            .clear_state_override_field(
                "expanded",
                StateOverrideTarget::Element("panel".into()),
                "x",
            )
            .unwrap();
        assert!(!changed);
        assert!(!project.is_dirty);
        assert!(project.state_overrides["expanded"]
            .elements
            .contains_key("panel"));
    }

    fn test_png(width: u32, height: u32, color: Rgba<u8>) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, color);
        let mut bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    #[test]
    fn element_button_icon_tooltip_fields_round_trip() {
        let json = serde_json::json!({
            "id": "settings_button",
            "type": "button",
            "x": 12,
            "y": 18,
            "width": 20,
            "height": 20,
            "asset": "textures/generated/button.png",
            "icon": "textures/gui/widgets.png",
            "icon_uv": { "x": 16, "y": 0, "width": 16, "height": 16 },
            "tooltip": "Open settings",
            "content": "Settings"
        });

        let element: Element = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(element.icon.as_deref(), Some("textures/gui/widgets.png"));
        assert_eq!(element.icon_uv.as_ref().unwrap().x, 16);
        assert_eq!(element.tooltip.as_deref(), Some("Open settings"));

        let serialized = serde_json::to_value(element).unwrap();
        assert_eq!(serialized["icon"], "textures/gui/widgets.png");
        assert_eq!(serialized["icon_uv"]["width"], 16);
        assert_eq!(serialized["tooltip"], "Open settings");
    }

    #[test]
    fn enums_serialize_with_frontend_casing() {
        let element = sample_element("slot_1");
        let value = serde_json::to_value(&element).unwrap();

        assert_eq!(value["type"], "slot");
        assert_eq!(value["direction"], "left_to_right");
        assert_eq!(
            serde_json::to_value(ModTarget::NeoForge).unwrap(),
            "neoforge"
        );
        assert_eq!(
            serde_json::to_value(crate::animation::AnimationType::Fill).unwrap(),
            "fill"
        );
    }

    #[test]
    fn element_visible_defaults_to_true_when_missing() {
        let value = serde_json::json!({
            "id": "slot_1",
            "type": "slot",
            "x": 8,
            "y": 18,
            "size": 18
        });

        let element: Element = serde_json::from_value(value).unwrap();

        assert!(element.visible);
    }

    #[test]
    fn element_layer_defaults_to_background_when_missing() {
        let value = serde_json::json!({
            "id": "slot_1",
            "type": "slot",
            "x": 8,
            "y": 18,
            "size": 18
        });
        let element: Element = serde_json::from_value(value).unwrap();
        assert_eq!(element.layer, Layer::Background);
    }

    #[test]
    fn element_layer_serializes_animatable() {
        let element = Element {
            id: "arrow".into(),
            element_type: ElementType::Progress,
            x: 79,
            y: 35,
            layer: Layer::Animatable,
            slot_role: None,
            slot_index: None,
            inventory_group: None,
            scroll_binding: None,
            scroll_min: None,
            scroll_max: None,
            visible_rows: None,
            total_rows: None,
            columns: None,
            target_group: None,
            binding: None,
            dock: None,
            open_width: None,
            open_height: None,
            attached_region: None,
            ..sample_element_defaults()
        };
        let value = serde_json::to_value(&element).unwrap();
        assert_eq!(value["layer"], "animatable");
    }

    #[test]
    fn element_layer_skips_background_default() {
        let element = Element {
            id: "bg".into(),
            element_type: ElementType::Texture,
            x: 0,
            y: 0,
            layer: Layer::Background,
            slot_role: None,
            slot_index: None,
            inventory_group: None,
            scroll_binding: None,
            scroll_min: None,
            scroll_max: None,
            visible_rows: None,
            total_rows: None,
            columns: None,
            target_group: None,
            binding: None,
            dock: None,
            open_width: None,
            open_height: None,
            attached_region: None,
            ..sample_element_defaults()
        };
        let value = serde_json::to_value(&element).unwrap();
        assert!(!value.as_object().unwrap().contains_key("layer"));
    }

    #[test]
    fn font_asset_serialization() {
        let mut glyph_map = GlyphMap::new();
        glyph_map.insert(
            'A',
            GlyphInfo {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
                ascent: 7,
                advance: 9,
                bearing_x: 1,
                bearing_y: 2,
            },
        );

        let font = FontAsset {
            id: "minecraft:default".into(),
            source: FontSource::Ttf {
                atlas_png: vec![1, 2, 3],
                font_size: 16,
                glyph_map: glyph_map.clone(),
            },
        };

        let value = serde_json::to_value(&font).unwrap();
        assert_eq!(value["id"], "minecraft:default");
        assert_eq!(value["source"]["type"], "ttf");
        assert!(value["source"]["atlas_png"].as_array().is_some());
        assert_eq!(value["source"]["font_size"], 16);

        let glyph_map_val = &value["source"]["glyph_map"];
        let glyph = glyph_map_val.get("A").unwrap();
        assert_eq!(glyph["advance"], 9);
        assert_eq!(glyph["bearing_x"], 1);
        assert_eq!(glyph["bearing_y"], 2);
    }

    #[test]
    fn glyph_info_deserializes_legacy_maps_without_metrics() {
        let value = serde_json::json!({
            "x": 1,
            "y": 2,
            "width": 3,
            "height": 4,
            "ascent": 5
        });

        let glyph: GlyphInfo = serde_json::from_value(value).unwrap();

        assert_eq!(glyph.advance, 0);
        assert_eq!(glyph.bearing_x, 0);
        assert_eq!(glyph.bearing_y, 0);
    }

    #[test]
    fn project_fonts_defaults_to_empty() {
        let value = serde_json::json!({
            "name": "Test",
            "gui_size": { "width": 176, "height": 166 },
            "mod_target": "forge",
            "elements": [],
            "groups": [],
            "animations": [],
            "assets": []
        });
        let project: Project = serde_json::from_value(value).unwrap();
        assert!(project.fonts.is_empty());
    }

    #[test]
    fn asset_metadata_round_trips_nine_slice_defaults() {
        let json = serde_json::json!({
            "name": "Meta",
            "gui_size": { "width": 176, "height": 166 },
            "mod_target": "forge",
            "elements": [],
            "groups": [],
            "animations": [],
            "assets": ["textures/gui/panel_atlas.png"],
            "asset_metadata": {
                "textures/gui/panel_atlas.png": {
                    "width": 64,
                    "height": 64,
                    "nine_slice": {
                        "left": 4,
                        "right": 4,
                        "top": 4,
                        "bottom": 4
                    }
                }
            }
        });

        let project: Project = serde_json::from_value(json).unwrap();
        let metadata = project
            .asset_metadata
            .get("textures/gui/panel_atlas.png")
            .unwrap();
        assert_eq!(metadata.width, Some(64));
        assert_eq!(metadata.height, Some(64));
        assert_eq!(metadata.nine_slice.as_ref().unwrap().left, 4);
        assert_eq!(
            metadata.nine_slice.as_ref().unwrap().edge_mode,
            NineSliceMode::Tile
        );
        assert_eq!(
            metadata.nine_slice.as_ref().unwrap().center_mode,
            NineSliceMode::Tile
        );
        let serialized = serde_json::to_value(&project).unwrap();
        assert_eq!(
            serialized["asset_metadata"]["textures/gui/panel_atlas.png"]["nine_slice"]["edge_mode"],
            serde_json::json!("tile")
        );
        assert_eq!(
            serialized["asset_metadata"]["textures/gui/panel_atlas.png"]["nine_slice"]
                ["center_mode"],
            serde_json::json!("tile")
        );
    }

    #[test]
    fn texture_element_round_trips_nine_slice_render_mode() {
        let json = serde_json::json!({
            "id": "background",
            "type": "texture",
            "x": 0,
            "y": 0,
            "width": 176,
            "height": 166,
            "asset": "textures/gui/panel_atlas.png",
            "render_mode": "nine_slice",
            "nine_slice": {
                "left": 4,
                "right": 4,
                "top": 4,
                "bottom": 4,
                "edge_mode": "tile",
                "center_mode": "tile"
            }
        });

        let element: Element = serde_json::from_value(json).unwrap();
        assert_eq!(element.render_mode, TextureRenderMode::NineSlice);
        assert_eq!(
            element.nine_slice.as_ref().unwrap().center_mode,
            NineSliceMode::Tile
        );
        assert_eq!(
            serde_json::to_value(&element).unwrap()["render_mode"],
            serde_json::json!("nine_slice")
        );
    }

    #[test]
    fn project_defaults_missing_semantic_fields() {
        let json = r#"{
            "name": "Legacy",
            "gui_size": { "width": 176, "height": 166 },
            "mod_target": "forge",
            "elements": [],
            "groups": [],
            "animations": [],
            "assets": []
        }"#;

        let project: Project = serde_json::from_str(json).unwrap();
        assert!(project.semantic_groups.is_empty());
        assert_eq!(project.export_settings.codegen_mode, CodegenMode::Simple);
        assert!(project.export_settings.generate_runtime_helpers);
        assert!(!project.export_settings.generate_semantic_registry);
    }

    #[test]
    fn project_defaults_attached_regions_to_empty() {
        let json = r#"{
            "name": "Legacy",
            "gui_size": { "width": 176, "height": 166 },
            "mod_target": "forge",
            "elements": [],
            "groups": [],
            "animations": [],
            "assets": []
        }"#;

        let project: Project = serde_json::from_str(json).unwrap();

        assert!(project.attached_regions.is_empty());
    }

    #[test]
    fn attached_region_and_element_membership_round_trip() {
        let mut project = Project::new("Attached", 100, 200, ModTarget::Forge);
        project.attached_regions.push(AttachedRegion {
            id: "returns_pocket".into(),
            anchor: AttachedRegionAnchor::Right,
            x: 100,
            y: 18,
            width: 54,
            height: 72,
            state: AttachedRegionState::Static,
            kind: Some("returns_pocket".into()),
            semantic_group: Some("food_returns".into()),
            visible: true,
            state_owned: Vec::new(),
        });
        let mut element = base_element_for_test("returns_0", ElementType::Slot, 108, 26);
        element.attached_region = Some("returns_pocket".into());
        project.elements.push(element);

        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.attached_regions.len(), 1);
        assert_eq!(
            loaded.attached_regions[0].anchor,
            AttachedRegionAnchor::Right
        );
        assert_eq!(
            loaded.attached_regions[0].state,
            AttachedRegionState::Static
        );
        assert_eq!(
            loaded.elements[0].attached_region.as_deref(),
            Some("returns_pocket")
        );
    }

    #[test]
    fn visual_bounds_include_main_negative_elements_and_regions() {
        let mut project = Project::new("Visual", 100, 200, ModTarget::Forge);
        let mut flair = base_element_for_test("flair", ElementType::Texture, 84, -16);
        flair.width = Some(32);
        flair.height = Some(32);
        project.elements.push(flair);
        project.attached_regions.push(AttachedRegion {
            id: "side".into(),
            anchor: AttachedRegionAnchor::Right,
            x: 100,
            y: 20,
            width: 44,
            height: 80,
            state: AttachedRegionState::Static,
            kind: Some("side_controls".into()),
            semantic_group: None,
            visible: true,
            state_owned: Vec::new(),
        });

        let bounds = project.visual_bounds();

        assert_eq!(bounds.x, 0);
        assert_eq!(bounds.y, -16);
        assert_eq!(bounds.width, 144);
        assert_eq!(bounds.height, 216);
    }

    #[test]
    fn visual_bounds_ignore_hidden_elements_and_regions() {
        let mut project = Project::new("Hidden Visual", 100, 200, ModTarget::Forge);
        let mut hidden = base_element_for_test("hidden", ElementType::Texture, -40, -40);
        hidden.width = Some(20);
        hidden.height = Some(20);
        hidden.visible = false;
        project.elements.push(hidden);
        project.attached_regions.push(AttachedRegion {
            id: "hidden_region".into(),
            anchor: AttachedRegionAnchor::Left,
            x: -60,
            y: 0,
            width: 20,
            height: 20,
            state: AttachedRegionState::Static,
            kind: None,
            semantic_group: None,
            visible: false,
            state_owned: Vec::new(),
        });

        let bounds = project.visual_bounds();

        assert_eq!(
            bounds,
            VisualBounds {
                x: 0,
                y: 0,
                width: 100,
                height: 200
            }
        );
    }

    #[test]
    fn visual_bounds_clamps_width_when_visible_region_exceeds_u32_extent() {
        let mut project = Project::new("Huge Visual", 1, 1, ModTarget::Forge);
        project.attached_regions.push(AttachedRegion {
            id: "huge_region".into(),
            anchor: AttachedRegionAnchor::Right,
            x: i32::MAX,
            y: 0,
            width: u32::MAX,
            height: 1,
            state: AttachedRegionState::Static,
            kind: None,
            semantic_group: None,
            visible: true,
            state_owned: Vec::new(),
        });

        let bounds = project.visual_bounds();

        assert_eq!(bounds.x, 0);
        assert_eq!(bounds.width, u32::MAX);
        assert_eq!(bounds.height, 1);
    }

    #[test]
    fn visual_bounds_use_render_defaults_for_slot_and_scrollbar() {
        let mut slot_project = Project::new("Slot Visual", 10, 10, ModTarget::Forge);
        slot_project
            .elements
            .push(base_element_for_test("slot", ElementType::Slot, 10, 0));

        let slot_bounds = slot_project.visual_bounds();

        assert_eq!(slot_bounds.width, 28);
        assert_eq!(slot_bounds.height, 18);

        let mut scrollbar_project = Project::new("Scrollbar Visual", 10, 10, ModTarget::Forge);
        scrollbar_project.elements.push(base_element_for_test(
            "scrollbar",
            ElementType::Scrollbar,
            10,
            10,
        ));

        let scrollbar_bounds = scrollbar_project.visual_bounds();

        assert_eq!(scrollbar_bounds.width, 22);
        assert_eq!(scrollbar_bounds.height, 64);
    }

    #[test]
    fn visual_bounds_use_texture_asset_size_when_explicit_size_missing() {
        let mut project = Project::new("Texture Visual", 100, 80, ModTarget::Forge);
        project.texture_data.insert(
            "textures/flair.png".into(),
            test_png(32, 24, Rgba([0xd7, 0xa3, 0x39, 0xff])),
        );
        let mut flair = base_element_for_test("flair", ElementType::Texture, 84, 70);
        flair.asset = Some("textures/flair.png".into());
        project.elements.push(flair);

        let bounds = project.visual_bounds();

        assert_eq!(
            bounds,
            VisualBounds {
                x: 0,
                y: 0,
                width: 116,
                height: 94,
            }
        );

        let atlas = crate::texture::composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.dimensions(), (116, 94));
        assert_eq!(image.get_pixel(115, 93).0, [0xd7, 0xa3, 0x39, 0xff]);
    }

    #[test]
    fn project_export_settings_defaults_missing_codegen_mode() {
        let json = r#"{
            "name": "Partial",
            "gui_size": { "width": 176, "height": 166 },
            "mod_target": "forge",
            "elements": [],
            "groups": [],
            "animations": [],
            "assets": [],
            "export_settings": {
                "generate_semantic_registry": true
            }
        }"#;

        let project: Project = serde_json::from_str(json).unwrap();
        assert_eq!(project.export_settings.codegen_mode, CodegenMode::Simple);
        assert!(project.export_settings.generate_runtime_helpers);
        assert!(project.export_settings.generate_semantic_registry);
    }

    #[test]
    fn element_semantics_round_trip() {
        let element = Element {
            id: "buffer_slot_0".into(),
            element_type: ElementType::Slot,
            x: 34,
            y: 54,
            width: None,
            height: None,
            size: Some(18),
            asset: None,
            icon: None,
            icon_uv: None,
            tooltip: None,
            direction: None,
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: None,
            visible: true,
            uv: None,
            render_mode: crate::project::TextureRenderMode::Plain,
            nine_slice: None,
            layer: Layer::Background,
            slot_role: Some(SlotRole::ScrollableInventory),
            slot_index: Some(0),
            inventory_group: Some("machine_buffer".into()),
            scroll_binding: Some("buffer_scroll".into()),
            scroll_min: None,
            scroll_max: None,
            visible_rows: None,
            total_rows: None,
            columns: None,
            target_group: None,
            binding: None,
            dock: None,
            open_width: None,
            open_height: None,
            attached_region: None,
        };

        let value = serde_json::to_value(&element).unwrap();
        assert_eq!(value["slot_role"], "scrollable_inventory");
        assert_eq!(value["inventory_group"], "machine_buffer");
        let decoded: Element = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, element);
    }

    #[test]
    fn session_manager_creates_lists_switches_and_isolates_projects() {
        let mut manager = ProjectSessionManager::default();
        let first = manager.create_session(Project::new("First", 176, 166, ModTarget::Forge));
        let second = manager.create_session(Project::new("Second", 200, 180, ModTarget::Fabric));

        manager
            .resolve_mut(Some(&first))
            .unwrap()
            .project
            .add_element(sample_element("slot_1"));
        manager.set_active(&second).unwrap();

        let summaries = manager.list_sessions();
        assert_eq!(summaries.len(), 2);
        assert_eq!(manager.active_session().unwrap().id, second);
        assert_eq!(
            manager
                .resolve(Some(&first))
                .unwrap()
                .project
                .elements
                .len(),
            1
        );
        assert_eq!(manager.resolve(None).unwrap().project.elements.len(), 0);
        assert!(summaries
            .iter()
            .any(|summary| summary.id == first && !summary.active));
        assert!(summaries
            .iter()
            .any(|summary| summary.id == second && summary.active));
    }

    #[test]
    fn session_history_undo_redo_restores_snapshots_and_clears_redo() {
        let mut manager = ProjectSessionManager::default();
        let id = manager.create_session(Project::new("History", 176, 166, ModTarget::Forge));

        manager.record_history(Some(&id)).unwrap();
        manager
            .resolve_mut(Some(&id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1"));
        manager.mark_changed(Some(&id)).unwrap();

        let undone = manager.undo(Some(&id)).unwrap();
        assert_eq!(
            manager.resolve(Some(&id)).unwrap().project.elements.len(),
            0
        );
        assert_eq!(undone.revision, 2);

        manager.redo(Some(&id)).unwrap();
        assert_eq!(
            manager.resolve(Some(&id)).unwrap().project.elements.len(),
            1
        );

        manager.record_history(Some(&id)).unwrap();
        manager
            .resolve_mut(Some(&id))
            .unwrap()
            .project
            .add_element(sample_element("slot_2"));
        manager.mark_changed(Some(&id)).unwrap();

        assert!(manager.redo(Some(&id)).is_err());
    }
}
