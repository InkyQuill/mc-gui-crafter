use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_binding: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dynamic_height: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CodegenMode {
    #[default]
    Simple,
    Modular,
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
    pub fn normalized(mut self) -> Self {
        match self.codegen_mode {
            CodegenMode::Simple => self.generate_semantic_registry = false,
            CodegenMode::Modular => self.generate_semantic_registry = true,
        }
        self
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    #[default]
    Background,
    Overlay,
    Animatable,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub elements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub name: String,
    pub gui_size: Size,
    pub mod_target: ModTarget,
    pub elements: Vec<Element>,
    pub groups: Vec<Group>,
    pub animations: Vec<crate::animation::Animation>,
    pub assets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_groups: Vec<SemanticGroup>,
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
            animations: Vec::new(),
            assets: Vec::new(),
            semantic_groups: Vec::new(),
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
            direction: None,
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: None,
            visible: true,
            uv: None,
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
            direction: None,
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: None,
            visible: true,
            uv: None,
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
