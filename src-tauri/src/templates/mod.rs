use crate::project::{
    Element, ElementType, FillDirection, Group, Layer, Project, SemanticGroup, SemanticGroupKind,
    SlotRole,
};
use serde::Serialize;

pub const GENERATED_GUI_PANEL: &str = "textures/generated/gui_panel.png";
pub const GENERATED_SLOT: &str = "textures/generated/slot.png";
pub const GENERATED_PROGRESS_ARROW: &str = "textures/generated/progress_arrow.png";
pub const GENERATED_FLUID_TANK: &str = "textures/generated/fluid_tank.png";
pub const GENERATED_ENERGY_BAR: &str = "textures/generated/energy_bar.png";

const SLOT_SIZE: i32 = 18;
const SLOT_STEP: i32 = 18;
const PLAYER_INVENTORY_ID: &str = "player_inventory";
const HOTBAR_ID: &str = "hotbar";
const PLAYER_INVENTORY_X: i32 = 8;
const PLAYER_INVENTORY_Y: i32 = 84;
const HOTBAR_X: i32 = 8;
const HOTBAR_Y: i32 = 142;

pub struct Template {
    pub name: &'static str,
    pub description: &'static str,
    pub default_width: u32,
    pub default_height: u32,
    pub elements: Vec<Element>,
    pub groups: Vec<Group>,
    pub semantic_groups: Vec<SemanticGroup>,
}

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
    vec![
        empty(),
        with_player_inventory(furnace()),
        crafting_3x3(),
        chest_9x3(),
        chest_9x6(),
        with_player_inventory(advanced_machine()),
        with_player_inventory(scrollable_inventory_machine()),
        with_player_inventory(fluid_tank()),
        with_player_inventory(brewing_stand()),
        with_player_inventory(anvil()),
        with_player_inventory(custom_grid_default()),
    ]
}

pub fn get_template(name: &str) -> Option<Template> {
    list_templates().into_iter().find(|t| t.name == name)
}

fn ensure_generated_asset_path(project: &mut Project, path: &str) {
    if !project.assets.iter().any(|asset| asset == path) {
        project.assets.push(path.to_string());
    }
}

fn generated_panel_matches_size(bytes: &[u8], width: u32, height: u32) -> bool {
    image::load_from_memory(bytes)
        .map(|img| img.width() == width && img.height() == height)
        .unwrap_or(false)
}

fn add_generated_panel_asset(project: &mut Project) -> Result<(), String> {
    ensure_generated_asset_path(project, GENERATED_GUI_PANEL);
    let should_regenerate = project
        .texture_data
        .get(GENERATED_GUI_PANEL)
        .map(|bytes| {
            !generated_panel_matches_size(bytes, project.gui_size.width, project.gui_size.height)
        })
        .unwrap_or(true);

    if should_regenerate {
        project.texture_data.insert(
            GENERATED_GUI_PANEL.to_string(),
            crate::texture::generated_gui_panel(project.gui_size.width, project.gui_size.height)?,
        );
    }

    Ok(())
}

fn add_static_generated_asset(project: &mut Project, path: &str, bytes: Vec<u8>) {
    ensure_generated_asset_path(project, path);
    project
        .texture_data
        .entry(path.to_string())
        .or_insert(bytes);
}

fn add_generated_template_assets(project: &mut Project) -> Result<(), String> {
    add_generated_panel_asset(project)?;
    add_static_generated_asset(project, GENERATED_SLOT, crate::texture::generated_slot()?);
    add_static_generated_asset(
        project,
        GENERATED_PROGRESS_ARROW,
        crate::texture::generated_progress_arrow()?,
    );
    add_static_generated_asset(
        project,
        GENERATED_FLUID_TANK,
        crate::texture::generated_fluid_tank()?,
    );
    add_static_generated_asset(
        project,
        GENERATED_ENERGY_BAR,
        crate::texture::generated_energy_bar()?,
    );
    Ok(())
}

