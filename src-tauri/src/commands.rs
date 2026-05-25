use crate::animation::Animation;
use crate::config::{AppConfig, EditorLayoutConfig, WindowConfig};
use crate::format;
use crate::project::{
    AssetMetadata, AttachedRegion, CodegenMode, Element, Group, ModTarget, Project,
    ProjectExportSettings, ProjectSessionSummary, SemanticGroup,
};
use crate::templates::TemplateInfo;
use crate::AppState;
use tauri::State;

const MAX_FONT_FILE_SIZE: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ElementMove {
    id: String,
    x: i32,
    y: i32,
}

#[tauri::command(rename_all = "snake_case")]
pub fn template_list() -> Vec<TemplateInfo> {
    crate::templates::list_template_info()
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_new(
    state: State<AppState>,
    name: String,
    width: u32,
    height: u32,
    mod_target: String,
    template: Option<String>,
) -> Result<serde_json::Value, String> {
    let target = parse_mod_target(&mod_target);

    let mut project = Project::new(&name, width, height, target);

    if let Some(tmpl) = template {
        crate::templates::apply_template(&mut project, &tmpl)?;
    } else {
        crate::templates::apply_generated_defaults(&mut project)?;
    }

    let mut sessions = state.sessions.lock().unwrap();
    let project_id = sessions.create_session(project);

    project_result(&sessions, &project_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn app_config_get() -> Result<AppConfig, String> {
    crate::config::load()
}

#[tauri::command(rename_all = "snake_case")]
pub fn editor_layout_save(layout: EditorLayoutConfig) -> Result<AppConfig, String> {
    let mut config = crate::config::load()?;
    config.editor_layout = Some(layout.clamped());
    crate::config::save(&config)?;
    Ok(config.clamped())
}

#[tauri::command(rename_all = "snake_case")]
pub fn app_window_save(window: WindowConfig) -> Result<AppConfig, String> {
    let mut config = crate::config::load()?;
    config.window = Some(window.clamped());
    crate::config::save(&config)?;
    Ok(config.clamped())
}

#[tauri::command(rename_all = "snake_case")]
pub fn ui_layout_reset(window: tauri::Window) -> Result<AppConfig, String> {
    let config = crate::config::load()?.with_reset_ui_layout();
    crate::config::save(&config)?;
    if let Some(window_config) = config.window.as_ref() {
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: window_config.width,
            height: window_config.height,
        }));
        let _ = window.center();
    }
    Ok(config.clamped())
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_open(state: State<AppState>, path: String) -> Result<serde_json::Value, String> {
    let project = format::load_from_mcgui(&path)?;
    let mut sessions = state.sessions.lock().unwrap();
    let project_id = sessions.create_session(project);

    project_result(&sessions, &project_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_save(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut sessions = state.sessions.lock().unwrap();
    save_session(&mut sessions, project_id.as_deref())
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_save_as(
    state: State<AppState>,
    project_id: Option<String>,
    path: String,
) -> Result<serde_json::Value, String> {
    let mut sessions = state.sessions.lock().unwrap();
    save_session_as(&mut sessions, project_id.as_deref(), path)
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_close(
    state: State<AppState>,
    project_id: String,
) -> Result<ProjectSessionSummary, String> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.close_session(&project_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_set_active(
    state: State<AppState>,
    project_id: String,
) -> Result<ProjectSessionSummary, String> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.set_active(&project_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_list_sessions(state: State<AppState>) -> Vec<ProjectSessionSummary> {
    let sessions = state.sessions.lock().unwrap();
    sessions.list_sessions()
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_get_active(state: State<AppState>) -> Result<serde_json::Value, String> {
    let sessions = state.sessions.lock().unwrap();
    let active = sessions.active_session()?;
    Ok(serde_json::json!({
        "summary": sessions.list_sessions().into_iter().find(|summary| summary.id == active.id),
        "project": active.project,
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_summary(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;

    Ok(serde_json::json!({
        "project_id": session.id,
        "name": session.project.name,
        "gui_size": session.project.gui_size,
        "mod_target": session.project.mod_target,
        "element_count": session.project.elements.len(),
        "is_dirty": session.project.is_dirty,
        "path": session.project.project_path,
        "revision": session.revision,
        "session": sessions.list_sessions().into_iter().find(|summary| summary.id == session.id),
    }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn project_export_settings_update(
    state: State<AppState>,
    settings: ProjectExportSettings,
    project_id: Option<String>,
) -> Result<ProjectExportSettings, String> {
    let mut sessions = state.sessions.lock().unwrap();
    update_export_settings_in_session(&mut sessions, project_id.as_deref(), settings)
}

#[tauri::command(rename_all = "camelCase")]
pub fn project_semantic_groups_update(
    state: State<AppState>,
    groups: Vec<SemanticGroup>,
    project_id: Option<String>,
) -> Result<Vec<SemanticGroup>, String> {
    let mut sessions = state.sessions.lock().unwrap();
    update_semantic_groups_in_session(&mut sessions, project_id.as_deref(), groups)
}

#[tauri::command(rename_all = "snake_case")]
pub fn element_add(
    state: State<AppState>,
    element: Element,
    project_id: Option<String>,
) -> Result<Element, String> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.record_history(project_id.as_deref())?;
    let session = sessions.resolve_mut(project_id.as_deref())?;

    session.project.add_element(element.clone());
    sessions.mark_changed(project_id.as_deref())?;
    Ok(element)
}

#[tauri::command(rename_all = "snake_case")]
pub fn element_move(
    state: State<AppState>,
    id: String,
    x: i32,
    y: i32,
    project_id: Option<String>,
) -> Result<Element, String> {
    let mut sessions = state.sessions.lock().unwrap();
    move_element_in_session(&mut sessions, project_id.as_deref(), &id, x, y)
}

#[tauri::command(rename_all = "snake_case")]
pub fn element_move_many(
    state: State<AppState>,
    moves: Vec<ElementMove>,
    project_id: Option<String>,
) -> Result<Vec<Element>, String> {
    let mut sessions = state.sessions.lock().unwrap();
    move_elements_in_session(&mut sessions, project_id.as_deref(), moves)
}

#[tauri::command(rename_all = "snake_case")]
pub fn element_update(
    state: State<AppState>,
    id: String,
    changes: serde_json::Value,
    project_id: Option<String>,
) -> Result<Element, String> {
    let mut sessions = state.sessions.lock().unwrap();
    update_element_in_session(&mut sessions, project_id.as_deref(), &id, changes)
}

#[tauri::command(rename_all = "snake_case")]
pub fn element_resize(
    state: State<AppState>,
    id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    project_id: Option<String>,
) -> Result<Element, String> {
    let mut sessions = state.sessions.lock().unwrap();
    resize_element_in_session(
        &mut sessions,
        project_id.as_deref(),
        &id,
        x,
        y,
        width,
        height,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn element_reorder(
    state: State<AppState>,
    id: String,
    index: usize,
    project_id: Option<String>,
) -> Result<ProjectSessionSummary, String> {
    let mut sessions = state.sessions.lock().unwrap();
    reorder_element_in_session(&mut sessions, project_id.as_deref(), &id, index)
}

#[tauri::command(rename_all = "snake_case")]
pub fn element_remove(
    state: State<AppState>,
    id: String,
    project_id: Option<String>,
) -> Result<bool, String> {
    let mut sessions = state.sessions.lock().unwrap();
    remove_element_from_session(&mut sessions, project_id.as_deref(), &id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn element_list(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<Vec<Element>, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;

    Ok(session.project.elements.clone())
}

#[tauri::command(rename_all = "snake_case")]
pub fn attached_region_create(
    state: State<AppState>,
    project_id: Option<String>,
    region: AttachedRegion,
) -> Result<AttachedRegion, String> {
    let mut sessions = state.sessions.lock().unwrap();
    create_attached_region_in_session(&mut sessions, project_id.as_deref(), region)
}

#[tauri::command(rename_all = "snake_case")]
pub fn attached_region_update(
    state: State<AppState>,
    project_id: Option<String>,
    id: String,
    changes: serde_json::Value,
) -> Result<AttachedRegion, String> {
    let mut sessions = state.sessions.lock().unwrap();
    update_attached_region_in_session(&mut sessions, project_id.as_deref(), id, changes)
}

#[tauri::command(rename_all = "snake_case")]
pub fn attached_region_remove(
    state: State<AppState>,
    project_id: Option<String>,
    id: String,
) -> Result<bool, String> {
    let mut sessions = state.sessions.lock().unwrap();
    remove_attached_region_in_session(&mut sessions, project_id.as_deref(), &id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn attached_region_list(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<Vec<AttachedRegion>, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;
    Ok(session.project.attached_regions.clone())
}

#[tauri::command(rename_all = "snake_case")]
pub fn attached_region_move_with_elements(
    state: State<AppState>,
    project_id: Option<String>,
    id: String,
    x: i32,
    y: i32,
) -> Result<AttachedRegion, String> {
    let mut sessions = state.sessions.lock().unwrap();
    move_attached_region_with_elements_in_session(&mut sessions, project_id.as_deref(), id, x, y)
}

#[tauri::command(rename_all = "snake_case")]
pub fn group_create(
    state: State<AppState>,
    element_ids: Vec<String>,
    group_id: Option<String>,
    project_id: Option<String>,
) -> Result<Group, String> {
    let mut sessions = state.sessions.lock().unwrap();
    create_group_in_session(&mut sessions, project_id.as_deref(), element_ids, group_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn group_ungroup(
    state: State<AppState>,
    group_id: String,
    project_id: Option<String>,
) -> Result<bool, String> {
    let mut sessions = state.sessions.lock().unwrap();
    ungroup_in_session(&mut sessions, project_id.as_deref(), &group_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn group_list(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<Vec<Group>, String> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;
    Ok(session.project.groups.clone())
}

#[tauri::command(rename_all = "snake_case")]
pub fn animation_create(
    state: State<AppState>,
    animation: Animation,
    project_id: Option<String>,
) -> Result<Animation, String> {
    let mut sessions = state.sessions.lock().unwrap();
    create_animation_in_session(&mut sessions, project_id.as_deref(), animation)
}

#[tauri::command(rename_all = "snake_case")]
pub fn animation_update(
    state: State<AppState>,
    id: String,
    changes: serde_json::Value,
    project_id: Option<String>,
) -> Result<Animation, String> {
    let mut sessions = state.sessions.lock().unwrap();
    update_animation_in_session(&mut sessions, project_id.as_deref(), &id, changes)
}

#[tauri::command(rename_all = "snake_case")]
pub fn animation_remove(
    state: State<AppState>,
    id: String,
    project_id: Option<String>,
) -> Result<bool, String> {
    let mut sessions = state.sessions.lock().unwrap();
    remove_animation_from_session(&mut sessions, project_id.as_deref(), &id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn animation_bind(
    state: State<AppState>,
    element_id: String,
    animation_id: String,
    project_id: Option<String>,
) -> Result<Element, String> {
    let mut sessions = state.sessions.lock().unwrap();
    bind_animation_in_session(
        &mut sessions,
        project_id.as_deref(),
        &element_id,
        &animation_id,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn animation_unbind(
    state: State<AppState>,
    element_id: String,
    project_id: Option<String>,
) -> Result<Element, String> {
    let mut sessions = state.sessions.lock().unwrap();
    unbind_animation_in_session(&mut sessions, project_id.as_deref(), &element_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn asset_import(
    state: State<AppState>,
    file_path: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    use std::io::Read;

    // Read file
    let mut file =
        std::fs::File::open(&file_path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    // Decode with image crate to get dimensions
    let img = image::load_from_memory(&data).map_err(|e| format!("Failed to decode image: {e}"))?;
    let (width, height) = (img.width(), img.height());

    // Generate asset name from filename
    let name = std::path::Path::new(&file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("texture");
    let asset_path = format!("textures/{name}.png");

    let data_url = data_url_png(&data);
    let bytes = data.len();
    let sha256 = sha256_hex(&data);

    let mut sessions = state.sessions.lock().unwrap();
    sessions.record_history(project_id.as_deref())?;
    let session = sessions.resolve_mut(project_id.as_deref())?;

    session
        .project
        .texture_data
        .insert(asset_path.clone(), data);
    if !session.project.assets.contains(&asset_path) {
        session.project.assets.push(asset_path.clone());
    }
    sessions.mark_changed(project_id.as_deref())?;

    Ok(serde_json::json!({
        "name": asset_path,
        "width": width,
        "height": height,
        "bytes": bytes,
        "sha256": sha256,
        "data_url": data_url,
        "nine_slice": serde_json::Value::Null,
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn asset_update(
    state: State<AppState>,
    name: String,
    data_url: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut sessions = state.sessions.lock().unwrap();
    update_asset_in_session(&mut sessions, project_id.as_deref(), &name, &data_url)
}

#[tauri::command(rename_all = "snake_case")]
pub fn asset_list(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let sessions = state.sessions.lock().unwrap();
    let project = &sessions.resolve(project_id.as_deref())?.project;

    Ok(project
        .assets
        .iter()
        .map(|name| {
            compact_asset_metadata(
                name,
                project.texture_data.get(name).map(Vec::as_slice),
                project.asset_metadata.get(name),
            )
        })
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub fn asset_metadata_update(
    state: State<'_, AppState>,
    name: String,
    metadata: AssetMetadata,
    project_id: Option<String>,
) -> Result<AssetMetadata, String> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.record_history(project_id.as_deref())?;
    let session = sessions.resolve_mut(project_id.as_deref())?;
    if !session.project.assets.iter().any(|asset| asset == &name) {
        return Err(format!("Asset not found: {name}"));
    }
    session
        .project
        .asset_metadata
        .insert(name, metadata.clone());
    sessions.mark_changed(project_id.as_deref())?;
    Ok(metadata)
}

#[tauri::command(rename_all = "snake_case")]
pub fn asset_remove(
    state: State<AppState>,
    name: String,
    project_id: Option<String>,
) -> Result<bool, String> {
    let mut sessions = state.sessions.lock().unwrap();
    remove_asset_from_session(&mut sessions, project_id.as_deref(), &name)
}

#[tauri::command(rename_all = "snake_case")]
pub fn asset_get_data_url(
    state: State<AppState>,
    name: String,
    project_id: Option<String>,
) -> Result<String, String> {
    let sessions = state.sessions.lock().unwrap();
    let project = &sessions.resolve(project_id.as_deref())?.project;

    let data = project
        .texture_data
        .get(&name)
        .ok_or(format!("Asset not found: {name}"))?;

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    Ok(format!("data:image/png;base64,{}", b64))
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_export_preview(
    state: State<AppState>,
    target: String,
    mod_id: String,
    package: String,
    class_name: String,
    output_dir: String,
    project_id: Option<String>,
    codegen_mode: Option<String>,
    generate_runtime_helpers: Option<bool>,
    generate_semantic_registry: Option<bool>,
    overwrite: Option<bool>,
) -> Result<crate::export::ExportPreview, String> {
    let sessions = state.sessions.lock().unwrap();
    let project = &sessions.resolve(project_id.as_deref())?.project;
    let settings_override = export_settings_override(
        project,
        codegen_mode,
        generate_runtime_helpers,
        generate_semantic_registry,
    )?;

    let config = crate::export::ExportConfig {
        mod_id,
        package,
        class_name,
        output_dir,
        settings_override,
        overwrite: overwrite.unwrap_or(false),
    };

    crate::export::preview_export(project, &config, &target)
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_export(
    state: State<AppState>,
    target: String,
    mod_id: String,
    package: String,
    class_name: String,
    output_dir: String,
    project_id: Option<String>,
    codegen_mode: Option<String>,
    generate_runtime_helpers: Option<bool>,
    generate_semantic_registry: Option<bool>,
    overwrite: Option<bool>,
) -> Result<Vec<String>, String> {
    let sessions = state.sessions.lock().unwrap();
    let project = &sessions.resolve(project_id.as_deref())?.project;
    let settings_override = export_settings_override(
        project,
        codegen_mode,
        generate_runtime_helpers,
        generate_semantic_registry,
    )?;

    let config = crate::export::ExportConfig {
        mod_id,
        package,
        class_name,
        output_dir,
        settings_override,
        overwrite: overwrite.unwrap_or(false),
    };

    crate::export::export_project(project, &config, &target)
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_undo(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<ProjectSessionSummary, String> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.undo(project_id.as_deref())
}

#[tauri::command(rename_all = "snake_case")]
pub fn project_redo(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<ProjectSessionSummary, String> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.redo(project_id.as_deref())
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_minecraft_sources() -> Vec<serde_json::Value> {
    let mut sources = Vec::new();
    let home = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();

    // Scan PrismLauncher instances
    let prism_path = home.join(".local/share/PrismLauncher/instances");
    if let Ok(entries) = std::fs::read_dir(&prism_path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                sources.push(serde_json::json!({
                    "name": entry.file_name().to_string_lossy(),
                    "path": entry.path().to_string_lossy(),
                    "source_type": "prismlauncher"
                }));
            }
        }
    }

    // Scan Gradle dev workspaces
    let dev_path = home.join("Development/minecraft");
    if let Ok(entries) = std::fs::read_dir(&dev_path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                sources.push(serde_json::json!({
                    "name": entry.file_name().to_string_lossy(),
                    "path": entry.path().to_string_lossy(),
                    "source_type": "gradle_dev"
                }));
            }
        }
    }

    sources
}

fn data_url_png(data: &[u8]) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    format!("data:image/png;base64,{b64}")
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(data))
}

fn compact_asset_metadata(
    name: &str,
    data: Option<&[u8]>,
    metadata: Option<&AssetMetadata>,
) -> serde_json::Value {
    let image = data.and_then(|data| image::load_from_memory(data).ok());
    let width = image
        .as_ref()
        .map(|image| image.width())
        .or_else(|| metadata.and_then(|metadata| metadata.width))
        .unwrap_or(16);
    let height = image
        .as_ref()
        .map(|image| image.height())
        .or_else(|| metadata.and_then(|metadata| metadata.height))
        .unwrap_or(16);
    let bytes = data.map_or(0, |data| data.len());
    let sha256 = data.map_or_else(|| sha256_hex(&[]), sha256_hex);
    let nine_slice = metadata.and_then(|metadata| metadata.nine_slice.clone());

    serde_json::json!({
        "name": name,
        "width": width,
        "height": height,
        "bytes": bytes,
        "sha256": sha256,
        "nine_slice": nine_slice,
    })
}

fn font_render_data_json(font: &crate::project::FontAsset) -> serde_json::Value {
    match &font.source {
        crate::project::FontSource::Minecraft {
            providers,
            glyph_map,
        } => serde_json::json!({
            "id": font.id,
            "source_type": "minecraft",
            "providers": providers.iter().map(|provider| {
                serde_json::json!({
                    "file": provider.file,
                    "ascent": provider.ascent,
                    "chars": provider.chars,
                    "image_width": provider.image_width,
                    "image_height": provider.image_height,
                    "image_data_url": data_url_png(&provider.image_data),
                })
            }).collect::<Vec<_>>(),
            "glyph_map": glyph_map,
        }),
        crate::project::FontSource::Ttf {
            atlas_png,
            font_size,
            glyph_map,
        } => serde_json::json!({
            "id": font.id,
            "source_type": "ttf",
            "font_size": font_size,
            "atlas_data_url": data_url_png(atlas_png),
            "glyph_map": glyph_map,
        }),
    }
}

fn font_list_json(project: &Project) -> Vec<serde_json::Value> {
    let mut fonts = vec![serde_json::json!({
        "id": "minecraft:default",
        "source": { "type": "minecraft" }
    })];

    fonts.extend(
        project
            .fonts
            .iter()
            .filter(|font| font.id != "minecraft:default")
            .map(|font| {
                let source_type = match &font.source {
                    crate::project::FontSource::Minecraft { .. } => "minecraft",
                    crate::project::FontSource::Ttf { .. } => "ttf",
                };
                serde_json::json!({
                    "id": font.id,
                    "source": { "type": source_type }
                })
            }),
    );

    fonts
}

fn validate_font_file_size(size: u64) -> Result<(), String> {
    if size > MAX_FONT_FILE_SIZE {
        return Err("Font file is too large; maximum supported size is 16 MiB".to_string());
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn font_import(
    state: State<AppState>,
    file_path: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    use std::io::Read;

    let metadata = std::fs::metadata(&file_path)
        .map_err(|e| format!("Failed to inspect font file metadata: {e}"))?;
    validate_font_file_size(metadata.len())?;

    let mut file =
        std::fs::File::open(&file_path).map_err(|e| format!("Failed to open font file: {e}"))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| format!("Failed to read font file: {e}"))?;

    let ext = std::path::Path::new(&file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let font_id = std::path::Path::new(&file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported_font");

    let font_asset = match ext.as_str() {
        "ttf" | "otf" => crate::font::rasterizer::rasterize_ttf(&data, 16, font_id)
            .map_err(|e| format!("Failed to rasterize font: {e}"))?,
        _ => return Err(format!("Unsupported font format: .{ext}. Use .ttf or .otf")),
    };

    let mut sessions = state.sessions.lock().unwrap();
    sessions.record_history(project_id.as_deref())?;
    let session = sessions.resolve_mut(project_id.as_deref())?;

    // Replace existing font with same ID, or add new
    session.project.fonts.retain(|f| f.id != font_asset.id);
    session.project.fonts.push(font_asset.clone());
    sessions.mark_changed(project_id.as_deref())?;

    Ok(serde_json::to_value(&font_asset).unwrap_or_default())
}

#[tauri::command(rename_all = "snake_case")]
pub fn font_list(
    state: State<AppState>,
    project_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let sessions = state.sessions.lock().unwrap();
    let project = &sessions.resolve(project_id.as_deref())?.project;

    Ok(font_list_json(project))
}

#[tauri::command(rename_all = "snake_case")]
pub fn font_glyph_map(
    state: State<AppState>,
    font_id: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    if font_id == "minecraft:default" {
        let font = crate::font::load_bundled_font();
        if let crate::project::FontSource::Minecraft { glyph_map, .. } = font.source {
            return serde_json::to_value(glyph_map)
                .map_err(|e| format!("Failed to serialize glyph map: {e}"));
        }
    }

    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;

    let font = session
        .project
        .fonts
        .iter()
        .find(|f| f.id == font_id)
        .ok_or_else(|| format!("Font not found: {font_id}"))?;

    let glyph_map = match &font.source {
        crate::project::FontSource::Minecraft { glyph_map, .. } => glyph_map,
        crate::project::FontSource::Ttf { glyph_map, .. } => glyph_map,
    };

    serde_json::to_value(glyph_map).map_err(|e| format!("Failed to serialize glyph map: {e}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn font_render_data(
    state: State<AppState>,
    font_id: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    if font_id == "minecraft:default" {
        let font = crate::font::load_bundled_font();
        return Ok(font_render_data_json(&font));
    }

    let sessions = state.sessions.lock().unwrap();
    let session = sessions.resolve(project_id.as_deref())?;
    let font = session
        .project
        .fonts
        .iter()
        .find(|font| font.id == font_id)
        .ok_or_else(|| format!("Font not found: {font_id}"))?;

    Ok(font_render_data_json(font))
}

fn parse_mod_target(mod_target: &str) -> ModTarget {
    match mod_target {
        "fabric" | "Fabric" => ModTarget::Fabric,
        "neoforge" | "NeoForge" => ModTarget::NeoForge,
        _ => ModTarget::Forge,
    }
}

fn update_export_settings_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    settings: ProjectExportSettings,
) -> Result<ProjectExportSettings, String> {
    let settings = settings.normalized();
    let current = &sessions.resolve(project_id)?.project.export_settings;
    if current == &settings {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    sessions.resolve_mut(project_id)?.project.export_settings = settings.clone();
    sessions.mark_changed(project_id)?;
    Ok(settings)
}

fn export_settings_override(
    project: &Project,
    codegen_mode: Option<String>,
    generate_runtime_helpers: Option<bool>,
    generate_semantic_registry: Option<bool>,
) -> Result<Option<ProjectExportSettings>, String> {
    if codegen_mode.is_none()
        && generate_runtime_helpers.is_none()
        && generate_semantic_registry.is_none()
    {
        return Ok(None);
    }

    let mut settings = project.export_settings.clone();
    if let Some(mode) = codegen_mode {
        settings.codegen_mode = match mode.as_str() {
            "simple" => CodegenMode::Simple,
            "modular" => CodegenMode::Modular,
            other => return Err(format!("Unknown codegen_mode: {other}")),
        };
    }
    if let Some(value) = generate_runtime_helpers {
        settings.generate_runtime_helpers = value;
    }
    if let Some(value) = generate_semantic_registry {
        settings.generate_semantic_registry = value;
    }

    Ok(Some(settings.normalized()))
}

fn update_semantic_groups_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    groups: Vec<SemanticGroup>,
) -> Result<Vec<SemanticGroup>, String> {
    let current = &sessions.resolve(project_id)?.project.semantic_groups;
    if current == &groups {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    sessions.resolve_mut(project_id)?.project.semantic_groups = groups.clone();
    sessions.mark_changed(project_id)?;
    Ok(groups)
}

fn move_element_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: &str,
    x: i32,
    y: i32,
) -> Result<Element, String> {
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .ok_or("Element not found")?;
    if current.x == x && current.y == y {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = {
        let element = session
            .project
            .find_element_mut(id)
            .ok_or("Element not found")?;
        element.x = x;
        element.y = y;
        element.clone()
    };
    refresh_group_positions_for_elements(&mut session.project, &[id.to_string()]);
    sessions.mark_changed(project_id)?;

    Ok(element)
}

fn move_elements_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    moves: Vec<ElementMove>,
) -> Result<Vec<Element>, String> {
    if moves.is_empty() {
        return Ok(Vec::new());
    }

    let project = &sessions.resolve(project_id)?.project;
    let mut ids = Vec::with_capacity(moves.len());
    let mut changed = false;
    for element_move in &moves {
        if ids.contains(&element_move.id.as_str()) {
            return Err(format!("Duplicate element move: {}", element_move.id));
        }
        ids.push(element_move.id.as_str());

        let current = project
            .find_element(&element_move.id)
            .ok_or_else(|| format!("Element not found: {}", element_move.id))?;
        changed |= current.x != element_move.x || current.y != element_move.y;
    }

    if !changed {
        return moves
            .iter()
            .map(|element_move| {
                project
                    .find_element(&element_move.id)
                    .cloned()
                    .ok_or_else(|| format!("Element not found: {}", element_move.id))
            })
            .collect();
    }

    let move_ids = moves
        .iter()
        .map(|element_move| element_move.id.clone())
        .collect::<Vec<_>>();

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let mut moved = Vec::with_capacity(moves.len());
    for element_move in moves {
        let element = session
            .project
            .find_element_mut(&element_move.id)
            .ok_or_else(|| format!("Element not found: {}", element_move.id))?;
        element.x = element_move.x;
        element.y = element_move.y;
        moved.push(element.clone());
    }
    refresh_group_positions_for_elements(&mut session.project, &move_ids);
    sessions.mark_changed(project_id)?;

    Ok(moved)
}

fn refresh_group_positions_for_elements(project: &mut Project, moved_ids: &[String]) {
    if moved_ids.is_empty() {
        return;
    }

    let elements = &project.elements;
    for group in &mut project.groups {
        if !group
            .elements
            .iter()
            .any(|element_id| moved_ids.iter().any(|moved_id| moved_id == element_id))
        {
            continue;
        }

        let mut positions = group.elements.iter().filter_map(|element_id| {
            elements
                .iter()
                .find(|element| element.id == *element_id)
                .map(|element| (element.x, element.y))
        });
        if let Some((mut min_x, mut min_y)) = positions.next() {
            for (x, y) in positions {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
            }
            group.x = min_x;
            group.y = min_y;
        }
    }
}

fn remove_element_from_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: &str,
) -> Result<bool, String> {
    if sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .is_none()
    {
        return Ok(false);
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let removed = session.project.remove_element(id).is_some();
    if removed {
        sessions.mark_changed(project_id)?;
    }
    Ok(removed)
}

fn update_element_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: &str,
    changes: serde_json::Value,
) -> Result<Element, String> {
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .ok_or("Element not found")?;
    let updated = apply_element_changes(current, changes)?;
    if &updated == current {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session
        .project
        .find_element_mut(id)
        .ok_or("Element not found")?;
    *element = updated.clone();
    sessions.mark_changed(project_id)?;
    Ok(updated)
}

fn apply_element_changes(element: &Element, changes: serde_json::Value) -> Result<Element, String> {
    let mut value = serde_json::to_value(element)
        .map_err(|error| format!("Failed to encode element: {error}"))?;
    let object = changes
        .as_object()
        .ok_or("Element changes must be an object")?;

    if object
        .get("id")
        .is_some_and(|value| value.as_str() != Some(element.id.as_str()))
    {
        return Err("Element id cannot be changed".to_string());
    }
    if object.get("type").is_some() {
        return Err("Element type cannot be changed".to_string());
    }

    let target = value
        .as_object_mut()
        .ok_or("Element payload must be an object")?;
    for (key, new_value) in object {
        if key == "id" || key == "type" {
            continue;
        }
        target.insert(key.clone(), new_value.clone());
    }

    serde_json::from_value(value).map_err(|error| format!("Invalid element update: {error}"))
}

fn create_attached_region_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    region: AttachedRegion,
) -> Result<AttachedRegion, String> {
    let project = &sessions.resolve(project_id)?.project;
    if project.find_attached_region(&region.id).is_some() {
        return Err(format!("Attached region already exists: {}", region.id));
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.attached_regions.push(region.clone());
    sessions.mark_changed(project_id)?;
    Ok(region)
}

fn update_attached_region_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: String,
    changes: serde_json::Value,
) -> Result<AttachedRegion, String> {
    let current = sessions
        .resolve(project_id)?
        .project
        .find_attached_region(&id)
        .ok_or_else(|| format!("Attached region not found: {id}"))?;
    let updated = apply_attached_region_changes(current, changes)?;
    if updated == *current {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let region = session
        .project
        .find_attached_region_mut(&id)
        .ok_or_else(|| format!("Attached region not found: {id}"))?;
    *region = updated.clone();
    sessions.mark_changed(project_id)?;
    Ok(updated)
}

fn apply_attached_region_changes(
    region: &AttachedRegion,
    changes: serde_json::Value,
) -> Result<AttachedRegion, String> {
    let mut value = serde_json::to_value(region)
        .map_err(|error| format!("Failed to encode attached region: {error}"))?;
    let object = changes
        .as_object()
        .ok_or("Attached region changes must be an object")?;
    if object
        .get("id")
        .is_some_and(|value| value.as_str() != Some(region.id.as_str()))
    {
        return Err("Attached region id cannot be changed".to_string());
    }

    let target = value
        .as_object_mut()
        .ok_or("Attached region payload must be an object")?;
    for (key, new_value) in object {
        if key == "id" {
            continue;
        }
        target.insert(key.clone(), new_value.clone());
    }

    serde_json::from_value(value)
        .map_err(|error| format!("Invalid attached region update: {error}"))
}

fn remove_attached_region_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: &str,
) -> Result<bool, String> {
    let project = &sessions.resolve(project_id)?.project;
    if project.find_attached_region(id).is_none() {
        return Ok(false);
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session
        .project
        .attached_regions
        .retain(|region| region.id != id);
    for element in &mut session.project.elements {
        if element.attached_region.as_deref() == Some(id) {
            element.attached_region = None;
        }
    }
    sessions.mark_changed(project_id)?;
    Ok(true)
}

fn move_attached_region_with_elements_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: String,
    x: i32,
    y: i32,
) -> Result<AttachedRegion, String> {
    let project = &sessions.resolve(project_id)?.project;
    let current = project
        .find_attached_region(&id)
        .ok_or_else(|| format!("Attached region not found: {id}"))?;
    if current.x == x && current.y == y {
        return Ok(current.clone());
    }

    let dx = x
        .checked_sub(current.x)
        .ok_or("Attached region move overflow")?;
    let dy = y
        .checked_sub(current.y)
        .ok_or("Attached region move overflow")?;
    let moved_child_ids = project
        .elements
        .iter()
        .filter(|element| element.attached_region.as_deref() == Some(id.as_str()))
        .map(|element| {
            element
                .x
                .checked_add(dx)
                .ok_or("Attached region child move overflow")?;
            element
                .y
                .checked_add(dy)
                .ok_or("Attached region child move overflow")?;
            Ok(element.id.clone())
        })
        .collect::<Result<Vec<_>, String>>()?;

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let updated = {
        let region = session
            .project
            .find_attached_region_mut(&id)
            .ok_or_else(|| format!("Attached region not found: {id}"))?;
        region.x = x;
        region.y = y;
        region.clone()
    };
    for element in &mut session.project.elements {
        if element.attached_region.as_deref() == Some(id.as_str()) {
            element.x = element
                .x
                .checked_add(dx)
                .ok_or("Attached region child move overflow")?;
            element.y = element
                .y
                .checked_add(dy)
                .ok_or("Attached region child move overflow")?;
        }
    }
    refresh_group_positions_for_elements(&mut session.project, &moved_child_ids);
    sessions.mark_changed(project_id)?;
    Ok(updated)
}

fn resize_element_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<Element, String> {
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(id)
        .ok_or("Element not found")?;
    let mut updated = current.clone();
    updated.x = x;
    updated.y = y;
    if updated.element_type == crate::project::ElementType::Slot {
        updated.size = Some(width.max(height).max(8));
    } else {
        updated.width = Some(width.max(4));
        updated.height = Some(height.max(4));
    }
    if &updated == current {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session
        .project
        .find_element_mut(id)
        .ok_or("Element not found")?;
    *element = updated.clone();
    sessions.mark_changed(project_id)?;
    Ok(updated)
}

fn reorder_element_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: &str,
    index: usize,
) -> Result<ProjectSessionSummary, String> {
    let session = sessions.resolve(project_id)?;
    let current_index = session
        .project
        .elements
        .iter()
        .position(|element| element.id == id)
        .ok_or("Element not found")?;
    let target_index = index.min(session.project.elements.len().saturating_sub(1));
    if current_index == target_index {
        return session_summary(sessions, &session.id);
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session.project.elements.remove(current_index);
    session.project.elements.insert(target_index, element);
    sessions.mark_changed(project_id)
}

fn create_group_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    element_ids: Vec<String>,
    group_id: Option<String>,
) -> Result<Group, String> {
    let id = group_id.unwrap_or_else(|| format!("group_{}", uuid::Uuid::new_v4()));

    {
        let project = &sessions.resolve(project_id)?.project;
        if project.groups.iter().any(|group| group.id == id) {
            return Err("Group already exists".to_string());
        }
        let mut unique_count = 0usize;
        let mut unique_ids: Vec<&String> = Vec::new();
        for element_id in &element_ids {
            if !unique_ids.contains(&element_id) {
                unique_count += 1;
                unique_ids.push(element_id);
            }
            if project.find_element(element_id).is_none() {
                return Err(format!("Element not found: {element_id}"));
            }
        }
        if unique_count < 2 {
            return Err("At least two elements are required to create a group".to_string());
        }
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let group = session.project.group_elements(id, element_ids)?;
    sessions.mark_changed(project_id)?;
    Ok(group)
}

fn ungroup_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    group_id: &str,
) -> Result<bool, String> {
    if !sessions
        .resolve(project_id)?
        .project
        .groups
        .iter()
        .any(|group| group.id == group_id)
    {
        return Ok(false);
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let removed = session.project.ungroup(group_id);
    if removed {
        sessions.mark_changed(project_id)?;
    }
    Ok(removed)
}

fn create_animation_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    animation: Animation,
) -> Result<Animation, String> {
    let project = &sessions.resolve(project_id)?.project;
    if project
        .animations
        .iter()
        .any(|existing| existing.id == animation.id)
    {
        return Err("Animation already exists".to_string());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.animations.push(animation.clone());
    sessions.mark_changed(project_id)?;
    Ok(animation)
}

fn update_animation_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: &str,
    changes: serde_json::Value,
) -> Result<Animation, String> {
    let current = sessions
        .resolve(project_id)?
        .project
        .animations
        .iter()
        .find(|animation| animation.id == id)
        .ok_or("Animation not found")?;
    let updated = apply_animation_changes(current, changes)?;
    if &updated == current {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let animation = session
        .project
        .animations
        .iter_mut()
        .find(|animation| animation.id == id)
        .ok_or("Animation not found")?;
    *animation = updated.clone();
    sessions.mark_changed(project_id)?;
    Ok(updated)
}

fn apply_animation_changes(
    animation: &Animation,
    changes: serde_json::Value,
) -> Result<Animation, String> {
    let mut value = serde_json::to_value(animation)
        .map_err(|error| format!("Failed to encode animation: {error}"))?;
    let object = changes
        .as_object()
        .ok_or("Animation changes must be an object")?;
    if object
        .get("id")
        .is_some_and(|value| value.as_str() != Some(animation.id.as_str()))
    {
        return Err("Animation id cannot be changed".to_string());
    }
    if object.get("type").is_some() {
        return Err("Animation type cannot be changed".to_string());
    }

    let target = value
        .as_object_mut()
        .ok_or("Animation payload must be an object")?;
    for (key, new_value) in object {
        if key == "id" || key == "type" {
            continue;
        }
        target.insert(key.clone(), new_value.clone());
    }

    serde_json::from_value(value).map_err(|error| format!("Invalid animation update: {error}"))
}

fn remove_animation_from_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    id: &str,
) -> Result<bool, String> {
    let project = &sessions.resolve(project_id)?.project;
    if !project
        .animations
        .iter()
        .any(|animation| animation.id == id)
    {
        return Ok(false);
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session
        .project
        .animations
        .retain(|animation| animation.id != id);
    for element in &mut session.project.elements {
        if element.animation.as_deref() == Some(id) {
            element.animation = None;
        }
    }
    sessions.mark_changed(project_id)?;
    Ok(true)
}

fn bind_animation_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    element_id: &str,
    animation_id: &str,
) -> Result<Element, String> {
    let project = &sessions.resolve(project_id)?.project;
    if !project
        .animations
        .iter()
        .any(|animation| animation.id == animation_id)
    {
        return Err("Animation not found".to_string());
    }
    let current = project
        .find_element(element_id)
        .ok_or("Element not found")?;
    if current.animation.as_deref() == Some(animation_id) {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session
        .project
        .find_element_mut(element_id)
        .ok_or("Element not found")?;
    element.animation = Some(animation_id.to_string());
    let element = element.clone();
    sessions.mark_changed(project_id)?;
    Ok(element)
}

fn unbind_animation_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    element_id: &str,
) -> Result<Element, String> {
    let current = sessions
        .resolve(project_id)?
        .project
        .find_element(element_id)
        .ok_or("Element not found")?;
    if current.animation.is_none() {
        return Ok(current.clone());
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let element = session
        .project
        .find_element_mut(element_id)
        .ok_or("Element not found")?;
    element.animation = None;
    let element = element.clone();
    sessions.mark_changed(project_id)?;
    Ok(element)
}

fn remove_asset_from_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    name: &str,
) -> Result<bool, String> {
    let exists = {
        let project = &sessions.resolve(project_id)?.project;
        project.assets.iter().any(|asset| asset == name) || project.texture_data.contains_key(name)
    };
    if !exists {
        return Ok(false);
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    let removed_texture = session.project.texture_data.remove(name).is_some();
    let removed_metadata = session.project.asset_metadata.remove(name).is_some();
    let old_len = session.project.assets.len();
    session.project.assets.retain(|asset| asset != name);
    let removed_asset = session.project.assets.len() != old_len;
    if removed_texture || removed_metadata || removed_asset {
        sessions.mark_changed(project_id)?;
    }

    Ok(removed_texture || removed_metadata || removed_asset)
}

fn update_asset_in_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    name: &str,
    data_url: &str,
) -> Result<serde_json::Value, String> {
    let data = decode_png_data_url(data_url)?;
    if image::guess_format(&data).map_err(|e| format!("Failed to detect image format: {e}"))?
        != image::ImageFormat::Png
    {
        return Err("Invalid asset image: expected PNG bytes".to_string());
    }
    let img = image::load_from_memory(&data).map_err(|e| format!("Failed to decode PNG: {e}"))?;
    let (width, height) = (img.width(), img.height());
    let bytes = data.len();
    let sha256 = sha256_hex(&data);

    {
        let project = &sessions.resolve(project_id)?.project;
        if !project.assets.iter().any(|asset| asset == name) {
            return Err(format!("Asset not found: {name}"));
        }
    }

    {
        let project = &sessions.resolve(project_id)?.project;
        if project
            .texture_data
            .get(name)
            .is_some_and(|current| current == &data)
        {
            return Ok(serde_json::json!({
                "name": name,
                "width": width,
                "height": height,
                "bytes": bytes,
                "sha256": sha256,
                "data_url": data_url,
                "nine_slice": project.asset_metadata.get(name).and_then(|metadata| metadata.nine_slice.clone()),
            }));
        }
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    session.project.texture_data.insert(name.to_string(), data);
    let nine_slice = session
        .project
        .asset_metadata
        .get(name)
        .and_then(|metadata| metadata.nine_slice.clone());
    sessions.mark_changed(project_id)?;

    Ok(serde_json::json!({
        "name": name,
        "width": width,
        "height": height,
        "bytes": bytes,
        "sha256": sha256,
        "data_url": data_url,
        "nine_slice": nine_slice,
    }))
}

fn decode_png_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let Some(payload) = data_url.strip_prefix("data:image/png;base64,") else {
        return Err("Invalid asset data URL: expected data:image/png;base64,...".to_string());
    };
    if payload.trim().is_empty() {
        return Err("Invalid asset data URL: missing PNG base64 payload".to_string());
    }

    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| format!("Invalid PNG base64 payload: {e}"))
}

fn save_session(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve_mut(project_id)?;
    format::save_to_mcgui(&session.project)?;
    session.project.is_dirty = false;

    Ok(serde_json::json!({
        "project_id": session.id,
        "status": "saved",
        "path": session.project.project_path,
        "is_dirty": false
    }))
}

fn save_session_as(
    sessions: &mut crate::project::ProjectSessionManager,
    project_id: Option<&str>,
    path: String,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve_mut(project_id)?;
    let previous_path = session.project.project_path.clone();
    session.project.project_path = Some(path.clone());
    if let Err(error) = format::save_to_mcgui(&session.project) {
        session.project.project_path = previous_path;
        return Err(error);
    }
    session.project.is_dirty = false;

    Ok(serde_json::json!({
        "project_id": session.id,
        "status": "saved",
        "path": path,
        "is_dirty": false
    }))
}

fn session_summary(
    sessions: &crate::project::ProjectSessionManager,
    project_id: &str,
) -> Result<ProjectSessionSummary, String> {
    sessions
        .list_sessions()
        .into_iter()
        .find(|summary| summary.id == project_id)
        .ok_or("Project session not found".to_string())
}

fn project_result(
    sessions: &crate::project::ProjectSessionManager,
    project_id: &str,
) -> Result<serde_json::Value, String> {
    let session = sessions.resolve(Some(project_id))?;
    let summary = session_summary(sessions, project_id)?;

    Ok(serde_json::json!({
        "project_id": summary.id,
        "name": &session.project.name,
        "gui_size": &session.project.gui_size,
        "mod_target": &session.project.mod_target,
        "path": &session.project.project_path,
        "element_count": session.project.elements.len(),
        "is_dirty": session.project.is_dirty,
        "revision": session.revision,
        "session": summary,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{Animation, AnimationType};
    use crate::project::ProjectSessionManager;

    fn sample_element(id: &str, x: i32, y: i32) -> Element {
        Element {
            id: id.to_string(),
            element_type: crate::project::ElementType::Slot,
            x,
            y,
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
            layer: crate::project::Layer::Background,
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

    fn sample_attached_region(id: &str, x: i32, y: i32) -> AttachedRegion {
        AttachedRegion {
            id: id.to_string(),
            anchor: crate::project::AttachedRegionAnchor::Right,
            x,
            y,
            width: 54,
            height: 72,
            state: crate::project::AttachedRegionState::Static,
            kind: Some(id.to_string()),
            semantic_group: Some("food_returns".to_string()),
            visible: true,
        }
    }

    fn seed_attached_region_redo_session() -> (ProjectSessionManager, String) {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Attached Regions", 176, 166, ModTarget::Forge));
        sessions.record_history(Some(&project_id)).unwrap();
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .attached_regions
            .push(sample_attached_region("returns_pocket", 100, 18));
        sessions.mark_changed(Some(&project_id)).unwrap();
        sessions.undo(Some(&project_id)).unwrap();
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .attached_regions
            .push(sample_attached_region("returns_pocket", 100, 18));
        (sessions, project_id)
    }

    fn png_data_url(color: [u8; 4]) -> String {
        use base64::Engine;
        use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
        use std::io::Cursor;

        let image = ImageBuffer::from_pixel(1, 1, Rgba(color));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
        )
    }

    #[test]
    fn save_session_as_sets_path_and_clears_dirty() {
        let path = std::env::temp_dir()
            .join(format!(
                "gui-crafter-save-as-{}.mcgui",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned();
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Save As", 176, 166, ModTarget::Forge));

        let result = save_session_as(&mut sessions, Some(&project_id), path.clone()).unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(result["project_id"], project_id);
        assert_eq!(result["path"], path);
        assert_eq!(session.project.project_path, Some(path));
        assert!(!session.project.is_dirty);
    }

    #[test]
    fn save_session_as_restores_previous_path_when_save_fails() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Save As", 176, 166, ModTarget::Forge));
        let previous_path = Some(
            std::env::temp_dir()
                .join(format!(
                    "gui-crafter-existing-{}.mcgui",
                    uuid::Uuid::new_v4()
                ))
                .to_string_lossy()
                .into_owned(),
        );
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .project_path = previous_path.clone();
        let invalid_path = std::env::temp_dir()
            .join(format!("gui-crafter-missing-{}", uuid::Uuid::new_v4()))
            .join("project.mcgui")
            .to_string_lossy()
            .into_owned();

        let result = save_session_as(&mut sessions, Some(&project_id), invalid_path);

        assert!(result.is_err());
        assert_eq!(
            sessions
                .resolve(Some(&project_id))
                .unwrap()
                .project
                .project_path,
            previous_path
        );
    }

    #[test]
    fn export_settings_update_normalizes_mode_and_records_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Export Settings", 176, 166, ModTarget::Forge));

        let updated = update_export_settings_in_session(
            &mut sessions,
            Some(&project_id),
            ProjectExportSettings {
                codegen_mode: CodegenMode::Modular,
                generate_runtime_helpers: true,
                generate_semantic_registry: false,
            },
        )
        .unwrap();

        assert_eq!(updated.codegen_mode, CodegenMode::Modular);
        assert!(updated.generate_semantic_registry);
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 1);
        assert!(session_summary(&sessions, &project_id).unwrap().can_undo);

        let updated = update_export_settings_in_session(
            &mut sessions,
            Some(&project_id),
            ProjectExportSettings {
                codegen_mode: CodegenMode::Simple,
                generate_runtime_helpers: false,
                generate_semantic_registry: true,
            },
        )
        .unwrap();

        assert_eq!(updated.codegen_mode, CodegenMode::Simple);
        assert!(!updated.generate_semantic_registry);
        assert!(!updated.generate_runtime_helpers);
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 2);
    }

    #[test]
    fn export_settings_noop_preserves_redo() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Export Settings", 176, 166, ModTarget::Forge));
        update_export_settings_in_session(
            &mut sessions,
            Some(&project_id),
            ProjectExportSettings {
                codegen_mode: CodegenMode::Modular,
                generate_runtime_helpers: true,
                generate_semantic_registry: true,
            },
        )
        .unwrap();
        sessions.undo(Some(&project_id)).unwrap();

        let unchanged = update_export_settings_in_session(
            &mut sessions,
            Some(&project_id),
            ProjectExportSettings::default(),
        )
        .unwrap();

        assert_eq!(unchanged, ProjectExportSettings::default());
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn export_settings_override_applies_supplied_fields_and_normalizes() {
        let mut project = Project::new("Export Override", 176, 166, ModTarget::Forge);
        project.export_settings = ProjectExportSettings {
            codegen_mode: CodegenMode::Simple,
            generate_runtime_helpers: true,
            generate_semantic_registry: false,
        };

        let override_settings = export_settings_override(
            &project,
            Some("modular".to_string()),
            Some(false),
            Some(false),
        )
        .unwrap()
        .unwrap();

        assert_eq!(override_settings.codegen_mode, CodegenMode::Modular);
        assert!(!override_settings.generate_runtime_helpers);
        assert!(override_settings.generate_semantic_registry);
        assert_eq!(project.export_settings.codegen_mode, CodegenMode::Simple);
    }

    #[test]
    fn export_settings_override_returns_none_when_no_fields_supplied() {
        let project = Project::new("Export Override", 176, 166, ModTarget::Forge);

        let override_settings =
            export_settings_override(&project, None, None, None).expect("override parsing failed");

        assert_eq!(override_settings, None);
    }

    #[test]
    fn export_settings_override_rejects_unknown_codegen_mode() {
        let project = Project::new("Export Override", 176, 166, ModTarget::Forge);

        let error = export_settings_override(&project, Some("split".to_string()), None, None)
            .expect_err("unknown codegen mode should fail");

        assert_eq!(error, "Unknown codegen_mode: split");
    }

    #[test]
    fn semantic_groups_update_records_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Semantic Groups", 176, 166, ModTarget::Forge));
        let groups = vec![SemanticGroup {
            id: "virtual_storage".to_string(),
            kind: crate::project::SemanticGroupKind::VirtualSlotGrid,
            columns: Some(9),
            visible_rows: Some(3),
            total_rows: Some(6),
            slot_count: Some(54),
            member_ids: Vec::new(),
            data_source: Some("storage".to_string()),
            scroll_binding: Some("storage_scroll".to_string()),
            dynamic_height: true,
        }];

        let updated =
            update_semantic_groups_in_session(&mut sessions, Some(&project_id), groups.clone())
                .unwrap();

        assert_eq!(updated, groups);
        assert_eq!(
            sessions
                .resolve(Some(&project_id))
                .unwrap()
                .project
                .semantic_groups,
            groups
        );
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert_eq!(summary.revision, 1);
        assert!(summary.can_undo);
    }

    #[test]
    fn element_move_missing_element_keeps_history_and_redo() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("History", 176, 166, ModTarget::Forge));
        sessions.record_history(Some(&project_id)).unwrap();
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1", 8, 18));
        sessions.mark_changed(Some(&project_id)).unwrap();
        sessions.undo(Some(&project_id)).unwrap();

        let result = move_element_in_session(&mut sessions, Some(&project_id), "missing", 10, 20);

        let summary = sessions
            .list_sessions()
            .into_iter()
            .find(|summary| summary.id == project_id)
            .unwrap();
        assert!(result.is_err());
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn element_remove_missing_element_keeps_history_and_redo() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("History", 176, 166, ModTarget::Forge));
        sessions.record_history(Some(&project_id)).unwrap();
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1", 8, 18));
        sessions.mark_changed(Some(&project_id)).unwrap();
        sessions.undo(Some(&project_id)).unwrap();

        let removed =
            remove_element_from_session(&mut sessions, Some(&project_id), "missing").unwrap();

        let summary = sessions
            .list_sessions()
            .into_iter()
            .find(|summary| summary.id == project_id)
            .unwrap();
        assert!(!removed);
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn asset_remove_missing_asset_keeps_history_and_redo() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("History", 176, 166, ModTarget::Forge));
        sessions.record_history(Some(&project_id)).unwrap();
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .assets
            .push("textures/slot.png".to_string());
        sessions.mark_changed(Some(&project_id)).unwrap();
        sessions.undo(Some(&project_id)).unwrap();

        let removed =
            remove_asset_from_session(&mut sessions, Some(&project_id), "textures/missing.png")
                .unwrap();

        let summary = sessions
            .list_sessions()
            .into_iter()
            .find(|summary| summary.id == project_id)
            .unwrap();
        assert!(!removed);
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn asset_update_replaces_existing_asset_bytes_and_records_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Assets", 176, 166, ModTarget::Forge));
        let original = png_data_url([0, 0, 0, 255]);
        let updated = png_data_url([255, 0, 0, 255]);
        update_asset_in_session(
            &mut sessions,
            Some(&project_id),
            "textures/slot.png",
            &original,
        )
        .unwrap_err();
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .assets
            .push("textures/slot.png".to_string());

        let result = update_asset_in_session(
            &mut sessions,
            Some(&project_id),
            "textures/slot.png",
            &updated,
        )
        .unwrap();

        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert_eq!(result["name"], "textures/slot.png");
        assert_eq!(result["width"], 1);
        assert_eq!(result["height"], 1);
        assert_eq!(result["data_url"], updated);
        assert!(session
            .project
            .texture_data
            .contains_key("textures/slot.png"));
        assert!(session.project.is_dirty);
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert_eq!(summary.revision, 1);
        assert!(summary.can_undo);
    }

    #[test]
    fn asset_update_rejects_invalid_data_url_without_dirtying_project() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Assets", 176, 166, ModTarget::Forge));
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project.assets.push("textures/slot.png".to_string());
            project.is_dirty = false;
        }

        let result = update_asset_in_session(
            &mut sessions,
            Some(&project_id),
            "textures/slot.png",
            "data:text/plain;base64,abc",
        );

        let summary = session_summary(&sessions, &project_id).unwrap();
        let session = sessions.resolve(Some(&project_id)).unwrap();
        assert!(result.is_err());
        assert!(!session.project.is_dirty);
        assert_eq!(summary.revision, 0);
        assert!(!summary.can_undo);
    }

    #[test]
    fn asset_update_missing_asset_keeps_history_and_redo() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("History", 176, 166, ModTarget::Forge));
        sessions.record_history(Some(&project_id)).unwrap();
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .assets
            .push("textures/slot.png".to_string());
        sessions.mark_changed(Some(&project_id)).unwrap();
        sessions.undo(Some(&project_id)).unwrap();

        let result = update_asset_in_session(
            &mut sessions,
            Some(&project_id),
            "textures/missing.png",
            &png_data_url([0, 0, 0, 255]),
        );

        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(result.is_err());
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn asset_update_noop_preserves_redo() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("History", 176, 166, ModTarget::Forge));
        let data_url = png_data_url([0, 0, 0, 255]);
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project.assets.push("textures/slot.png".to_string());
        }
        update_asset_in_session(
            &mut sessions,
            Some(&project_id),
            "textures/slot.png",
            &data_url,
        )
        .unwrap();
        sessions.undo(Some(&project_id)).unwrap();
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project.assets.push("textures/slot.png".to_string());
            use base64::Engine;
            let payload = data_url.strip_prefix("data:image/png;base64,").unwrap();
            project.texture_data.insert(
                "textures/slot.png".to_string(),
                base64::engine::general_purpose::STANDARD
                    .decode(payload)
                    .unwrap(),
            );
        }

        update_asset_in_session(
            &mut sessions,
            Some(&project_id),
            "textures/slot.png",
            &data_url,
        )
        .unwrap();

        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn valid_element_move_records_history_and_clears_redo() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("History", 176, 166, ModTarget::Forge));
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1", 8, 18));

        let moved =
            move_element_in_session(&mut sessions, Some(&project_id), "slot_1", 10, 20).unwrap();

        let summary = sessions
            .list_sessions()
            .into_iter()
            .find(|summary| summary.id == project_id)
            .unwrap();
        assert_eq!((moved.x, moved.y), (10, 20));
        assert!(summary.can_undo);
        assert!(!summary.can_redo);
    }

    #[test]
    fn element_move_many_records_one_history_entry_for_multiple_moves() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("History", 176, 166, ModTarget::Forge));
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project.add_element(sample_element("slot_1", 8, 18));
            project.add_element(sample_element("slot_2", 26, 18));
            project.groups.push(Group {
                id: "group_player_inventory".to_string(),
                x: 8,
                y: 18,
                elements: vec!["slot_1".to_string(), "slot_2".to_string()],
            });
        }

        let moved = move_elements_in_session(
            &mut sessions,
            Some(&project_id),
            vec![
                ElementMove {
                    id: "slot_1".to_string(),
                    x: 10,
                    y: 20,
                },
                ElementMove {
                    id: "slot_2".to_string(),
                    x: 28,
                    y: 20,
                },
            ],
        )
        .unwrap();

        assert_eq!(
            moved
                .iter()
                .map(|el| (el.id.as_str(), el.x, el.y))
                .collect::<Vec<_>>(),
            vec![("slot_1", 10, 20), ("slot_2", 28, 20)]
        );
        assert_eq!(
            sessions.resolve(Some(&project_id)).unwrap().project.groups[0].x,
            10
        );
        assert_eq!(
            sessions.resolve(Some(&project_id)).unwrap().project.groups[0].y,
            20
        );
        sessions.undo(Some(&project_id)).unwrap();
        let project = &sessions.resolve(Some(&project_id)).unwrap().project;
        assert_eq!(
            project
                .elements
                .iter()
                .map(|el| (el.id.as_str(), el.x, el.y))
                .collect::<Vec<_>>(),
            vec![("slot_1", 8, 18), ("slot_2", 26, 18)]
        );
        assert_eq!((project.groups[0].x, project.groups[0].y), (8, 18));
        assert!(!session_summary(&sessions, &project_id).unwrap().can_undo);
    }

    #[test]
    fn element_update_changes_properties_once_and_preserves_redo_on_noop() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Update", 176, 166, ModTarget::Forge));
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1", 8, 18));

        let updated = update_element_in_session(
            &mut sessions,
            Some(&project_id),
            "slot_1",
            serde_json::json!({
                "x": 10,
                "visible": false,
                "uv": { "x": 1, "y": 2, "width": 16, "height": 16 }
            }),
        )
        .unwrap();

        assert_eq!(updated.x, 10);
        assert!(!updated.visible);
        assert_eq!(updated.uv.unwrap().width, 16);
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert_eq!(summary.revision, 1);
        assert!(summary.can_undo);

        sessions.undo(Some(&project_id)).unwrap();
        let unchanged = update_element_in_session(
            &mut sessions,
            Some(&project_id),
            "slot_1",
            serde_json::json!({ "x": 8 }),
        )
        .unwrap();

        assert_eq!(unchanged.x, 8);
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn element_resize_and_reorder_record_real_changes() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Resize", 176, 166, ModTarget::Forge));
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project.add_element(sample_element("slot_1", 8, 18));
            project.add_element(sample_element("slot_2", 20, 18));
        }

        let resized =
            resize_element_in_session(&mut sessions, Some(&project_id), "slot_1", 9, 19, 24, 24)
                .unwrap();
        assert_eq!((resized.x, resized.y, resized.size), (9, 19, Some(24)));

        let summary =
            reorder_element_in_session(&mut sessions, Some(&project_id), "slot_1", 1).unwrap();
        assert_eq!(summary.revision, 2);
        let elements = &sessions
            .resolve(Some(&project_id))
            .unwrap()
            .project
            .elements;
        assert_eq!(elements[1].id, "slot_1");
    }

    #[test]
    fn attached_region_create_update_remove_record_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Attached Regions", 176, 166, ModTarget::Forge));

        let created = create_attached_region_in_session(
            &mut sessions,
            Some(&project_id),
            crate::project::AttachedRegion {
                id: "returns_pocket".to_string(),
                anchor: crate::project::AttachedRegionAnchor::Right,
                x: 100,
                y: 18,
                width: 54,
                height: 72,
                state: crate::project::AttachedRegionState::Static,
                kind: Some("returns_pocket".to_string()),
                semantic_group: Some("food_returns".to_string()),
                visible: true,
            },
        )
        .unwrap();
        assert_eq!(created.id, "returns_pocket");

        let updated = update_attached_region_in_session(
            &mut sessions,
            Some(&project_id),
            "returns_pocket".to_string(),
            serde_json::json!({
                "x": 112,
                "state": "toggleable"
            }),
        )
        .unwrap();
        assert_eq!(updated.x, 112);
        assert_eq!(
            updated.state,
            crate::project::AttachedRegionState::Toggleable
        );

        let removed =
            remove_attached_region_in_session(&mut sessions, Some(&project_id), "returns_pocket")
                .unwrap();
        assert!(removed);
        assert!(sessions
            .resolve(Some(&project_id))
            .unwrap()
            .project
            .attached_regions
            .is_empty());
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 3);
    }

    #[test]
    fn attached_region_remove_command_returns_bool() {
        let _: for<'a> fn(State<'a, AppState>, Option<String>, String) -> Result<bool, String> =
            attached_region_remove;
    }

    #[test]
    fn attached_region_remove_clears_children_and_returns_true() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Attached Regions", 176, 166, ModTarget::Forge));
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project
                .attached_regions
                .push(sample_attached_region("returns_pocket", 100, 18));
            let mut attached = sample_element("returns_0", 108, 26);
            attached.attached_region = Some("returns_pocket".to_string());
            project.add_element(attached);
            project.add_element(sample_element("slot_1", 8, 18));
        }

        let removed =
            remove_attached_region_in_session(&mut sessions, Some(&project_id), "returns_pocket")
                .unwrap();

        let project = &sessions.resolve(Some(&project_id)).unwrap().project;
        assert!(removed);
        assert!(project.attached_regions.is_empty());
        assert_eq!(
            project.find_element("returns_0").unwrap().attached_region,
            None
        );
        assert_eq!(
            project.find_element("slot_1").unwrap().attached_region,
            None
        );
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 1);
    }

    #[test]
    fn attached_region_update_noop_preserves_redo() {
        let (mut sessions, project_id) = seed_attached_region_redo_session();

        let unchanged = update_attached_region_in_session(
            &mut sessions,
            Some(&project_id),
            "returns_pocket".to_string(),
            serde_json::json!({ "x": 100 }),
        )
        .unwrap();

        assert_eq!(unchanged.x, 100);
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn attached_region_move_noop_preserves_redo() {
        let (mut sessions, project_id) = seed_attached_region_redo_session();

        let unchanged = move_attached_region_with_elements_in_session(
            &mut sessions,
            Some(&project_id),
            "returns_pocket".to_string(),
            100,
            18,
        )
        .unwrap();

        assert_eq!((unchanged.x, unchanged.y), (100, 18));
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn attached_region_missing_remove_update_and_move_preserve_history_and_redo() {
        let (mut sessions, project_id) = seed_attached_region_redo_session();

        let removed =
            remove_attached_region_in_session(&mut sessions, Some(&project_id), "missing").unwrap();
        let update_result = update_attached_region_in_session(
            &mut sessions,
            Some(&project_id),
            "missing".to_string(),
            serde_json::json!({ "x": 101 }),
        );
        let move_result = move_attached_region_with_elements_in_session(
            &mut sessions,
            Some(&project_id),
            "missing".to_string(),
            101,
            19,
        );

        assert!(!removed);
        assert!(update_result.is_err());
        assert!(move_result.is_err());
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn attached_region_invalid_partial_update_preserves_project_history_and_dirty_state() {
        let invalid_changes = [
            serde_json::json!({ "id": "renamed" }),
            serde_json::json!({ "anchor": null }),
            serde_json::json!({ "state": "animated" }),
        ];

        for changes in invalid_changes {
            let mut sessions = ProjectSessionManager::default();
            let project_id = sessions.create_session(Project::new(
                "Attached Regions",
                176,
                166,
                ModTarget::Forge,
            ));
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .attached_regions
                .push(sample_attached_region("returns_pocket", 100, 18));
            sessions
                .resolve_mut(Some(&project_id))
                .unwrap()
                .project
                .is_dirty = false;
            let before = sessions.resolve(Some(&project_id)).unwrap().project.clone();

            let result = update_attached_region_in_session(
                &mut sessions,
                Some(&project_id),
                "returns_pocket".to_string(),
                changes,
            );

            let summary = session_summary(&sessions, &project_id).unwrap();
            let after = &sessions.resolve(Some(&project_id)).unwrap().project;
            assert!(result.is_err());
            assert_eq!(after, &before);
            assert!(!after.is_dirty);
            assert_eq!(summary.revision, 0);
            assert!(!summary.can_undo);
        }
    }

    #[test]
    fn attached_region_move_child_overflow_preserves_project_before_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Attached Regions", 176, 166, ModTarget::Forge));
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project
                .attached_regions
                .push(sample_attached_region("returns_pocket", 100, 18));
            let mut child = sample_element("returns_0", i32::MAX - 1, 26);
            child.attached_region = Some("returns_pocket".to_string());
            project.add_element(child);
        }
        let before = sessions.resolve(Some(&project_id)).unwrap().project.clone();

        let result = move_attached_region_with_elements_in_session(
            &mut sessions,
            Some(&project_id),
            "returns_pocket".to_string(),
            102,
            18,
        );

        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(result.is_err());
        assert_eq!(sessions.resolve(Some(&project_id)).unwrap().project, before);
        assert_eq!(summary.revision, 0);
        assert!(!summary.can_undo);
    }

    #[test]
    fn attached_region_move_with_elements_updates_absolute_child_coordinates() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Attached Regions", 176, 166, ModTarget::Forge));
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project
                .attached_regions
                .push(crate::project::AttachedRegion {
                    id: "returns_pocket".to_string(),
                    anchor: crate::project::AttachedRegionAnchor::Right,
                    x: 100,
                    y: 18,
                    width: 54,
                    height: 72,
                    state: crate::project::AttachedRegionState::Static,
                    kind: Some("returns_pocket".to_string()),
                    semantic_group: Some("food_returns".to_string()),
                    visible: true,
                });
            let mut slot = sample_element("returns_0", 108, 26);
            slot.attached_region = Some("returns_pocket".to_string());
            project.add_element(slot);
        }

        let moved = move_attached_region_with_elements_in_session(
            &mut sessions,
            Some(&project_id),
            "returns_pocket".to_string(),
            110,
            28,
        )
        .unwrap();

        assert_eq!((moved.x, moved.y), (110, 28));
        let child = sessions
            .resolve(Some(&project_id))
            .unwrap()
            .project
            .find_element("returns_0")
            .unwrap();
        assert_eq!((child.x, child.y), (118, 36));
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 1);
    }

    #[test]
    fn attached_region_move_refreshes_group_position_for_attached_children() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Attached Regions", 176, 166, ModTarget::Forge));
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project
                .attached_regions
                .push(sample_attached_region("returns_pocket", 100, 18));
            let mut attached = sample_element("returns_0", 108, 26);
            attached.attached_region = Some("returns_pocket".to_string());
            project.add_element(attached);
            project.add_element(sample_element("slot_1", 200, 200));
            project.groups.push(Group {
                id: "group_returns".to_string(),
                x: 108,
                y: 26,
                elements: vec!["slot_1".to_string(), "returns_0".to_string()],
            });
        }

        move_attached_region_with_elements_in_session(
            &mut sessions,
            Some(&project_id),
            "returns_pocket".to_string(),
            110,
            28,
        )
        .unwrap();

        let group = &sessions.resolve(Some(&project_id)).unwrap().project.groups[0];
        assert_eq!((group.x, group.y), (118, 36));
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 1);
    }

    #[test]
    fn group_create_and_ungroup_record_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Groups", 176, 166, ModTarget::Forge));
        {
            let project = &mut sessions.resolve_mut(Some(&project_id)).unwrap().project;
            project.add_element(sample_element("slot_1", 8, 18));
            project.add_element(sample_element("slot_2", 28, 18));
        }

        let group = create_group_in_session(
            &mut sessions,
            Some(&project_id),
            vec!["slot_1".to_string(), "slot_2".to_string()],
            Some("group_1".to_string()),
        )
        .unwrap();

        assert_eq!(group.id, "group_1");
        assert_eq!(
            group.elements,
            vec!["slot_1".to_string(), "slot_2".to_string()]
        );
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 1);
        assert!(session_summary(&sessions, &project_id).unwrap().can_undo);

        sessions.undo(Some(&project_id)).unwrap();
        assert!(sessions
            .resolve(Some(&project_id))
            .unwrap()
            .project
            .groups
            .is_empty());

        sessions.redo(Some(&project_id)).unwrap();
        assert_eq!(
            sessions
                .resolve(Some(&project_id))
                .unwrap()
                .project
                .groups
                .len(),
            1
        );

        assert!(ungroup_in_session(&mut sessions, Some(&project_id), "group_1").unwrap());
        assert!(sessions
            .resolve(Some(&project_id))
            .unwrap()
            .project
            .groups
            .is_empty());
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 4);
    }

    #[test]
    fn group_create_missing_or_too_small_preserves_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Groups", 176, 166, ModTarget::Forge));
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1", 8, 18));

        assert!(create_group_in_session(
            &mut sessions,
            Some(&project_id),
            vec!["slot_1".to_string()],
            Some("group_1".to_string()),
        )
        .is_err());
        assert!(create_group_in_session(
            &mut sessions,
            Some(&project_id),
            vec!["slot_1".to_string(), "missing".to_string()],
            Some("group_1".to_string()),
        )
        .is_err());

        let summary = session_summary(&sessions, &project_id).unwrap();
        assert_eq!(summary.revision, 0);
        assert!(!summary.can_undo);
    }

    #[test]
    fn element_reorder_missing_or_same_position_does_not_corrupt_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Reorder", 176, 166, ModTarget::Forge));
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1", 8, 18));
        sessions.record_history(Some(&project_id)).unwrap();
        sessions.mark_changed(Some(&project_id)).unwrap();
        sessions.undo(Some(&project_id)).unwrap();

        assert!(
            reorder_element_in_session(&mut sessions, Some(&project_id), "missing", 0).is_err()
        );
        let summary =
            reorder_element_in_session(&mut sessions, Some(&project_id), "slot_1", 0).unwrap();

        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    fn sample_animation(id: &str) -> Animation {
        Animation {
            id: id.to_string(),
            animation_type: AnimationType::Fill,
            data_key: "progress".to_string(),
            texture: None,
            direction: None,
            frame_count: None,
            fps: None,
            min_value: None,
            max_value: None,
            triggers_on: None,
        }
    }

    #[test]
    fn animation_create_update_remove_and_bind_record_history() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Animations", 176, 166, ModTarget::Forge));
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1", 8, 18));

        let created = create_animation_in_session(
            &mut sessions,
            Some(&project_id),
            sample_animation("fill_1"),
        )
        .unwrap();
        assert_eq!(created.id, "fill_1");

        let updated = update_animation_in_session(
            &mut sessions,
            Some(&project_id),
            "fill_1",
            serde_json::json!({ "min_value": 0.25, "max_value": 0.75 }),
        )
        .unwrap();
        assert_eq!(updated.min_value, Some(0.25));

        let bound = bind_animation_in_session(&mut sessions, Some(&project_id), "slot_1", "fill_1")
            .unwrap();
        assert_eq!(bound.animation.as_deref(), Some("fill_1"));

        let unbound =
            unbind_animation_in_session(&mut sessions, Some(&project_id), "slot_1").unwrap();
        assert_eq!(unbound.animation, None);

        let removed =
            remove_animation_from_session(&mut sessions, Some(&project_id), "fill_1").unwrap();
        assert!(removed);
        assert!(sessions
            .resolve(Some(&project_id))
            .unwrap()
            .project
            .animations
            .is_empty());
        assert_eq!(session_summary(&sessions, &project_id).unwrap().revision, 5);
    }

    #[test]
    fn animation_missing_targets_do_not_corrupt_redo() {
        let mut sessions = ProjectSessionManager::default();
        let project_id =
            sessions.create_session(Project::new("Animations", 176, 166, ModTarget::Forge));
        sessions
            .resolve_mut(Some(&project_id))
            .unwrap()
            .project
            .add_element(sample_element("slot_1", 8, 18));
        create_animation_in_session(&mut sessions, Some(&project_id), sample_animation("fill_1"))
            .unwrap();
        sessions.undo(Some(&project_id)).unwrap();

        assert!(
            bind_animation_in_session(&mut sessions, Some(&project_id), "slot_1", "missing")
                .is_err()
        );
        let removed =
            remove_animation_from_session(&mut sessions, Some(&project_id), "missing").unwrap();
        assert!(!removed);
        let summary = session_summary(&sessions, &project_id).unwrap();
        assert!(!summary.can_undo);
        assert!(summary.can_redo);
    }

    #[test]
    fn font_list_always_includes_minecraft_default() {
        let project = Project::new("Fonts", 176, 166, ModTarget::Forge);

        let fonts = font_list_json(&project);

        assert!(fonts.iter().any(|font| font["id"] == "minecraft:default"));
    }

    #[test]
    fn bundled_default_font_render_data_contains_glyphs_and_provider_images() {
        let font = crate::font::load_bundled_font();

        let render_data = font_render_data_json(&font);

        assert_eq!(render_data["id"], "minecraft:default");
        assert!(render_data["glyph_map"].get("A").is_some());
        assert!(render_data["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["image_data_url"]
                .as_str()
                .is_some_and(|data_url| data_url.starts_with("data:image/png;base64,"))));
    }

    #[test]
    fn validate_font_file_size_rejects_files_over_16_mib() {
        assert!(validate_font_file_size(MAX_FONT_FILE_SIZE).is_ok());
        assert_eq!(
            validate_font_file_size(MAX_FONT_FILE_SIZE + 1).unwrap_err(),
            "Font file is too large; maximum supported size is 16 MiB"
        );
    }
}
