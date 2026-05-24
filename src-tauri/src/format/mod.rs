use crate::project::Project;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use zip::ZipArchive;

#[derive(Deserialize)]
struct LayoutData {
    #[serde(default)]
    elements: Vec<crate::project::Element>,
    #[serde(default)]
    groups: Vec<crate::project::Group>,
    #[serde(default)]
    semantic_groups: Vec<crate::project::SemanticGroup>,
    #[serde(default)]
    export_settings: crate::project::ProjectExportSettings,
}

#[derive(Deserialize)]
struct FontsData {
    #[serde(default)]
    fonts: Vec<crate::project::FontAsset>,
}

pub fn load_from_mcgui(path: &str) -> Result<Project, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open project: {e}"))?;

    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read .mcgui archive: {e}"))?;

    // Read manifest
    let manifest_json = read_json_entry(&mut archive, "manifest.json")?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_json)
        .map_err(|e| format!("Failed to parse manifest.json: {e}"))?;

    let name = manifest["name"].as_str().unwrap_or("Untitled").to_string();
    let gui_width = manifest["gui_size"]["width"].as_u64().unwrap_or(176) as u32;
    let gui_height = manifest["gui_size"]["height"].as_u64().unwrap_or(166) as u32;
    let mod_target = match manifest["mod_target"].as_str().unwrap_or("forge") {
        "fabric" => crate::project::ModTarget::Fabric,
        "neoforge" => crate::project::ModTarget::NeoForge,
        _ => crate::project::ModTarget::Forge,
    };

    // Read layout
    let layout_json = read_json_entry(&mut archive, "layout.json")?;
    let layout: LayoutData = serde_json::from_str(&layout_json)
        .map_err(|e| format!("Failed to parse layout.json: {e}"))?;

    let mut project = Project::new(&name, gui_width, gui_height, mod_target);
    project.elements = layout.elements;
    project.groups = layout.groups;
    project.semantic_groups = layout.semantic_groups;
    project.export_settings = layout.export_settings;

    // Read animations
    if let Ok(anim_json) = read_json_entry(&mut archive, "animations.json") {
        project.animations = serde_json::from_str(&anim_json)
            .map_err(|e| format!("Failed to parse animations.json: {e}"))?;
    }

    if let Ok(fonts_json) = read_json_entry(&mut archive, "fonts.json") {
        let fonts: FontsData = serde_json::from_str(&fonts_json)
            .map_err(|e| format!("Failed to parse fonts.json: {e}"))?;
        project.fonts = fonts.fonts;
    }

    // Collect asset names and read texture data
    let mut texture_data: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {e}"))?;
        let name = entry.name().to_string();
        if name.starts_with("textures/") && name.ends_with(".png") {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read texture entry '{}': {e}", name))?;
            texture_data.insert(name.clone(), buf);
            project.assets.push(name);
        }
    }
    project.texture_data = texture_data;

    project.project_path = Some(path.to_string());
    project.is_dirty = false;

    Ok(project)
}