fn base_element(id: &str, element_type: ElementType, x: i32, y: i32) -> Element {
    Element {
        id: id.into(),
        element_type,
        x,
        y,
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

fn slot_grid(
    id_prefix: &str,
    x: i32,
    y: i32,
    columns: u32,
    rows: u32,
    slot_role: SlotRole,
    inventory_group: &str,
    slot_index_start: u32,
) -> Vec<Element> {
    let mut elements = Vec::with_capacity((columns * rows) as usize);
    for local_index in 0..columns * rows {
        let column = local_index % columns;
        let row = local_index / columns;
        let slot_index = slot_index_start + local_index;
        elements.push(Element {
            size: Some(SLOT_SIZE as u32),
            slot_role: Some(slot_role.clone()),
            slot_index: Some(slot_index),
            inventory_group: Some(inventory_group.into()),
            ..base_element(
                &format!("{id_prefix}_{slot_index}"),
                ElementType::Slot,
                x + column as i32 * SLOT_STEP,
                y + row as i32 * SLOT_STEP,
            )
        });
    }
    elements
}

fn player_inventory_grid() -> Vec<Element> {
    slot_grid(
        PLAYER_INVENTORY_ID,
        PLAYER_INVENTORY_X,
        PLAYER_INVENTORY_Y,
        9,
        3,
        SlotRole::PlayerInventory,
        PLAYER_INVENTORY_ID,
        9,
    )
}

fn hotbar_grid() -> Vec<Element> {
    slot_grid(
        HOTBAR_ID,
        HOTBAR_X,
        HOTBAR_Y,
        9,
        1,
        SlotRole::Hotbar,
        HOTBAR_ID,
        0,
    )
}

fn inventory_semantic_group(
    id: &str,
    kind: SemanticGroupKind,
    rows: u32,
    slot_count: u32,
) -> SemanticGroup {
    SemanticGroup {
        id: id.into(),
        kind,
        columns: Some(9),
        visible_rows: Some(rows),
        total_rows: Some(rows),
        slot_count: Some(slot_count),
        data_source: Some(id.into()),
        scroll_binding: None,
        dynamic_height: false,
    }
}

fn player_inventory_semantic_groups() -> [SemanticGroup; 2] {
    [
        inventory_semantic_group(
            PLAYER_INVENTORY_ID,
            SemanticGroupKind::PlayerInventory,
            3,
            27,
        ),
        inventory_semantic_group(HOTBAR_ID, SemanticGroupKind::Hotbar, 1, 9),
    ]
}

fn group_for_slots(id: &str, x: i32, y: i32, elements: &[Element]) -> Option<Group> {
    let element_ids = elements
        .iter()
        .filter(|element| element.inventory_group.as_deref() == Some(id))
        .map(|element| element.id.clone())
        .collect::<Vec<_>>();
    (!element_ids.is_empty()).then(|| Group {
        id: id.into(),
        x,
        y,
        elements: element_ids,
    })
}

fn append_player_inventory(template: &mut Template) {
    if !template.elements.iter().any(|element| {
        element.slot_role == Some(SlotRole::PlayerInventory)
            && element.inventory_group.as_deref() == Some(PLAYER_INVENTORY_ID)
    }) {
        template.elements.extend(player_inventory_grid());
    }
    if !template.elements.iter().any(|element| {
        element.slot_role == Some(SlotRole::Hotbar)
            && element.inventory_group.as_deref() == Some(HOTBAR_ID)
    }) {
        template.elements.extend(hotbar_grid());
    }

    for semantic_group in player_inventory_semantic_groups() {
        if !template
            .semantic_groups
            .iter()
            .any(|group| group.id == semantic_group.id)
        {
            template.semantic_groups.push(semantic_group);
        }
    }

    if !template
        .groups
        .iter()
        .any(|group| group.id == PLAYER_INVENTORY_ID)
    {
        if let Some(group) = group_for_slots(
            PLAYER_INVENTORY_ID,
            PLAYER_INVENTORY_X,
            PLAYER_INVENTORY_Y,
            &template.elements,
        ) {
            template.groups.push(group);
        }
    }
    if !template.groups.iter().any(|group| group.id == HOTBAR_ID) {
        if let Some(group) = group_for_slots(HOTBAR_ID, HOTBAR_X, HOTBAR_Y, &template.elements) {
            template.groups.push(group);
        }
    }
}

fn with_player_inventory(mut template: Template) -> Template {
    append_player_inventory(&mut template);
    template
}

fn empty() -> Template {
    Template {
        name: "empty",
        description: "Blank canvas of configurable size",
        default_width: 176,
        default_height: 166,
        elements: vec![],
        groups: vec![],
        semantic_groups: vec![],
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
                asset: Some(GENERATED_GUI_PANEL.into()),
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
            },
            Element {
                id: "input_slot".into(),
                element_type: ElementType::Slot,
                x: 56,
                y: 17,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "fuel_slot".into(),
                element_type: ElementType::Slot,
                x: 56,
                y: 53,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "output_slot".into(),
                element_type: ElementType::Slot,
                x: 116,
                y: 35,
                size: Some(18),
                width: None,
                height: None,
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
                layer: Layer::Overlay,
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
            },
        ],
        groups: vec![],
        semantic_groups: vec![],
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
        asset: Some(GENERATED_GUI_PANEL.into()),
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
    }];

    for row in 0..3 {
        for col in 0..3 {
            elements.push(Element {
                id: format!("craft_grid_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 30 + col * SLOT_STEP,
                y: 17 + row * SLOT_STEP,
                size: Some(SLOT_SIZE as u32),
                width: None,
                height: None,
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
            });
        }
    }

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
    });

    elements.push(Element {
        id: "output_slot".into(),
        element_type: ElementType::Slot,
        x: 124,
        y: 35,
        size: Some(18),
        width: None,
        height: None,
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
    });

    Template {
        name: "crafting_3x3",
        description: "3×3 crafting grid with output slot",
        default_width: 176,
        default_height: 166,
        elements,
        groups: vec![],
        semantic_groups: vec![],
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
        asset: Some(GENERATED_GUI_PANEL.into()),
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
    }];

    for row in 0..3 {
        for col in 0..9 {
            elements.push(Element {
                id: format!("inv_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 8 + col * SLOT_STEP,
                y: 18 + row * SLOT_STEP,
                size: Some(SLOT_SIZE as u32),
                width: None,
                height: None,
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
            });
        }
    }

    Template {
        name: "chest_9x3",
        description: "Standard chest inventory (9×3 grid)",
        default_width: 176,
        default_height: 166,
        elements,
        groups: vec![],
        semantic_groups: vec![],
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
        asset: Some(GENERATED_GUI_PANEL.into()),
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
    }];

    for row in 0..6 {
        for col in 0..9 {
            elements.push(Element {
                id: format!("inv_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 8 + col * SLOT_STEP,
                y: 18 + row * SLOT_STEP,
                size: Some(SLOT_SIZE as u32),
                width: None,
                height: None,
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
            });
        }
    }

    Template {
        name: "chest_9x6",
        description: "Double chest inventory (9×6 grid)",
        default_width: 176,
        default_height: 222,
        elements,
        groups: vec![],
        semantic_groups: vec![],
    }
}

