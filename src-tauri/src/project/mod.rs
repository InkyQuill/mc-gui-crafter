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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UvRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
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
    #[serde(skip)]
    pub project_path: Option<String>,
    #[serde(skip)]
    pub is_dirty: bool,
    #[serde(skip)]
    pub texture_data: HashMap<String, Vec<u8>>,
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
            project_path: None,
            is_dirty: true,
            texture_data: HashMap::new(),
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

    fn sample_element(id: &str) -> Element {
        Element {
            id: id.to_string(),
            element_type: ElementType::Slot,
            x: 8,
            y: 18,
            width: None,
            height: None,
            size: Some(18),
            asset: None,
            direction: Some(FillDirection::LeftToRight),
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: None,
            visible: true,
            uv: Some(UvRect {
                x: 1,
                y: 2,
                width: 16,
                height: 16,
            }),
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