pub fn save_to_mcgui(project: &Project) -> Result<(), String> {
    let path = project
        .project_path
        .as_ref()
        .ok_or("No project path set. Use Save As first.")?;

    // Write to temp file, then rename atomically
    let tmp_path = format!("{path}.tmp");

    let file =
        std::fs::File::create(&tmp_path).map_err(|e| format!("Failed to create temp file: {e}"))?;

    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Write manifest
    let manifest = serde_json::json!({
        "version": 1,
        "name": project.name,
        "gui_size": project.gui_size,
        "mod_target": project.mod_target,
    });
    zip_writer
        .start_file("manifest.json", options)
        .map_err(|e| format!("Zip error: {e}"))?;
    zip_writer
        .write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;

    // Write layout (elements + groups, minus non-serializable fields)
    let layout = serde_json::json!({
        "elements": project.elements,
        "groups": project.groups,
        "semantic_groups": project.semantic_groups,
        "export_settings": project.export_settings,
    });
    zip_writer
        .start_file("layout.json", options)
        .map_err(|e| format!("Zip error: {e}"))?;
    zip_writer
        .write_all(serde_json::to_string_pretty(&layout).unwrap().as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;

    // Write animations
    if !project.animations.is_empty() {
        zip_writer
            .start_file("animations.json", options)
            .map_err(|e| format!("Zip error: {e}"))?;
        zip_writer
            .write_all(
                serde_json::to_string_pretty(&project.animations)
                    .unwrap()
                    .as_bytes(),
            )
            .map_err(|e| format!("Write error: {e}"))?;
    }

    if !project.fonts.is_empty() {
        let fonts = serde_json::json!({
            "fonts": project.fonts,
        });
        zip_writer
            .start_file("fonts.json", options)
            .map_err(|e| format!("Zip error: {e}"))?;
        zip_writer
            .write_all(serde_json::to_string_pretty(&fonts).unwrap().as_bytes())
            .map_err(|e| format!("Write error: {e}"))?;
    }

    // Write texture data
    for (name, data) in &project.texture_data {
        zip_writer
            .start_file(name.as_str(), options)
            .map_err(|e| format!("Zip error for texture '{}': {e}", name))?;
        zip_writer
            .write_all(data)
            .map_err(|e| format!("Write error for texture '{}': {e}", name))?;
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;

    // Atomic rename
    std::fs::rename(&tmp_path, path).map_err(|e| format!("Failed to save file: {e}"))?;

    Ok(())
}

fn read_json_entry(archive: &mut ZipArchive<std::fs::File>, name: &str) -> Result<String, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|e| format!("Entry '{name}' not found in project: {e}"))?;
    let mut contents = String::new();
    entry
        .read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read '{name}': {e}"))?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{Animation, AnimationType};
    use crate::project::{
        CodegenMode, Element, ElementType, FillDirection, FontAsset, FontSource, GlyphInfo,
        GlyphMap, Group, Layer, ModTarget, Project, ProjectExportSettings, SemanticGroup,
        SemanticGroupKind, UvRect,
    };

    fn temp_project_path() -> String {
        std::env::temp_dir()
            .join(format!(
                "gui-crafter-round-trip-{}.mcgui",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn mcgui_round_trip_preserves_path_elements_groups_uv_visible_animation_and_textures() {
        let path = temp_project_path();
        let mut project = Project::new("Round Trip", 176, 166, ModTarget::NeoForge);
        project.project_path = Some(path.clone());
        project.elements.push(Element {
            id: "texture_1".to_string(),
            element_type: ElementType::Texture,
            x: 4,
            y: 5,
            width: Some(32),
            height: Some(16),
            size: None,
            asset: Some("textures/widget.png".to_string()),
            icon: None,
            icon_uv: None,
            tooltip: None,
            direction: Some(FillDirection::LeftToRight),
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: Some("fill_1".to_string()),
            visible: false,
            uv: Some(UvRect {
                x: 2,
                y: 3,
                width: 12,
                height: 10,
            }),
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
        });
        project.groups.push(Group {
            id: "group_1".to_string(),
            x: 1,
            y: 2,
            elements: vec!["texture_1".to_string()],
        });
        project.animations.push(Animation {
            id: "fill_1".to_string(),
            animation_type: AnimationType::Fill,
            data_key: "progress".to_string(),
            texture: None,
            direction: Some(FillDirection::LeftToRight),
            frame_count: None,
            fps: None,
            min_value: Some(0.0),
            max_value: Some(1.0),
            triggers_on: None,
        });
        project
            .texture_data
            .insert("textures/widget.png".to_string(), vec![137, 80, 78, 71]);

        save_to_mcgui(&project).unwrap();
        let loaded = load_from_mcgui(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.project_path, Some(path));
        assert!(!loaded.is_dirty);
        assert_eq!(loaded.elements, project.elements);
        assert_eq!(loaded.groups, project.groups);
        assert_eq!(loaded.animations, project.animations);
        assert!(loaded.fonts.is_empty());
        assert_eq!(
            loaded.texture_data.get("textures/widget.png"),
            Some(&vec![137, 80, 78, 71])
        );
    }

    #[test]
    fn mcgui_round_trip_preserves_ttf_fonts() {
        let path = temp_project_path();
        let mut project = Project::new("Fonts", 176, 166, ModTarget::Forge);
        project.project_path = Some(path.clone());
        let mut glyph_map = GlyphMap::new();
        glyph_map.insert(
            'A',
            GlyphInfo {
                x: 4,
                y: 5,
                width: 6,
                height: 7,
                ascent: 8,
                advance: 9,
                bearing_x: 1,
                bearing_y: -2,
            },
        );
        project.fonts.push(FontAsset {
            id: "custom".to_string(),
            source: FontSource::Ttf {
                atlas_png: vec![1, 2, 3, 4],
                font_size: 16,
                glyph_map,
            },
        });

        save_to_mcgui(&project).unwrap();
        let loaded = load_from_mcgui(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.fonts.len(), 1);
        assert_eq!(loaded.fonts[0].id, "custom");
        match &loaded.fonts[0].source {
            FontSource::Ttf {
                atlas_png,
                font_size,
                glyph_map,
            } => {
                assert_eq!(atlas_png, &vec![1, 2, 3, 4]);
                assert_eq!(*font_size, 16);
                let glyph = glyph_map.get(&'A').unwrap();
                assert_eq!(glyph.advance, 9);
                assert_eq!(glyph.bearing_x, 1);
                assert_eq!(glyph.bearing_y, -2);
            }
            FontSource::Minecraft { .. } => panic!("expected TTF font"),
        }
    }

    #[test]
    fn mcgui_round_trip_preserves_semantic_groups_and_export_settings() {
        let path = temp_project_path();
        let mut project = Project::new("Semantic", 176, 166, ModTarget::Forge);
        project.project_path = Some(path.clone());
        project.semantic_groups.push(SemanticGroup {
            id: "buffer".to_string(),
            kind: SemanticGroupKind::VirtualSlotGrid,
            columns: Some(9),
            visible_rows: Some(3),
            total_rows: Some(6),
            slot_count: Some(54),
            member_ids: Vec::new(),
            data_source: Some("machine_buffer".to_string()),
            scroll_binding: Some("buffer_scroll".to_string()),
            dynamic_height: true,
        });
        project.export_settings = ProjectExportSettings {
            codegen_mode: CodegenMode::Modular,
            generate_runtime_helpers: false,
            generate_semantic_registry: true,
        };

        save_to_mcgui(&project).unwrap();
        let loaded = load_from_mcgui(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.semantic_groups, project.semantic_groups);
        assert_eq!(loaded.export_settings, project.export_settings);
    }
}