// --- NEW TEMPLATES ---

fn advanced_machine() -> Template {
    Template {
        name: "advanced_machine",
        description:
            "Advanced machine: input, fuel, output, progress arrow, 2 fluid tanks, energy bar",
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
                asset: Some(GENERATED_GUI_PANEL.into()),
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
                layer: Layer::Overlay,
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
            },
            Element {
                id: "input_slot".into(),
                element_type: ElementType::Slot,
                x: 44,
                y: 17,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "fuel_slot".into(),
                element_type: ElementType::Slot,
                x: 44,
                y: 59,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "output_slot".into(),
                element_type: ElementType::Slot,
                x: 116,
                y: 38,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "progress_arrow".into(),
                element_type: ElementType::Progress,
                x: 73,
                y: 38,
                width: Some(22),
                height: Some(15),
                size: None,
                asset: None,
                direction: Some(crate::project::FillDirection::LeftToRight),
                content: None,
                font: None,
                color: None,
                shadow: None,
                animation: Some("cook_progress".into()),
                visible: true,
                uv: None,
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
            },
            Element {
                id: "fluid_tank_left".into(),
                element_type: ElementType::FluidTank,
                x: 16,
                y: 17,
                width: Some(16),
                height: Some(48),
                size: None,
                asset: None,
                direction: Some(crate::project::FillDirection::BottomToTop),
                content: None,
                font: None,
                color: None,
                shadow: None,
                animation: Some("fluid_left".into()),
                visible: true,
                uv: None,
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
            },
            Element {
                id: "fluid_tank_right".into(),
                element_type: ElementType::FluidTank,
                x: 144,
                y: 17,
                width: Some(16),
                height: Some(48),
                size: None,
                asset: None,
                direction: Some(crate::project::FillDirection::BottomToTop),
                content: None,
                font: None,
                color: None,
                shadow: None,
                animation: Some("fluid_right".into()),
                visible: true,
                uv: None,
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
            },
            Element {
                id: "energy_bar".into(),
                element_type: ElementType::EnergyBar,
                x: 152,
                y: 17,
                width: Some(12),
                height: Some(48),
                size: None,
                asset: None,
                direction: Some(crate::project::FillDirection::BottomToTop),
                content: None,
                font: None,
                color: None,
                shadow: None,
                animation: Some("energy".into()),
                visible: true,
                uv: None,
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
            },
        ],
        groups: vec![],
        semantic_groups: vec![],
    }
}

