use crate::project::{Element, ElementType, Project};

pub struct Template {
    pub name: &'static str,
    pub description: &'static str,
    pub default_width: u32,
    pub default_height: u32,
    pub elements: Vec<Element>,
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TemplateInfo {
    pub name: String,
    pub description: String,
    pub default_width: u32,
    pub default_height: u32,
    pub element_count: usize,
}

pub fn list_template_info() -> Vec<TemplateInfo> {
    list_templates()
        .into_iter()
        .map(|t| TemplateInfo {
            name: t.name.to_string(),
            description: t.description.to_string(),
            default_width: t.default_width,
            default_height: t.default_height,
            element_count: t.elements.len(),
        })
        .collect()
}

pub fn list_templates() -> Vec<Template> {
    vec![empty(), furnace(), crafting_3x3(), chest_9x3(), chest_9x6()]
}

pub fn get_template(name: &str) -> Option<Template> {
    list_templates().into_iter().find(|t| t.name == name)
}

fn empty() -> Template {
    Template {
        name: "empty",
        description: "Blank canvas of configurable size",
        default_width: 176,
        default_height: 166,
        elements: vec![],
    }
}

fn furnace() -> Template {
    Template {
        name: "furnace",
        description: "Furnace: input, fuel, progress arrow, output, player inventory",
        default_width: 176,
        default_height: 166,
        elements: vec![
            Element {
                id: "bg".into(),
                element_type: ElementType::Texture,
                x: 0,
                y: 0,
                width: Some(176),
                height: Some(166),
                size: None,
                asset: Some("textures/background.png".into()),
                direction: None,
                content: None,
                font: None,
                color: None,
                shadow: None,
                animation: None,
                visible: true,
                uv: None,
            },
            Element {
                id: "input_slot".into(),
                element_type: ElementType::Slot,
                x: 56,
                y: 17,
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
            },
            Element {
                id: "fuel_slot".into(),
                element_type: ElementType::Slot,
                x: 56,
                y: 53,
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
            },
            Element {
                id: "output_slot".into(),
                element_type: ElementType::Slot,
                x: 116,
                y: 35,
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
            },
            Element {
                id: "progress_arrow".into(),
                element_type: ElementType::Progress,
                x: 79,
                y: 35,
                width: Some(22),
                height: Some(15),
                size: None,
                asset: None,
                direction: Some(crate::project::FillDirection::LeftToRight),
                content: None,
                font: None,
                color: None,
                shadow: None,
                animation: Some("arrow_fill".into()),
                visible: true,
                uv: None,
            },
            Element {
                id: "title".into(),
                element_type: ElementType::Text,
                x: 8,
                y: 6,
                width: None,
                height: None,
                size: None,
                asset: None,
                direction: None,
                content: Some("{machine_name}".into()),
                font: Some("minecraft:default".into()),
                color: Some(0x404040),
                shadow: Some(true),
                animation: None,
                visible: true,
                uv: None,
            },
        ],
    }
}

fn crafting_3x3() -> Template {
    let mut elements = vec![Element {
        id: "bg".into(),
        element_type: ElementType::Texture,
        x: 0,
        y: 0,
        width: Some(176),
        height: Some(166),
        size: None,
        asset: Some("textures/background.png".into()),
        direction: None,
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: None,
        visible: true,
        uv: None,
    }];

    // 3x3 crafting grid (x=30, y=17 to x=84, y=71)
    for row in 0..3 {
        for col in 0..3 {
            elements.push(Element {
                id: format!("craft_grid_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 30 + col * (18 + 2),
                y: 17 + row * (18 + 2),
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
            });
        }
    }

    // Arrow between grid and output
    elements.push(Element {
        id: "craft_arrow".into(),
        element_type: ElementType::Progress,
        x: 98,
        y: 36,
        width: Some(22),
        height: Some(15),
        size: None,
        asset: None,
        direction: Some(crate::project::FillDirection::LeftToRight),
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: Some("craft_progress".into()),
        visible: true,
        uv: None,
    });

    // Output slot
    elements.push(Element {
        id: "output_slot".into(),
        element_type: ElementType::Slot,
        x: 124,
        y: 35,
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
    });

    Template {
        name: "crafting_3x3",
        description: "3×3 crafting grid with output slot",
        default_width: 176,
        default_height: 166,
        elements,
    }
}

fn chest_9x3() -> Template {
    let mut elements = vec![Element {
        id: "bg".into(),
        element_type: ElementType::Texture,
        x: 0,
        y: 0,
        width: Some(176),
        height: Some(166),
        size: None,
        asset: Some("textures/background.png".into()),
        direction: None,
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: None,
        visible: true,
        uv: None,
    }];

    for row in 0..3 {
        for col in 0..9 {
            elements.push(Element {
                id: format!("inv_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 8 + col * (18 + 2),
                y: 18 + row * (18 + 2),
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
            });
        }
    }

    Template {
        name: "chest_9x3",
        description: "Standard chest inventory (9×3 grid)",
        default_width: 176,
        default_height: 166,
        elements,
    }
}

fn chest_9x6() -> Template {
    let mut elements = vec![Element {
        id: "bg".into(),
        element_type: ElementType::Texture,
        x: 0,
        y: 0,
        width: Some(176),
        height: Some(222),
        size: None,
        asset: Some("textures/background.png".into()),
        direction: None,
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: None,
        visible: true,
        uv: None,
    }];

    for row in 0..6 {
        for col in 0..9 {
            elements.push(Element {
                id: format!("inv_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 8 + col * (18 + 2),
                y: 18 + row * (18 + 2),
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
            });
        }
    }

    Template {
        name: "chest_9x6",
        description: "Double chest inventory (9×6 grid)",
        default_width: 176,
        default_height: 222,
        elements,
    }
}

pub fn apply_template(project: &mut Project, template_name: &str) -> Result<(), String> {
    let template =
        get_template(template_name).ok_or_else(|| format!("Unknown template: {template_name}"))?;

    project.gui_size.width = template.default_width;
    project.gui_size.height = template.default_height;
    project.elements = template.elements;
    project.groups.clear();
    project.animations.clear();
    project.is_dirty = true;

    Ok(())
}