fn scrollable_inventory_machine() -> Template {
    let mut elements = vec![
        Element {
            width: Some(176),
            height: Some(166),
            asset: Some(GENERATED_GUI_PANEL.into()),
            ..base_element("bg", ElementType::Texture, 0, 0)
        },
        Element {
            content: Some("Scrollable Machine".into()),
            font: Some("minecraft:default".into()),
            color: Some(0x404040),
            shadow: Some(true),
            layer: Layer::Overlay,
            ..base_element("title", ElementType::Text, 8, 6)
        },
        Element {
            size: Some(SLOT_SIZE as u32),
            slot_role: Some(SlotRole::Machine),
            slot_index: Some(0),
            inventory_group: Some("machine".into()),
            ..base_element("input_left", ElementType::Slot, 106, 22)
        },
        Element {
            size: Some(SLOT_SIZE as u32),
            slot_role: Some(SlotRole::Machine),
            slot_index: Some(1),
            inventory_group: Some("machine".into()),
            ..base_element("input_right", ElementType::Slot, 126, 22)
        },
        Element {
            size: Some(SLOT_SIZE as u32),
            slot_role: Some(SlotRole::Machine),
            slot_index: Some(2),
            inventory_group: Some("machine".into()),
            ..base_element("output", ElementType::Slot, 146, 22)
        },
        Element {
            width: Some(22),
            height: Some(15),
            direction: Some(FillDirection::LeftToRight),
            animation: Some("progress".into()),
            layer: Layer::Animatable,
            ..base_element("progress_arrow", ElementType::Progress, 122, 46)
        },
    ];

    for row in 0..3 {
        for column in 0..5 {
            let visible_index = row * 5 + column;
            elements.push(Element {
                size: Some(SLOT_SIZE as u32),
                slot_role: Some(SlotRole::ScrollableInventory),
                slot_index: Some(visible_index as u32),
                inventory_group: Some("machine_buffer".into()),
                scroll_binding: Some("buffer_scroll".into()),
                ..base_element(
                    &format!("buffer_slot_{row}_{column}"),
                    ElementType::VirtualSlotCell,
                    8 + column * SLOT_STEP,
                    30 + row * SLOT_STEP,
                )
            });
        }
    }

    elements.push(Element {
        width: Some(12),
        height: Some(54),
        scroll_min: Some(0),
        scroll_max: Some(3),
        visible_rows: Some(3),
        total_rows: Some(6),
        columns: Some(5),
        target_group: Some("machine_buffer".into()),
        ..base_element("buffer_scroll", ElementType::Scrollbar, 102, 30)
    });

    Template {
        name: "scrollable_inventory_machine",
        description: "Machine with a scrollable 5x3 inventory viewport",
        default_width: 176,
        default_height: 166,
        elements,
        groups: vec![],
        semantic_groups: vec![SemanticGroup {
            id: "machine_buffer".into(),
            kind: SemanticGroupKind::VirtualSlotGrid,
            columns: Some(5),
            visible_rows: Some(3),
            total_rows: Some(6),
            slot_count: Some(30),
            data_source: Some("machine_buffer".into()),
            scroll_binding: Some("buffer_scroll".into()),
            dynamic_height: false,
        }],
    }
}

fn fluid_tank() -> Template {
    Template {
        name: "fluid_tank",
        description: "Fluid tank: input/output slots, fluid fill gauge, capacity text",
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
                asset: Some(GENERATED_GUI_PANEL.into()),
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
                content: Some("{fluid_name}".into()),
                font: Some("minecraft:default".into()),
                color: Some(0x404040),
                shadow: Some(true),
                animation: None,
                visible: true,
                uv: None,
                layer: Layer::Overlay,
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
            },
            Element {
                id: "fluid_fill".into(),
                element_type: ElementType::FluidTank,
                x: 35,
                y: 17,
                width: Some(20),
                height: Some(64),
                size: None,
                asset: None,
                direction: Some(crate::project::FillDirection::BottomToTop),
                content: None,
                font: None,
                color: None,
                shadow: None,
                animation: Some("fluid_amount".into()),
                visible: true,
                uv: None,
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
            },
            Element {
                id: "input_fluid_slot".into(),
                element_type: ElementType::Slot,
                x: 12,
                y: 56,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "output_fluid_slot".into(),
                element_type: ElementType::Slot,
                x: 62,
                y: 56,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "capacity_text".into(),
                element_type: ElementType::Text,
                x: 8,
                y: 88,
                width: None,
                height: None,
                size: None,
                asset: None,
                direction: None,
                content: Some("{amount} / {capacity} mB".into()),
                font: Some("minecraft:default".into()),
                color: Some(0x808080),
                shadow: Some(false),
                animation: None,
                visible: true,
                uv: None,
                layer: Layer::Overlay,
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
            },
        ],
        groups: vec![],
        semantic_groups: vec![],
    }
}

fn brewing_stand() -> Template {
    let mut elements = vec![
        Element {
            id: "bg".into(),
            element_type: ElementType::Texture,
            x: 0,
            y: 0,
            width: Some(176),
            height: Some(166),
            size: None,
            asset: Some(GENERATED_GUI_PANEL.into()),
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
            layer: Layer::Overlay,
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
        },
        Element {
            id: "ingredient_slot".into(),
            element_type: ElementType::Slot,
            x: 79,
            y: 17,
            size: Some(18),
            width: None,
            height: None,
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
        },
        Element {
            id: "blaze_slot".into(),
            element_type: ElementType::Slot,
            x: 79,
            y: 65,
            size: Some(18),
            width: None,
            height: None,
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
        },
    ];

    for i in 0..3 {
        let bottle_x = 56 + i * 24;
        elements.push(Element {
            id: format!("bottle_{i}"),
            element_type: ElementType::Slot,
            x: bottle_x,
            y: 40,
            size: Some(18),
            width: None,
            height: None,
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
        });
        elements.push(Element {
            id: format!("bubble_{i}"),
            element_type: ElementType::Progress,
            x: bottle_x + 14,
            y: 29,
            width: Some(8),
            height: Some(26),
            size: None,
            asset: None,
            direction: Some(crate::project::FillDirection::TopToBottom),
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: Some("brew_time".into()),
            visible: true,
            uv: None,
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
        });
    }

    elements.push(Element {
        id: "fuel_gauge".into(),
        element_type: ElementType::Progress,
        x: 79,
        y: 47,
        width: Some(18),
        height: Some(14),
        size: None,
        asset: None,
        direction: Some(crate::project::FillDirection::LeftToRight),
        content: None,
        font: None,
        color: None,
        shadow: None,
        animation: Some("fuel".into()),
        visible: true,
        uv: None,
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
    });

    Template {
        name: "brewing_stand",
        description:
            "Brewing stand: 3 bottles, ingredient, blaze powder, progress bubbles, fuel gauge",
        default_width: 176,
        default_height: 166,
        elements,
        groups: vec![],
        semantic_groups: vec![],
    }
}

fn anvil() -> Template {
    Template {
        name: "anvil",
        description: "Anvil: 2 input slots, output, level cost text, repair progress",
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
                asset: Some(GENERATED_GUI_PANEL.into()),
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
                content: Some("{item_name}".into()),
                font: Some("minecraft:default".into()),
                color: Some(0x404040),
                shadow: Some(true),
                animation: None,
                visible: true,
                uv: None,
                layer: Layer::Overlay,
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
            },
            Element {
                id: "input_slot_1".into(),
                element_type: ElementType::Slot,
                x: 27,
                y: 23,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "input_slot_2".into(),
                element_type: ElementType::Slot,
                x: 27,
                y: 47,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "output_slot".into(),
                element_type: ElementType::Slot,
                x: 107,
                y: 35,
                size: Some(18),
                width: None,
                height: None,
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
            },
            Element {
                id: "cost_text".into(),
                element_type: ElementType::Text,
                x: 130,
                y: 50,
                width: None,
                height: None,
                size: None,
                asset: None,
                direction: None,
                content: Some("{cost}".into()),
                font: Some("minecraft:default".into()),
                color: Some(0x00FF00),
                shadow: Some(false),
                animation: None,
                visible: true,
                uv: None,
                layer: Layer::Overlay,
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
            },
            Element {
                id: "progress_arrow".into(),
                element_type: ElementType::Progress,
                x: 75,
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
                animation: Some("repair_progress".into()),
                visible: true,
                uv: None,
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
            },
        ],
        groups: vec![],
        semantic_groups: vec![],
    }
}

fn custom_grid_default() -> Template {
    let mut elements = vec![Element {
        id: "bg".into(),
        element_type: ElementType::Texture,
        x: 0,
        y: 0,
        width: Some(176),
        height: Some(166),
        size: None,
        asset: Some(GENERATED_GUI_PANEL.into()),
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
    }];

    for row in 0..3 {
        for col in 0..3 {
            elements.push(Element {
                id: format!("grid_{}_{}", row, col),
                element_type: ElementType::Slot,
                x: 30 + col * SLOT_STEP,
                y: 17 + row * SLOT_STEP,
                size: Some(SLOT_SIZE as u32),
                width: None,
                height: None,
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
            });
        }
    }

    elements.push(Element {
        id: "progress_arrow".into(),
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
        animation: Some("custom_progress".into()),
        visible: true,
        uv: None,
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
    });

    elements.push(Element {
        id: "output_slot".into(),
        element_type: ElementType::Slot,
        x: 134,
        y: 35,
        size: Some(18),
        width: None,
        height: None,
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
    });

    Template {
        name: "custom_grid",
        description: "Custom N×M grid with optional output, progress, and inventory",
        default_width: 176,
        default_height: 166,
        elements,
        groups: vec![],
        semantic_groups: vec![],
    }
}

pub fn apply_template(project: &mut Project, template_name: &str) -> Result<(), String> {
    let template =
        get_template(template_name).ok_or_else(|| format!("Unknown template: {template_name}"))?;

    if template.name != "empty" {
        project.gui_size.width = template.default_width;
        project.gui_size.height = template.default_height;
    }
    project.elements = template.elements;
    project.groups = template.groups;
    project.semantic_groups = template.semantic_groups;
    project.animations.clear();
    add_generated_template_assets(project)?;
    project.is_dirty = true;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ElementType, ModTarget, Project, SemanticGroupKind, SlotRole};

    fn slot_right(element: &crate::project::Element) -> i32 {
        element.x + element.size.unwrap_or(18) as i32
    }

    fn slot_bottom(element: &crate::project::Element) -> i32 {
        element.y + element.size.unwrap_or(18) as i32
    }

    fn slot_like_rect(element: &crate::project::Element) -> Option<(i32, i32, i32, i32)> {
        match element.element_type {
            ElementType::Slot | ElementType::VirtualSlotCell => Some((
                element.x,
                element.y,
                slot_right(element),
                slot_bottom(element),
            )),
            _ => None,
        }
    }

    fn slot_like_rects_overlap(
        left: &crate::project::Element,
        right: &crate::project::Element,
    ) -> bool {
        let Some((left_x1, left_y1, left_x2, left_y2)) = slot_like_rect(left) else {
            return false;
        };
        let Some((right_x1, right_y1, right_x2, right_y2)) = slot_like_rect(right) else {
            return false;
        };

        left_x1 < right_x2 && right_x1 < left_x2 && left_y1 < right_y2 && right_y1 < left_y2
    }

    #[test]
    fn starter_template_slots_stay_inside_gui_bounds() {
        for template in list_templates() {
            for element in &template.elements {
                if element.element_type != ElementType::Slot {
                    continue;
                }

                assert!(
                    element.x >= 0,
                    "{} slot {} has negative x {}",
                    template.name,
                    element.id,
                    element.x
                );
                assert!(
                    element.y >= 0,
                    "{} slot {} has negative y {}",
                    template.name,
                    element.id,
                    element.y
                );
                assert!(
                    slot_right(element) <= template.default_width as i32,
                    "{} slot {} right edge {} exceeds width {}",
                    template.name,
                    element.id,
                    slot_right(element),
                    template.default_width
                );
                assert!(
                    slot_bottom(element) <= template.default_height as i32,
                    "{} slot {} bottom edge {} exceeds height {}",
                    template.name,
                    element.id,
                    slot_bottom(element),
                    template.default_height
                );
            }
        }
    }

    #[test]
    fn nine_column_inventory_templates_use_eighteen_pixel_cadence() {
        for name in ["chest_9x3", "chest_9x6"] {
            let template = get_template(name).expect("template exists");
            let first_row: Vec<_> = template
                .elements
                .iter()
                .filter(|element| element.element_type == ElementType::Slot && element.y == 18)
                .collect();

            assert_eq!(first_row.len(), 9, "{name} first row should have 9 slots");
            for pair in first_row.windows(2) {
                assert_eq!(
                    pair[1].x - pair[0].x,
                    18,
                    "{name} slot cadence should be 18px"
                );
            }
            assert_eq!(first_row[8].x + 18 - first_row[0].x, 162);
        }
    }

    #[test]
    fn crafting_grid_uses_eighteen_pixel_cadence() {
        let template = get_template("crafting_3x3").expect("template exists");
        let first_row: Vec<_> = template
            .elements
            .iter()
            .filter(|element| {
                element.element_type == ElementType::Slot && element.id.starts_with("craft_grid_0_")
            })
            .collect();

        assert_eq!(first_row.len(), 3);
        for pair in first_row.windows(2) {
            assert_eq!(pair[1].x - pair[0].x, 18);
        }
    }

    #[test]
    fn applying_empty_template_preserves_requested_canvas_size() {
        let mut project = Project::new("Custom Empty", 264, 162, ModTarget::Forge);

        apply_template(&mut project, "empty").expect("template applies");

        assert_eq!(project.gui_size.width, 264);
        assert_eq!(project.gui_size.height, 162);
        assert!(project.elements.is_empty());
    }

    #[test]
    fn machine_templates_include_vanilla_player_inventory_and_hotbar_metadata() {
        for name in [
            "furnace",
            "advanced_machine",
            "fluid_tank",
            "brewing_stand",
            "anvil",
            "scrollable_inventory_machine",
            "custom_grid",
        ] {
            let template = get_template(name).expect("template exists");
            let player_inventory = template
                .elements
                .iter()
                .filter(|element| {
                    element.element_type == ElementType::Slot
                        && element.slot_role == Some(SlotRole::PlayerInventory)
                        && element.inventory_group.as_deref() == Some("player_inventory")
                })
                .collect::<Vec<_>>();
            let hotbar = template
                .elements
                .iter()
                .filter(|element| {
                    element.element_type == ElementType::Slot
                        && element.slot_role == Some(SlotRole::Hotbar)
                        && element.inventory_group.as_deref() == Some("hotbar")
                })
                .collect::<Vec<_>>();

            assert_eq!(player_inventory.len(), 27, "{name} player inventory slots");
            assert_eq!(hotbar.len(), 9, "{name} hotbar slots");
            for slot in &player_inventory {
                let index = slot.slot_index.expect("player inventory slot index");
                assert!(
                    (9..=35).contains(&index),
                    "{name} player inventory slot index {index}"
                );
                assert_eq!(slot.x, 8 + ((index - 9) % 9) as i32 * 18);
                assert_eq!(slot.y, 84 + ((index - 9) / 9) as i32 * 18);
            }
            for slot in &hotbar {
                let index = slot.slot_index.expect("hotbar slot index");
                assert!(index <= 8, "{name} hotbar slot index {index}");
                assert_eq!(slot.x, 8 + index as i32 * 18);
                assert_eq!(slot.y, 142);
            }

            let player_group = template
                .semantic_groups
                .iter()
                .find(|group| group.id == "player_inventory")
                .expect("player_inventory semantic group");
            assert_eq!(player_group.kind, SemanticGroupKind::PlayerInventory);
            assert_eq!(player_group.columns, Some(9));
            assert_eq!(player_group.visible_rows, Some(3));
            assert_eq!(player_group.total_rows, Some(3));
            assert_eq!(player_group.slot_count, Some(27));
            assert_eq!(
                player_group.data_source.as_deref(),
                Some("player_inventory")
            );

            let hotbar_group = template
                .semantic_groups
                .iter()
                .find(|group| group.id == "hotbar")
                .expect("hotbar semantic group");
            assert_eq!(hotbar_group.kind, SemanticGroupKind::Hotbar);
            assert_eq!(hotbar_group.columns, Some(9));
            assert_eq!(hotbar_group.visible_rows, Some(1));
            assert_eq!(hotbar_group.total_rows, Some(1));
            assert_eq!(hotbar_group.slot_count, Some(9));
            assert_eq!(hotbar_group.data_source.as_deref(), Some("hotbar"));

            let player_project_group = template
                .groups
                .iter()
                .find(|group| group.id == "player_inventory")
                .expect("player_inventory project group");
            assert_eq!(player_project_group.x, 8);
            assert_eq!(player_project_group.y, 84);
            assert_eq!(player_project_group.elements.len(), 27);

            let hotbar_project_group = template
                .groups
                .iter()
                .find(|group| group.id == "hotbar")
                .expect("hotbar project group");
            assert_eq!(hotbar_project_group.x, 8);
            assert_eq!(hotbar_project_group.y, 142);
            assert_eq!(hotbar_project_group.elements.len(), 9);
        }
    }

    #[test]
    fn applying_template_inserts_generated_background_asset() {
        let mut project = Project::new("Generated", 1, 1, ModTarget::Forge);

        apply_template(&mut project, "furnace").expect("template applies");

        assert!(project
            .assets
            .iter()
            .any(|asset| asset == GENERATED_GUI_PANEL));
        assert!(project.texture_data.contains_key(GENERATED_GUI_PANEL));
    }

    #[test]
    fn scrollable_inventory_template_is_listed() {
        let templates = list_template_info();
        let template = templates
            .iter()
            .find(|template| template.name == "scrollable_inventory_machine")
            .unwrap();
        assert_eq!(template.default_width, 176);
        assert_eq!(template.default_height, 166);
    }

    #[test]
    fn scrollable_inventory_template_has_semantic_slots_and_scrollbar() {
        let mut project = Project::new("Scrollable", 176, 166, ModTarget::Forge);
        apply_template(&mut project, "scrollable_inventory_machine").unwrap();

        let scrollable_slots = project
            .elements
            .iter()
            .filter(|element| element.slot_role == Some(SlotRole::ScrollableInventory))
            .count();
        assert_eq!(scrollable_slots, 15);
        assert!(project
            .elements
            .iter()
            .any(|element| element.element_type == ElementType::Scrollbar));
        assert!(project
            .semantic_groups
            .iter()
            .any(|group| group.id == "machine_buffer"));
    }

    #[test]
    fn scrollable_inventory_template_has_no_real_and_virtual_slot_overlap() {
        let template = get_template("scrollable_inventory_machine").expect("template exists");
        let real_slots = template
            .elements
            .iter()
            .filter(|element| element.element_type == ElementType::Slot);
        let virtual_slots = template
            .elements
            .iter()
            .filter(|element| element.element_type == ElementType::VirtualSlotCell);

        for real_slot in real_slots {
            for virtual_slot in virtual_slots.clone() {
                assert!(
                    !slot_like_rects_overlap(real_slot, virtual_slot),
                    "{} real slot {} at ({}, {}) overlaps virtual slot {} at ({}, {})",
                    template.name,
                    real_slot.id,
                    real_slot.x,
                    real_slot.y,
                    virtual_slot.id,
                    virtual_slot.x,
                    virtual_slot.y
                );
            }
        }
    }

    #[test]
    fn applying_different_sized_templates_regenerates_generated_background_asset() {
        let mut project = Project::new("Generated", 1, 1, ModTarget::Forge);

        apply_template(&mut project, "chest_9x3").expect("template applies");
        let first_panel = project
            .texture_data
            .get(GENERATED_GUI_PANEL)
            .expect("generated panel exists");
        let first_decoded = image::load_from_memory(first_panel).unwrap().to_rgba8();
        assert_eq!(first_decoded.height(), 166);

        apply_template(&mut project, "chest_9x6").expect("template applies");
        let second_panel = project
            .texture_data
            .get(GENERATED_GUI_PANEL)
            .expect("generated panel exists");
        let second_decoded = image::load_from_memory(second_panel).unwrap().to_rgba8();
        assert_eq!(second_decoded.height(), 222);

        assert_eq!(
            project
                .assets
                .iter()
                .filter(|asset| asset.as_str() == GENERATED_GUI_PANEL)
                .count(),
            1
        );
    }

    #[test]
    fn applying_same_sized_template_preserves_existing_generated_background_asset() {
        let mut project = Project::new("Generated", 176, 166, ModTarget::Forge);
        let edited_panel =
            image::RgbaImage::from_pixel(176, 166, image::Rgba([0x42, 0x42, 0x42, 0xff]));
        let mut edited_bytes = Vec::new();
        edited_panel
            .write_to(
                &mut std::io::Cursor::new(&mut edited_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        project
            .texture_data
            .insert(GENERATED_GUI_PANEL.to_string(), edited_bytes.clone());
        project.assets.push(GENERATED_GUI_PANEL.to_string());

        apply_template(&mut project, "furnace").expect("template applies");

        assert_eq!(
            project.texture_data.get(GENERATED_GUI_PANEL),
            Some(&edited_bytes)
        );
    }
}
