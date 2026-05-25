use crate::animation::Animation;
use crate::project::{
    CodegenMode, ElementType, Layer, Project, ProjectExportSettings, SemanticGroup,
    SemanticGroupKind, SlotRole,
};
use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ExportConfig {
    pub mod_id: String,
    pub package: String,
    pub class_name: String,
    pub output_dir: String,
    pub settings_override: Option<ProjectExportSettings>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportPreview {
    pub target: String,
    pub mod_id: String,
    pub package: String,
    pub class_name: String,
    pub output_dir: String,
    pub files: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportTarget {
    Forge,
    Fabric,
    NeoForge,
}

#[derive(Debug, Clone)]
struct SanitizedExport {
    mod_id: String,
    package: String,
    package_path: String,
    class_name: String,
    screen_class_name: String,
    resource_name: String,
    output_dir: PathBuf,
}

#[derive(Debug)]
struct PlannedFile {
    path: PathBuf,
    data: Vec<u8>,
}

#[derive(Debug)]
struct ExportPlan {
    target: ExportTarget,
    export: SanitizedExport,
    files: Vec<PlannedFile>,
    errors: Vec<String>,
}

impl ExportTarget {
    fn parse(target: &str) -> Result<Self, String> {
        match target.trim().to_ascii_lowercase().as_str() {
            "forge" => Ok(Self::Forge),
            "fabric" => Ok(Self::Fabric),
            "neoforge" | "neo_forge" => Ok(Self::NeoForge),
            other => Err(format!("Unsupported export target: {other}")),
        }
    }

    fn loader_name(self) -> &'static str {
        match self {
            Self::Forge => "Forge",
            Self::Fabric => "Fabric",
            Self::NeoForge => "NeoForge",
        }
    }

    fn loader_id(self) -> &'static str {
        match self {
            Self::Forge => "forge",
            Self::Fabric => "fabric",
            Self::NeoForge => "neoforge",
        }
    }
}

impl SanitizedExport {
    fn new(config: &ExportConfig) -> Result<Self, String> {
        let mod_id = sanitize_mod_id(&config.mod_id);
        let class_name = sanitize_class_name(&config.class_name);
        let screen_class_name = screen_class_name(&class_name);
        let package = sanitize_package(&config.package, &mod_id);
        let package_path = package.replace('.', "/");
        let resource_name = sanitize_resource_name(&config.class_name);
        let output_dir = PathBuf::from(config.output_dir.trim());

        if output_dir.as_os_str().is_empty() {
            return Err("Export output directory cannot be empty".to_string());
        }

        Ok(Self {
            mod_id,
            package,
            package_path,
            class_name,
            screen_class_name,
            resource_name,
            output_dir,
        })
    }

    fn java_dir(&self) -> PathBuf {
        self.output_dir
            .join("src/main/java")
            .join(&self.package_path)
    }

    fn asset_dir(&self) -> PathBuf {
        self.output_dir
            .join("src/main/resources/assets")
            .join(&self.mod_id)
    }
}

pub fn export_project(
    project: &Project,
    config: &ExportConfig,
    target: &str,
) -> Result<Vec<String>, String> {
    let plan = plan_export(project, config, target)?;
    if !plan.errors.is_empty() {
        return Err(plan.errors.join("\n"));
    }

    let mut files = Vec::new();
    for file in plan.files {
        write_file(&file.path, &file.data)?;
        files.push(file.path.to_string_lossy().to_string());
    }

    Ok(files)
}

pub fn preview_export(
    project: &Project,
    config: &ExportConfig,
    target: &str,
) -> Result<ExportPreview, String> {
    let settings = effective_export_settings(project, config);
    let plan = plan_export(project, config, target)?;
    let mut warnings = if config.overwrite {
        Vec::new()
    } else {
        existing_file_warnings(&plan.files)
    };
    warnings.extend(semantic_warnings(project, &settings));
    warnings.extend(progress_texture_warnings(project));
    Ok(ExportPreview {
        target: plan.target.loader_id().to_string(),
        mod_id: plan.export.mod_id,
        package: plan.export.package,
        class_name: plan.export.class_name,
        output_dir: plan.export.output_dir.to_string_lossy().to_string(),
        files: plan
            .files
            .iter()
            .map(|file| file.path.to_string_lossy().to_string())
            .collect(),
        warnings,
        errors: plan.errors,
    })
}

fn plan_export(
    project: &Project,
    config: &ExportConfig,
    target: &str,
) -> Result<ExportPlan, String> {
    let target = ExportTarget::parse(target)?;
    let export = SanitizedExport::new(config)?;
    let settings = effective_export_settings(project, config);
    validate_project_dimensions(project)?;

    let mut files = Vec::new();
    let errors = missing_texture_errors(project);

    let settings_path = export.output_dir.join("settings.gradle");
    plan_file(
        &mut files,
        settings_path,
        generated_text(generate_settings_gradle(&export, target)),
    )?;

    let gradle_path = export.output_dir.join("build.gradle");
    plan_file(
        &mut files,
        gradle_path,
        generated_text(generate_build_gradle(&export, target)),
    )?;

    let properties_path = export.output_dir.join("gradle.properties");
    plan_file(
        &mut files,
        properties_path,
        generated_text(generate_gradle_properties(&export, target)),
    )?;

    // Background atlas
    let bg_atlas =
        crate::texture::composite_atlas_for_layer_with_visual_empty(project, Layer::Background)?;
    let bg_texture_path = export
        .asset_dir()
        .join(format!("textures/gui/{}_gui.png", export.resource_name));
    plan_file(&mut files, bg_texture_path, bg_atlas)?;

    // Overlay atlas (only if overlay elements exist)
    let has_overlay = project
        .elements
        .iter()
        .any(|e| e.visible && e.layer == Layer::Overlay);
    if has_overlay {
        let overlay_atlas =
            crate::texture::composite_atlas_for_layer_with_visual_empty(project, Layer::Overlay)?;
        let overlay_texture_path = export
            .asset_dir()
            .join(format!("textures/gui/{}_overlay.png", export.resource_name));
        plan_file(&mut files, overlay_texture_path, overlay_atlas)?;
    }

    // Animatable sprites
    for element in &project.elements {
        if element.visible && element.layer == Layer::Animatable {
            let sprite = crate::texture::composite_single_element(element, project)?;
            let sprite_path = export
                .asset_dir()
                .join(format!("textures/gui/{}.png", element.id));
            plan_file(&mut files, sprite_path, sprite)?;
        }
    }

    for asset in referenced_texture_assets(project) {
        if let Some(data) = project.texture_data.get(asset.as_ref()) {
            let asset_path = export.asset_dir().join(asset.as_ref());
            plan_file(&mut files, asset_path, data.clone())?;
        }
    }

    let mut textures_json = serde_json::json!({
        "background": format!("textures/gui/{}_gui.png", export.resource_name),
    });
    if has_overlay {
        textures_json["overlay"] =
            serde_json::json!(format!("textures/gui/{}_overlay.png", export.resource_name));
    }

    let layout_project = project_with_effective_settings(project, &settings);
    let layout = layout_json_value(&layout_project, textures_json);
    let layout_path = export
        .asset_dir()
        .join(format!("gui/{}_layout.json", export.resource_name));
    let layout_json = serde_json::to_vec_pretty(&layout)
        .map_err(|e| format!("Failed to serialize layout JSON: {e}"))?;
    plan_file(&mut files, layout_path, layout_json)?;

    let layout_java_path = export.java_dir().join("GuiLayout.java");
    plan_file(
        &mut files,
        layout_java_path,
        generated_text(generate_gui_layout_java(&export, target, project)),
    )?;

    if settings.codegen_mode == CodegenMode::Modular && settings.generate_semantic_registry {
        let registry_path = export.java_dir().join("GuiSemanticRegistry.java");
        plan_file(
            &mut files,
            registry_path,
            generated_text(generate_semantic_registry_java(&export, project)),
        )?;
    }

    let screen_path = export
        .java_dir()
        .join(format!("{}.java", export.screen_class_name));
    let screen_code = match target {
        ExportTarget::Forge => generate_forge_screen(&export, project),
        ExportTarget::Fabric => generate_fabric_screen(&export, project),
        ExportTarget::NeoForge => generate_neoforge_screen(&export, project),
    };
    plan_file(&mut files, screen_path, generated_text(screen_code))?;

    let mod_entry_path = export
        .java_dir()
        .join(format!("{}Client.java", export.class_name));
    plan_file(
        &mut files,
        mod_entry_path,
        generated_text(generate_client_entrypoint(&export, target)),
    )?;

    let metadata_path = loader_metadata_path(&export, target);
    plan_file(
        &mut files,
        metadata_path,
        loader_metadata_data(generate_loader_metadata(&export, target), target),
    )?;

    let readme_path = export.output_dir.join("README.txt");
    plan_file(
        &mut files,
        readme_path,
        generated_text(generate_readme(&export, target, project)),
    )?;

    Ok(ExportPlan {
        target,
        export,
        files,
        errors,
    })
}

fn plan_file(files: &mut Vec<PlannedFile>, path: PathBuf, data: Vec<u8>) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Export file path cannot be empty".to_string());
    }
    files.push(PlannedFile { path, data });
    Ok(())
}

fn generated_text(text: String) -> Vec<u8> {
    let mut output = String::new();
    for line in text.lines() {
        output.push_str(line.trim_end());
        output.push('\n');
    }
    output.into_bytes()
}

fn loader_metadata_data(metadata: String, target: ExportTarget) -> Vec<u8> {
    match target {
        ExportTarget::Fabric => metadata.into_bytes(),
        ExportTarget::Forge | ExportTarget::NeoForge => generated_text(metadata),
    }
}

fn effective_export_settings(project: &Project, config: &ExportConfig) -> ProjectExportSettings {
    config
        .settings_override
        .clone()
        .unwrap_or_else(|| project.export_settings.clone())
        .normalized()
}

fn project_with_effective_settings<'a>(
    project: &'a Project,
    settings: &ProjectExportSettings,
) -> Cow<'a, Project> {
    if &project.export_settings == settings {
        Cow::Borrowed(project)
    } else {
        let mut project = project.clone();
        project.export_settings = settings.clone();
        Cow::Owned(project)
    }
}

fn layout_json_value(project: &Project, mut textures_json: serde_json::Value) -> serde_json::Value {
    let visual_bounds = project.visual_bounds();
    textures_json["visual_offset_x"] = serde_json::json!(visual_bounds.x);
    textures_json["visual_offset_y"] = serde_json::json!(visual_bounds.y);

    let elements_json: Vec<serde_json::Value> = project
        .elements
        .iter()
        .map(|e| {
            let mut val = serde_json::to_value(e).unwrap();
            if e.visible && e.layer == Layer::Animatable {
                val["texture"] = serde_json::json!(format!("textures/gui/{}.png", e.id));
            }
            val
        })
        .collect();

    serde_json::json!({
        "gui_size": project.gui_size,
        "visual_bounds": visual_bounds,
        "textures": textures_json,
        "elements": elements_json,
        "groups": project.groups,
        "semantic_groups": project.semantic_groups,
        "attached_regions": project.attached_regions,
        "animations": project.animations,
        "export_settings": project.export_settings,
    })
}

fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_project_dimensions(project: &Project) -> Result<(), String> {
    if project.gui_size.width == 0 || project.gui_size.height == 0 {
        return Err("Project GUI dimensions must be greater than zero".to_string());
    }

    Ok(())
}

fn missing_texture_errors(project: &Project) -> Vec<String> {
    referenced_texture_assets(project)
        .into_iter()
        .filter(|asset| !project.texture_data.contains_key(asset.as_ref()))
        .map(|asset| format!("Texture asset referenced by project is missing: {asset}"))
        .collect()
}

fn existing_file_warnings(files: &[PlannedFile]) -> Vec<String> {
    files
        .iter()
        .filter(|file| file.path.exists())
        .map(|file| {
            format!(
                "Target file already exists and will be overwritten: {}",
                file.path.to_string_lossy()
            )
        })
        .collect()
}

fn progress_texture_warnings(project: &Project) -> Vec<String> {
    project
        .elements
        .iter()
        .filter(|element| element.element_type == ElementType::Progress)
        .filter_map(|element| {
            let asset = element.asset.as_deref()?;
            let data = project.texture_data.get(asset)?;
            let image = image::load_from_memory(data).ok()?;
            let (source_width, source_height) =
                progress_source_dimensions(image.width(), image.height(), element.uv.as_ref())?;
            let width = element.width.unwrap_or(source_width);
            let height = element.height.unwrap_or(source_height);
            ((width, height) != (source_width, source_height)).then(|| {
                format!(
                    "Progress element '{}' is stretched from texture '{}' ({}x{}) to {}x{}; this is allowed but may be accidental for pixel-art GUI work.",
                    element.id,
                    asset,
                    source_width,
                    source_height,
                    width,
                    height
                )
            })
        })
        .collect()
}

fn progress_source_dimensions(
    texture_width: u32,
    texture_height: u32,
    uv: Option<&crate::project::UvRect>,
) -> Option<(u32, u32)> {
    if let Some(uv) = uv {
        let x = uv.x.min(texture_width);
        let y = uv.y.min(texture_height);
        let width = uv.width.min(texture_width.saturating_sub(x));
        let height = uv.height.min(texture_height.saturating_sub(y));
        if width == 0 || height == 0 {
            return None;
        }
        Some((width, height))
    } else {
        Some((texture_width, texture_height))
    }
}

fn semantic_warnings(project: &Project, settings: &ProjectExportSettings) -> Vec<String> {
    if settings.codegen_mode != CodegenMode::Modular {
        return Vec::new();
    }

    let mut warnings = Vec::new();
    if project.semantic_groups.is_empty() {
        return vec![
            "Modular code generation is enabled, but the project has no semantic groups."
                .to_string(),
        ];
    }

    for element in &project.elements {
        if matches!(
            element.element_type,
            ElementType::Panel | ElementType::Tab | ElementType::VirtualSlotCell
        ) && element.inventory_group.is_none()
            && element.target_group.is_none()
        {
            warnings.push(format!(
                "Element '{}' is modular but has no semantic group binding.",
                element.id
            ));
        }
    }

    warnings.extend(semantic_integrity_warnings(project));
    warnings
}

fn semantic_integrity_warnings(project: &Project) -> Vec<String> {
    let mut warnings = Vec::new();

    for group in &project.semantic_groups {
        let explicit_members = explicit_member_elements(project, group);
        warnings.extend(duplicate_member_warnings(group));
        warnings.extend(
            explicit_members
                .iter()
                .filter_map(|member| member.as_ref().err().map(std::string::ToString::to_string)),
        );

        if let Some(slot_count) = slot_count_requirement(group) {
            let matching = count_matching_group_elements(project, group, &explicit_members);
            if matching != slot_count {
                let qualifier = if matching < slot_count { "only " } else { "" };
                warnings.push(format!(
                    "Semantic group '{}' declares {} slots but {}{} matching elements were found.",
                    group.id, slot_count, qualifier, matching
                ));
            }
        }

        if scroll_binding_semantics_apply(group) {
            if scroll_binding_expected(group) && group.scroll_binding.is_none() {
                warnings.push(format!(
                    "Semantic group '{}' is scrollable or dynamic but has no scroll binding.",
                    group.id
                ));
            }

            if let Some(binding) = group.scroll_binding.as_deref() {
                if !has_matching_scrollbar(project, group, binding) {
                    warnings.push(format!(
                        "Semantic group '{}' declares scroll binding '{}' but no matching scrollbar element was found.",
                        group.id, binding
                    ));
                }
            }
        }

        warnings.extend(control_button_warnings(project, group, &explicit_members));
    }

    warnings
}

fn explicit_member_elements<'a>(
    project: &'a Project,
    group: &SemanticGroup,
) -> Vec<Result<&'a crate::project::Element, String>> {
    group
        .member_ids
        .iter()
        .map(|id| {
            project
                .elements
                .iter()
                .find(|element| element.id == *id)
                .ok_or_else(|| {
                    format!(
                        "Semantic group '{}' references missing element '{}'.",
                        group.id, id
                    )
                })
        })
        .collect()
}

fn duplicate_member_warnings(group: &SemanticGroup) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut warned = HashSet::new();
    group
        .member_ids
        .iter()
        .filter(|id| !seen.insert(id.as_str()) && warned.insert(id.as_str()))
        .map(|id| {
            format!(
                "Semantic group '{}' references duplicate member id '{}'.",
                group.id, id
            )
        })
        .collect()
}

fn control_button_warnings(
    project: &Project,
    group: &SemanticGroup,
    explicit_members: &[Result<&crate::project::Element, String>],
) -> Vec<String> {
    if group.kind != SemanticGroupKind::ControlButtons {
        return Vec::new();
    }
    if !group.member_ids.is_empty() {
        let explicit_buttons = distinct_resolved_explicit_members(explicit_members)
            .filter(|element| {
                matches!(
                    element.element_type,
                    ElementType::Button | ElementType::ToggleButton
                )
            })
            .count();
        let mut warnings = distinct_resolved_explicit_members(explicit_members)
            .filter(|element| {
                !matches!(
                    element.element_type,
                    ElementType::Button | ElementType::ToggleButton
                )
            })
            .map(|element| {
                format!(
                    "Semantic group '{}' references non-button element '{}'.",
                    group.id, element.id
                )
            })
            .collect::<Vec<_>>();

        if let Some(expected) = group.slot_count.map(|slot_count| slot_count as usize) {
            if explicit_buttons != expected {
                let qualifier = if explicit_buttons < expected {
                    "only "
                } else {
                    ""
                };
                warnings.push(format!(
                    "Semantic group '{}' declares {} control buttons but {}{} matching button elements were found.",
                    group.id, expected, qualifier, explicit_buttons
                ));
            }
        }

        return warnings;
    }

    let Some(expected) = group.slot_count.map(|slot_count| slot_count as usize) else {
        return Vec::new();
    };
    let matching = project
        .elements
        .iter()
        .filter(|element| {
            matches!(
                element.element_type,
                ElementType::Button | ElementType::ToggleButton
            )
        })
        .filter(|element| {
            element.inventory_group.as_deref() == Some(group.id.as_str())
                || group
                    .data_source
                    .as_deref()
                    .is_some_and(|data_source| element.binding.as_deref() == Some(data_source))
                || element.target_group.as_deref() == Some(group.id.as_str())
        })
        .count();
    if matching < expected {
        vec![format!(
            "Semantic group '{}' declares control buttons but only {} matching button elements were found.",
            group.id, matching
        )]
    } else {
        Vec::new()
    }
}

fn slot_count_requirement(group: &SemanticGroup) -> Option<u32> {
    if matches!(
        group.kind,
        SemanticGroupKind::FixedSlots
            | SemanticGroupKind::PlayerInventory
            | SemanticGroupKind::Hotbar
            | SemanticGroupKind::UpgradeSlots
    ) {
        group.slot_count
    } else {
        None
    }
}

fn count_matching_group_elements(
    project: &Project,
    group: &SemanticGroup,
    explicit_members: &[Result<&crate::project::Element, String>],
) -> u32 {
    if !group.member_ids.is_empty() {
        return distinct_resolved_explicit_members(explicit_members)
            .filter(|element| element.element_type == ElementType::Slot)
            .filter(|element| slot_role_matches_group(element.slot_role.as_ref(), &group.kind))
            .count() as u32;
    }

    project
        .elements
        .iter()
        .filter(|element| element.element_type == ElementType::Slot)
        .filter(|element| element.inventory_group.as_deref() == Some(group.id.as_str()))
        .filter(|element| slot_role_matches_group(element.slot_role.as_ref(), &group.kind))
        .count() as u32
}

fn distinct_resolved_explicit_members<'a>(
    explicit_members: &'a [Result<&'a crate::project::Element, String>],
) -> impl Iterator<Item = &'a crate::project::Element> + 'a {
    let mut seen = HashSet::new();
    explicit_members.iter().filter_map(move |member| {
        let element = member.as_ref().ok().copied()?;
        if seen.insert(element.id.as_str()) {
            Some(element)
        } else {
            None
        }
    })
}

fn slot_role_matches_group(slot_role: Option<&SlotRole>, kind: &SemanticGroupKind) -> bool {
    match kind {
        SemanticGroupKind::FixedSlots => true,
        SemanticGroupKind::PlayerInventory => matches!(slot_role, Some(SlotRole::PlayerInventory)),
        SemanticGroupKind::Hotbar => matches!(slot_role, Some(SlotRole::Hotbar)),
        SemanticGroupKind::UpgradeSlots => matches!(
            slot_role,
            Some(SlotRole::Upgrade | SlotRole::UpgradeSettings)
        ),
        _ => false,
    }
}

fn scroll_binding_expected(group: &SemanticGroup) -> bool {
    group.kind == SemanticGroupKind::VirtualSlotGrid
        && (group.dynamic_height
            || group
                .total_rows
                .zip(group.visible_rows)
                .is_some_and(|(total, visible)| total > visible))
}

fn scroll_binding_semantics_apply(group: &SemanticGroup) -> bool {
    matches!(group.kind, SemanticGroupKind::VirtualSlotGrid)
}

fn has_matching_scrollbar(project: &Project, group: &SemanticGroup, binding: &str) -> bool {
    project.elements.iter().any(|element| {
        element.element_type == ElementType::Scrollbar
            && element_binds_scrollbar(element, group, binding)
    })
}

fn element_binds_scrollbar(
    element: &crate::project::Element,
    group: &SemanticGroup,
    binding: &str,
) -> bool {
    let binding_matches = element.id == binding
        || element.scroll_binding.as_deref() == Some(binding)
        || element.binding.as_deref() == Some(binding);
    let group_matches = element
        .target_group
        .as_deref()
        .is_none_or(|target_group| target_group == group.id);

    binding_matches && group_matches
}

fn referenced_texture_assets(project: &Project) -> Vec<Cow<'_, str>> {
    let mut assets = Vec::new();
    for element in &project.elements {
        if !element.visible {
            continue;
        }
        if let Some(asset) = element.asset.as_deref() {
            if !assets.iter().any(|known: &Cow<'_, str>| known == asset) {
                assets.push(Cow::Borrowed(asset));
            }
        }
        if let Some(icon) = element.icon.as_deref() {
            if !assets.iter().any(|known: &Cow<'_, str>| known == icon) {
                assets.push(Cow::Borrowed(icon));
            }
        }
    }
    for animation in &project.animations {
        if let Some(asset) = animation.texture.as_deref() {
            if !assets.iter().any(|known: &Cow<'_, str>| known == asset) {
                assets.push(Cow::Borrowed(asset));
            }
        }
    }
    assets
}

fn sanitize_mod_id(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_lowercase() || lower.is_ascii_digit() || lower == '_' || lower == '-' {
            out.push(lower);
        } else if lower.is_ascii_whitespace() || lower == '.' {
            out.push('_');
        }
    }
    trim_invalid_resource_edges(&out).unwrap_or_else(|| "mcgui_export".to_string())
}

fn sanitize_resource_name(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_lowercase() || lower.is_ascii_digit() || lower == '_' || lower == '-' {
            out.push(lower);
        } else if lower.is_ascii_whitespace() || lower == '.' {
            out.push('_');
        }
    }
    trim_invalid_resource_edges(&out).unwrap_or_else(|| "gui".to_string())
}

fn trim_invalid_resource_edges(value: &str) -> Option<String> {
    let trimmed = value.trim_matches(|ch| ch == '_' || ch == '-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn sanitize_class_name(value: &str) -> String {
    let mut out = String::new();
    let mut capitalize_next = true;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            let ch = if capitalize_next {
                ch.to_ascii_uppercase()
            } else {
                ch
            };
            out.push(ch);
            capitalize_next = false;
        } else {
            capitalize_next = true;
        }
    }

    if out.is_empty() {
        out.push_str("GeneratedGui");
    }
    if out
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        out.insert(0, 'G');
    }
    if java_keywords().contains(&out.as_str()) {
        out.push_str("Gui");
    }
    out
}

fn screen_class_name(class_name: &str) -> String {
    if class_name.ends_with("Screen") {
        class_name.to_string()
    } else {
        format!("{class_name}Screen")
    }
}

fn sanitize_package(value: &str, mod_id: &str) -> String {
    let segments: Vec<String> = value
        .split('.')
        .filter_map(sanitize_package_segment)
        .collect();

    if segments.is_empty() {
        format!("com.example.{}", mod_id.replace('-', "_"))
    } else {
        segments.join(".")
    }
}

fn sanitize_package_segment(segment: &str) -> Option<String> {
    let mut out = String::new();
    for ch in segment.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() || lower == '_' {
            out.push(lower);
        }
    }
    if out.is_empty() {
        return None;
    }
    if out
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        out.insert(0, '_');
    }
    if java_keywords().contains(&out.as_str()) {
        out.push('_');
    }
    Some(out)
}

fn java_keywords() -> &'static [&'static str] {
    &[
        "abstract",
        "assert",
        "boolean",
        "break",
        "byte",
        "case",
        "catch",
        "char",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "double",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "float",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "int",
        "interface",
        "long",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "short",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "void",
        "volatile",
        "while",
    ]
}

fn generate_settings_gradle(export: &SanitizedExport, target: ExportTarget) -> String {
    format!(
        r#"pluginManagement {{
    repositories {{
        gradlePluginPortal()
        maven {{ url = "https://maven.fabricmc.net/" }}
        maven {{ url = "https://maven.minecraftforge.net/" }}
        maven {{ url = "https://maven.neoforged.net/releases" }}
    }}
}}

dependencyResolutionManagement {{
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {{
        mavenCentral()
        maven {{ url = "https://maven.fabricmc.net/" }}
        maven {{ url = "https://maven.minecraftforge.net/" }}
        maven {{ url = "https://maven.neoforged.net/releases" }}
    }}
}}

rootProject.name = "{}-{}-gui"
"#,
        export.mod_id,
        target.loader_id()
    )
}

fn generate_build_gradle(export: &SanitizedExport, target: ExportTarget) -> String {
    let plugins = match target {
        ExportTarget::Fabric => {
            r#"plugins {
    id "fabric-loom" version "1.7-SNAPSHOT"
}
"#
        }
        ExportTarget::Forge => {
            r#"plugins {
    id "net.minecraftforge.gradle" version "[6.0,6.2)"
}
"#
        }
        ExportTarget::NeoForge => {
            r#"plugins {
    id "net.neoforged.gradle.userdev" version "7.0.+"
}
"#
        }
    };
    let dependencies = match target {
        ExportTarget::Fabric => {
            r#"dependencies {
    minecraft "com.mojang:minecraft:${minecraft_version}"
    mappings "net.fabricmc:yarn:${yarn_mappings}:v2"
    modImplementation "net.fabricmc:fabric-loader:${loader_version}"
    modImplementation "net.fabricmc.fabric-api:fabric-api:${fabric_api_version}"
    implementation "com.google.code.gson:gson:2.11.0"
}
"#
        }
        ExportTarget::Forge => {
            r#"minecraft {
    mappings channel: "official", version: minecraft_version
}

dependencies {
    minecraft "net.minecraftforge:forge:${minecraft_version}-${forge_version}"
    implementation "com.google.code.gson:gson:2.11.0"
}
"#
        }
        ExportTarget::NeoForge => {
            r#"runs {
    configureEach {
        modSource project.sourceSets.main
    }
}

dependencies {
    implementation "net.neoforged:neoforge:${neoforge_version}"
    implementation "com.google.code.gson:gson:2.11.0"
}
"#
        }
    };

    format!(
        r#"{plugins}

group = "{package}"
version = "1.0.0"

base {{
    archivesName = "{mod_id}-gui"
}}

java {{
    toolchain.languageVersion = JavaLanguageVersion.of(21)
}}

{dependencies}
"#,
        plugins = plugins,
        package = export.package,
        mod_id = export.mod_id,
        dependencies = dependencies
    )
}

fn generate_gradle_properties(_export: &SanitizedExport, target: ExportTarget) -> String {
    match target {
        ExportTarget::Fabric => r#"org.gradle.jvmargs=-Xmx2G
minecraft_version=1.21.1
yarn_mappings=1.21.1+build.3
loader_version=0.16.9
fabric_api_version=0.107.0+1.21.1
"#
        .to_string(),
        ExportTarget::Forge => r#"org.gradle.jvmargs=-Xmx2G
minecraft_version=1.21.1
forge_version=52.0.28
"#
        .to_string(),
        ExportTarget::NeoForge => r#"org.gradle.jvmargs=-Xmx2G
minecraft_version=1.21.1
neoforge_version=21.1.90
"#
        .to_string(),
    }
}

fn generate_gui_layout_java(
    export: &SanitizedExport,
    target: ExportTarget,
    project: &Project,
) -> String {
    match target {
        ExportTarget::Fabric => generate_fabric_layout_java(export, project),
        ExportTarget::Forge | ExportTarget::NeoForge => {
            generate_forge_like_layout_java(export, target, project)
        }
    }
}

fn generate_semantic_registry_java(export: &SanitizedExport, project: &Project) -> String {
    let groups =
        serde_json::to_string_pretty(&project.semantic_groups).unwrap_or_else(|_| "[]".into());
    format!(
        r#"package {};

public final class GuiSemanticRegistry {{
    public static final String CODEGEN_MODE = "modular";
    public static final String GROUPS_JSON = "{}";

    private GuiSemanticRegistry() {{}}
}}
"#,
        export.package,
        groups
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

fn generate_forge_like_layout_java(
    export: &SanitizedExport,
    target: ExportTarget,
    project: &Project,
) -> String {
    let resource_location_ctor = match target {
        ExportTarget::NeoForge => "ResourceLocation.fromNamespaceAndPath(namespace, path)",
        _ => "new ResourceLocation(namespace, path)",
    };

    let has_overlay = project
        .elements
        .iter()
        .any(|e| e.visible && e.layer == Layer::Overlay);
    let overlay_field = if has_overlay {
        "private final ResourceLocation overlay;\n    "
    } else {
        ""
    };
    let overlay_ctor = if has_overlay {
        ", ResourceLocation overlay"
    } else {
        ""
    };
    let overlay_assign = if has_overlay {
        "this.overlay = overlay;\n        "
    } else {
        ""
    };
    let load_layout_body = if has_overlay {
        r#"            int visualOffsetX = data.textures.visualOffsetXOrDefault();
            int visualOffsetY = data.textures.visualOffsetYOrDefault();
            int backgroundWidth = data.visualBounds != null ? data.visualBounds.width : WIDTH;
            int backgroundHeight = data.visualBounds != null ? data.visualBounds.height : HEIGHT;
            ResourceLocation overlayId = data.textures.overlay != null ? resource(namespace, data.textures.overlay) : null;
            GuiLayout layout = new GuiLayout(data.elements, data.animations, bgId, visualOffsetX, visualOffsetY, backgroundWidth, backgroundHeight, overlayId);
            layout.namespace = namespace;
            return layout;"#
    } else {
        r#"            int visualOffsetX = data.textures.visualOffsetXOrDefault();
            int visualOffsetY = data.textures.visualOffsetYOrDefault();
            int backgroundWidth = data.visualBounds != null ? data.visualBounds.width : WIDTH;
            int backgroundHeight = data.visualBounds != null ? data.visualBounds.height : HEIGHT;
            GuiLayout layout = new GuiLayout(data.elements, data.animations, bgId, visualOffsetX, visualOffsetY, backgroundWidth, backgroundHeight);
            layout.namespace = namespace;
            return layout;"#
    };
    let overlay_render = if has_overlay {
        r#"
    public void renderOverlay(GuiGraphics graphics, int left, int top) {
        if (overlay != null) {
            graphics.blit(overlay, left + visualOffsetX, top + visualOffsetY, 0, 0, backgroundWidth, backgroundHeight, backgroundWidth, backgroundHeight);
        }
    }
"#
    } else {
        r#"
    public void renderOverlay(GuiGraphics graphics, int left, int top) {
        // No overlay elements in this layout
    }
"#
    };

    let progress_body = r#"        for (Element element : elements) {
            if (!element.isVisible() || !animationId.equals(element.animation)) {
                continue;
            }
            int x = left + element.x;
            int y = top + element.y;
            int width = element.widthOrDefault(22);
            int height = element.heightOrDefault(15);
            float ratio = animation.normalize(value);
            String direction = element.directionOrDefault(animation.directionOrDefault());
            if (element.texture != null) {
                ResourceLocation spriteTexture = resource(namespace, element.texture);
                switch (direction) {
                    case "right_to_left" -> graphics.blit(spriteTexture, x + width - Math.round(width * ratio), y, width - Math.round(width * ratio), 0, Math.round(width * ratio), height, width, height);
                    case "bottom_to_top" -> graphics.blit(spriteTexture, x, y + height - Math.round(height * ratio), 0, height - Math.round(height * ratio), width, Math.round(height * ratio), width, height);
                    case "top_to_bottom" -> graphics.blit(spriteTexture, x, y, 0, 0, width, Math.round(height * ratio), width, height);
                    default -> graphics.blit(spriteTexture, x, y, 0, 0, Math.round(width * ratio), height, width, height);
                }
            } else {
                switch (direction) {
                    case "right_to_left" -> graphics.fill(x + width - Math.round(width * ratio), y, x + width, y + height, 0xFFE9A23B);
                    case "bottom_to_top" -> graphics.fill(x, y + height - Math.round(height * ratio), x + width, y + height, 0xFF3B82E9);
                    case "top_to_bottom" -> graphics.fill(x, y, x + width, y + Math.round(height * ratio), 0xFF3B82E9);
                    default -> graphics.fill(x, y, x + Math.round(width * ratio), y + height, 0xFFE9A23B);
                }
            }
        }"#;

    format!(
        r#"package {package};

import com.google.gson.Gson;
import com.google.gson.annotations.SerializedName;
import com.google.gson.reflect.TypeToken;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.resources.ResourceLocation;

public final class GuiLayout {{
    public static final int WIDTH = {width};
    public static final int HEIGHT = {height};
    private final List<Element> elements;
    private final List<Animation> animations;
    private final ResourceLocation texture;
    {overlay_field}
    private final int visualOffsetX;
    private final int visualOffsetY;
    private final int backgroundWidth;
    private final int backgroundHeight;
    private String namespace;

    private GuiLayout(List<Element> elements, List<Animation> animations, ResourceLocation texture, int visualOffsetX, int visualOffsetY, int backgroundWidth, int backgroundHeight{overlay_ctor}) {{
        this.elements = elements == null ? List.of() : elements;
        this.animations = animations == null ? List.of() : animations;
        this.texture = texture;
        this.visualOffsetX = visualOffsetX;
        this.visualOffsetY = visualOffsetY;
        this.backgroundWidth = backgroundWidth;
        this.backgroundHeight = backgroundHeight;
        {overlay_assign}
    }}

    public static ResourceLocation resource(String namespace, String path) {{
        return {resource_location_ctor};
    }}

    public static GuiLayout load(String namespace, String layoutPath, String texturePath) {{
        ResourceLocation layoutId = resource(namespace, layoutPath);
        String classpathResource = "assets/" + layoutId.getNamespace() + "/" + layoutId.getPath();
        try (InputStreamReader reader = new InputStreamReader(
                GuiLayout.class.getClassLoader().getResourceAsStream(classpathResource),
                StandardCharsets.UTF_8)) {{
            Gson gson = new Gson();
            LayoutData data = gson.fromJson(reader, new TypeToken<LayoutData>() {{}}.getType());
            ResourceLocation bgId = resource(namespace, data.textures.background);
{load_layout_body}
        }} catch (Exception error) {{
            throw new IllegalStateException("Failed to load GUI layout " + layoutId, error);
        }}
    }}

    public void renderTexture(GuiGraphics graphics, int left, int top) {{
        graphics.blit(texture, left + visualOffsetX, top + visualOffsetY, 0, 0, backgroundWidth, backgroundHeight, backgroundWidth, backgroundHeight);
    }}
{overlay_render}
    public void renderStaticElements(GuiGraphics graphics, int left, int top) {{
        Font font = Minecraft.getInstance().font;
        for (Element element : elements) {{
            if (!element.isVisible() || "texture".equals(element.type) || "slot".equals(element.type) || "progress".equals(element.type)) {{
                continue;
            }}
            int x = left + element.x;
            int y = top + element.y;
            switch (element.type) {{
                case "text" -> graphics.drawString(font, element.contentOrEmpty(), x, y, element.colorOrDefault(), element.shadowOrDefault());
                case "button", "toggle_button" -> renderButtonLabel(graphics, font, element, x, y);
                case "fluid_tank", "energy_bar" -> renderMeterShell(graphics, x, y, element.widthOrDefault(16), element.heightOrDefault(48));
                default -> {{ }}
            }}
        }}
    }}

    public void renderProgress(String animationId, GuiGraphics graphics, int left, int top, float value) {{
        Animation animation = findAnimation(animationId);
        if (animation == null) {{
            return;
        }}
{progress_body}
    }}

    private Animation findAnimation(String id) {{
        for (Animation animation : animations) {{
            if (id.equals(animation.id)) {{
                return animation;
            }}
        }}
        return null;
    }}

    private static void renderMeterShell(GuiGraphics graphics, int x, int y, int width, int height) {{
        graphics.fill(x, y, x + width, y + height, 0xAA000000);
        graphics.fill(x + 1, y + 1, x + width - 1, y + height - 1, 0x55333333);
    }}

    private static void renderButtonLabel(GuiGraphics graphics, Font font, Element element, int x, int y) {{
        String label = element.contentOrEmpty();
        if (label.isEmpty() || element.hasIcon()) {{
            return;
        }}
        int width = element.widthOrDefault(40);
        int height = element.heightOrDefault(20);
        int labelX = x + (width - font.width(label)) / 2;
        int labelY = y + (height - 8) / 2;
        graphics.drawString(font, label, labelX, labelY, element.colorOrDefault(), element.shadowOrDefault());
    }}

    private static final class LayoutData {{
        @SerializedName("visual_bounds")
        VisualBounds visualBounds;
        TexturesData textures;
        List<Element> elements;
        List<Animation> animations;
    }}

    private static final class TexturesData {{
        String background;
        String overlay;
        @SerializedName("visual_offset_x")
        Integer visualOffsetX;
        @SerializedName("visual_offset_y")
        Integer visualOffsetY;

        int visualOffsetXOrDefault() {{ return visualOffsetX == null ? 0 : visualOffsetX; }}
        int visualOffsetYOrDefault() {{ return visualOffsetY == null ? 0 : visualOffsetY; }}
    }}

    private static final class VisualBounds {{
        int x;
        int y;
        int width;
        int height;
    }}

    private static final class Element {{
        String id;
        String type;
        int x;
        int y;
        Integer width;
        Integer height;
        Integer size;
        String content;
        Integer color;
        Boolean shadow;
        String animation;
        Boolean visible;
        String texture;
        String icon;
        String direction;

        boolean isVisible() {{ return visible == null || visible; }}
        int widthOrDefault(int fallback) {{ return width == null ? fallback : width; }}
        int heightOrDefault(int fallback) {{ return height == null ? fallback : height; }}
        int sizeOrDefault() {{ return size == null ? 18 : size; }}
        String contentOrEmpty() {{ return content == null ? "" : content; }}
        boolean hasIcon() {{ return icon != null && !icon.isEmpty(); }}
        String directionOrDefault(String fallback) {{ return direction == null ? fallback : direction; }}
        int colorOrDefault() {{ return color == null ? 0x404040 : color; }}
        boolean shadowOrDefault() {{ return shadow != null && shadow; }}
    }}

    private static final class Animation {{
        String id;
        @SerializedName("data_key")
        String dataKey;
        String direction;
        @SerializedName("min_value")
        Float minValue;
        @SerializedName("max_value")
        Float maxValue;

        String directionOrDefault() {{ return direction == null ? "left_to_right" : direction; }}

        float normalize(float value) {{
            float min = minValue == null ? 0.0F : minValue;
            float max = maxValue == null ? 1.0F : maxValue;
            if (max <= min) {{
                return 0.0F;
            }}
            return Math.max(0.0F, Math.min(1.0F, (value - min) / (max - min)));
        }}
    }}
}}
"#,
        package = export.package,
        width = project.gui_size.width,
        height = project.gui_size.height,
        resource_location_ctor = resource_location_ctor,
        load_layout_body = load_layout_body,
        progress_body = progress_body
    )
}

fn generate_fabric_layout_java(export: &SanitizedExport, project: &Project) -> String {
    let has_overlay = project
        .elements
        .iter()
        .any(|e| e.visible && e.layer == Layer::Overlay);
    let overlay_field = if has_overlay {
        "private final Identifier overlay;\n    "
    } else {
        ""
    };
    let overlay_ctor = if has_overlay {
        ", Identifier overlay"
    } else {
        ""
    };
    let overlay_assign = if has_overlay {
        "this.overlay = overlay;\n        "
    } else {
        ""
    };
    let layout_ctor_args = if has_overlay {
        "data.elements, data.animations, bgId, visualOffsetX, visualOffsetY, backgroundWidth, backgroundHeight, overlayId"
    } else {
        "data.elements, data.animations, bgId, visualOffsetX, visualOffsetY, backgroundWidth, backgroundHeight"
    };
    let overlay_render = if has_overlay {
        r#"
    public void renderOverlay(DrawContext context, int left, int top) {
        if (overlay != null) {
            context.drawTexture(overlay, left + visualOffsetX, top + visualOffsetY, 0, 0, backgroundWidth, backgroundHeight, backgroundWidth, backgroundHeight);
        }
    }
"#
    } else {
        r#"
    public void renderOverlay(DrawContext context, int left, int top) {
        // No overlay elements in this layout
    }
"#
    };

    let progress_body = r#"        for (Element element : elements) {
            if (!element.isVisible() || !animationId.equals(element.animation)) {
                continue;
            }
            int x = left + element.x;
            int y = top + element.y;
            int width = element.widthOrDefault(22);
            int height = element.heightOrDefault(15);
            float ratio = animation.normalize(value);
            String direction = element.directionOrDefault(animation.directionOrDefault());
            if (element.texture != null) {
                Identifier spriteTexture = resource(namespace, element.texture);
                switch (direction) {
                    case "right_to_left" -> context.drawTexture(spriteTexture, x + width - Math.round(width * ratio), y, width - Math.round(width * ratio), 0, Math.round(width * ratio), height, width, height);
                    case "bottom_to_top" -> context.drawTexture(spriteTexture, x, y + height - Math.round(height * ratio), 0, height - Math.round(height * ratio), width, Math.round(height * ratio), width, height);
                    case "top_to_bottom" -> context.drawTexture(spriteTexture, x, y, 0, 0, width, Math.round(height * ratio), width, height);
                    default -> context.drawTexture(spriteTexture, x, y, 0, 0, Math.round(width * ratio), height, width, height);
                }
            } else {
                switch (direction) {
                    case "right_to_left" -> context.fill(x + width - Math.round(width * ratio), y, x + width, y + height, 0xFFE9A23B);
                    case "bottom_to_top" -> context.fill(x, y + height - Math.round(height * ratio), x + width, y + height, 0xFF3B82E9);
                    case "top_to_bottom" -> context.fill(x, y, x + width, y + Math.round(height * ratio), 0xFF3B82E9);
                    default -> context.fill(x, y, x + Math.round(width * ratio), y + height, 0xFFE9A23B);
                }
            }
        }"#;

    format!(
        r#"package {package};

import com.google.gson.Gson;
import com.google.gson.annotations.SerializedName;
import com.google.gson.reflect.TypeToken;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.List;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.util.Identifier;

public final class GuiLayout {{
    public static final int WIDTH = {width};
    public static final int HEIGHT = {height};
    private final List<Element> elements;
    private final List<Animation> animations;
    private final Identifier texture;
    {overlay_field}
    private final int visualOffsetX;
    private final int visualOffsetY;
    private final int backgroundWidth;
    private final int backgroundHeight;
    private String namespace;

    private GuiLayout(List<Element> elements, List<Animation> animations, Identifier texture, int visualOffsetX, int visualOffsetY, int backgroundWidth, int backgroundHeight{overlay_ctor}) {{
        this.elements = elements == null ? List.of() : elements;
        this.animations = animations == null ? List.of() : animations;
        this.texture = texture;
        this.visualOffsetX = visualOffsetX;
        this.visualOffsetY = visualOffsetY;
        this.backgroundWidth = backgroundWidth;
        this.backgroundHeight = backgroundHeight;
        {overlay_assign}
    }}

    public static Identifier resource(String namespace, String path) {{
        return Identifier.of(namespace, path);
    }}

    public static GuiLayout load(String namespace, String layoutPath, String texturePath) {{
        Identifier layoutId = resource(namespace, layoutPath);
        String classpathResource = "assets/" + layoutId.getNamespace() + "/" + layoutId.getPath();
        try (InputStreamReader reader = new InputStreamReader(
                GuiLayout.class.getClassLoader().getResourceAsStream(classpathResource),
                StandardCharsets.UTF_8)) {{
            Gson gson = new Gson();
            LayoutData data = gson.fromJson(reader, new TypeToken<LayoutData>() {{}}.getType());
            Identifier bgId = resource(namespace, data.textures.background);
            int visualOffsetX = data.textures.visualOffsetXOrDefault();
            int visualOffsetY = data.textures.visualOffsetYOrDefault();
            int backgroundWidth = data.visualBounds != null ? data.visualBounds.width : WIDTH;
            int backgroundHeight = data.visualBounds != null ? data.visualBounds.height : HEIGHT;
            Identifier overlayId = data.textures.overlay != null ? resource(namespace, data.textures.overlay) : null;
            GuiLayout layout = new GuiLayout({layout_ctor_args});
            layout.namespace = namespace;
            return layout;
        }} catch (Exception error) {{
            throw new IllegalStateException("Failed to load GUI layout " + layoutId, error);
        }}
    }}

    public void renderTexture(DrawContext context, int left, int top) {{
        context.drawTexture(texture, left + visualOffsetX, top + visualOffsetY, 0, 0, backgroundWidth, backgroundHeight, backgroundWidth, backgroundHeight);
    }}
{overlay_render}
    public void renderStaticElements(DrawContext context, int left, int top) {{
        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;
        for (Element element : elements) {{
            if (!element.isVisible() || "texture".equals(element.type) || "slot".equals(element.type) || "progress".equals(element.type)) {{
                continue;
            }}
            int x = left + element.x;
            int y = top + element.y;
            switch (element.type) {{
                case "text" -> context.drawText(textRenderer, element.contentOrEmpty(), x, y, element.colorOrDefault(), element.shadowOrDefault());
                case "button", "toggle_button" -> renderButtonLabel(context, textRenderer, element, x, y);
                case "fluid_tank", "energy_bar" -> renderMeterShell(context, x, y, element.widthOrDefault(16), element.heightOrDefault(48));
                default -> {{ }}
            }}
        }}
    }}

    public void renderProgress(String animationId, DrawContext context, int left, int top, float value) {{
        Animation animation = findAnimation(animationId);
        if (animation == null) {{
            return;
        }}
{progress_body}
    }}

    private Animation findAnimation(String id) {{
        for (Animation animation : animations) {{
            if (id.equals(animation.id)) {{
                return animation;
            }}
        }}
        return null;
    }}

    private static void renderMeterShell(DrawContext context, int x, int y, int width, int height) {{
        context.fill(x, y, x + width, y + height, 0xAA000000);
        context.fill(x + 1, y + 1, x + width - 1, y + height - 1, 0x55333333);
    }}

    private static void renderButtonLabel(DrawContext context, TextRenderer textRenderer, Element element, int x, int y) {{
        String label = element.contentOrEmpty();
        if (label.isEmpty() || element.hasIcon()) {{
            return;
        }}
        int width = element.widthOrDefault(40);
        int height = element.heightOrDefault(20);
        int labelX = x + (width - textRenderer.getWidth(label)) / 2;
        int labelY = y + (height - 8) / 2;
        context.drawText(textRenderer, label, labelX, labelY, element.colorOrDefault(), element.shadowOrDefault());
    }}

    private static final class LayoutData {{
        @SerializedName("visual_bounds")
        VisualBounds visualBounds;
        TexturesData textures;
        List<Element> elements;
        List<Animation> animations;
    }}

    private static final class TexturesData {{
        String background;
        String overlay;
        @SerializedName("visual_offset_x")
        Integer visualOffsetX;
        @SerializedName("visual_offset_y")
        Integer visualOffsetY;

        int visualOffsetXOrDefault() {{ return visualOffsetX == null ? 0 : visualOffsetX; }}
        int visualOffsetYOrDefault() {{ return visualOffsetY == null ? 0 : visualOffsetY; }}
    }}

    private static final class VisualBounds {{
        int x;
        int y;
        int width;
        int height;
    }}

    private static final class Element {{
        String id;
        String type;
        int x;
        int y;
        Integer width;
        Integer height;
        Integer size;
        String content;
        Integer color;
        Boolean shadow;
        String animation;
        Boolean visible;
        String texture;
        String icon;
        String direction;

        boolean isVisible() {{ return visible == null || visible; }}
        int widthOrDefault(int fallback) {{ return width == null ? fallback : width; }}
        int heightOrDefault(int fallback) {{ return height == null ? fallback : height; }}
        int sizeOrDefault() {{ return size == null ? 18 : size; }}
        String contentOrEmpty() {{ return content == null ? "" : content; }}
        boolean hasIcon() {{ return icon != null && !icon.isEmpty(); }}
        String directionOrDefault(String fallback) {{ return direction == null ? fallback : direction; }}
        int colorOrDefault() {{ return color == null ? 0x404040 : color; }}
        boolean shadowOrDefault() {{ return shadow != null && shadow; }}
    }}

    private static final class Animation {{
        String id;
        @SerializedName("data_key")
        String dataKey;
        String direction;
        @SerializedName("min_value")
        Float minValue;
        @SerializedName("max_value")
        Float maxValue;

        String directionOrDefault() {{ return direction == null ? "left_to_right" : direction; }}

        float normalize(float value) {{
            float min = minValue == null ? 0.0F : minValue;
            float max = maxValue == null ? 1.0F : maxValue;
            if (max <= min) {{
                return 0.0F;
            }}
            return Math.max(0.0F, Math.min(1.0F, (value - min) / (max - min)));
        }}
    }}
}}
"#,
        package = export.package,
        width = project.gui_size.width,
        height = project.gui_size.height,
        overlay_field = overlay_field,
        overlay_ctor = overlay_ctor,
        overlay_assign = overlay_assign,
        layout_ctor_args = layout_ctor_args,
        overlay_render = overlay_render,
        progress_body = progress_body
    )
}

fn generate_forge_screen(export: &SanitizedExport, project: &Project) -> String {
    format!(
        r#"package {package};

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.inventory.AbstractContainerMenu;

public class {screen_class_name} extends AbstractContainerScreen<AbstractContainerMenu> {{
    private GuiLayout layout;

    public {screen_class_name}(AbstractContainerMenu menu, Inventory inventory, Component title) {{
        super(menu, inventory, title);
        this.imageWidth = {width};
        this.imageHeight = {height};
    }}

    @Override
    protected void init() {{
        super.init();
        this.layout = GuiLayout.load("{mod_id}", "gui/{resource_name}_layout.json", "textures/gui/{resource_name}_gui.png");
    }}

    @Override
    protected void renderBg(GuiGraphics graphics, float partialTick, int mouseX, int mouseY) {{
        layout.renderTexture(graphics, leftPos, topPos);
        layout.renderOverlay(graphics, leftPos, topPos);
        layout.renderStaticElements(graphics, leftPos, topPos);
{animation_hooks}
    }}

    @Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float partialTick) {{
        this.renderBackground(graphics, mouseX, mouseY, partialTick);
        super.render(graphics, mouseX, mouseY, partialTick);
        this.renderTooltip(graphics, mouseX, mouseY);
    }}
}}
"#,
        package = export.package,
        screen_class_name = export.screen_class_name,
        width = project.gui_size.width,
        height = project.gui_size.height,
        mod_id = export.mod_id,
        resource_name = export.resource_name,
        animation_hooks =
            generate_animation_hooks(&project.animations, "graphics", "leftPos", "topPos")
    )
}

fn generate_neoforge_screen(export: &SanitizedExport, project: &Project) -> String {
    format!(
        r#"package {package};

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.inventory.AbstractContainerMenu;

public class {screen_class_name} extends AbstractContainerScreen<AbstractContainerMenu> {{
    private GuiLayout layout;

    public {screen_class_name}(AbstractContainerMenu menu, Inventory inventory, Component title) {{
        super(menu, inventory, title);
        this.imageWidth = {width};
        this.imageHeight = {height};
    }}

    @Override
    protected void init() {{
        super.init();
        this.layout = GuiLayout.load("{mod_id}", "gui/{resource_name}_layout.json", "textures/gui/{resource_name}_gui.png");
    }}

    @Override
    protected void renderBg(GuiGraphics graphics, float partialTick, int mouseX, int mouseY) {{
        layout.renderTexture(graphics, leftPos, topPos);
        layout.renderOverlay(graphics, leftPos, topPos);
        layout.renderStaticElements(graphics, leftPos, topPos);
{animation_hooks}
    }}

    @Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float partialTick) {{
        this.renderBackground(graphics, mouseX, mouseY, partialTick);
        super.render(graphics, mouseX, mouseY, partialTick);
        this.renderTooltip(graphics, mouseX, mouseY);
    }}
}}
"#,
        package = export.package,
        screen_class_name = export.screen_class_name,
        width = project.gui_size.width,
        height = project.gui_size.height,
        mod_id = export.mod_id,
        resource_name = export.resource_name,
        animation_hooks =
            generate_animation_hooks(&project.animations, "graphics", "leftPos", "topPos")
    )
}

fn generate_fabric_screen(export: &SanitizedExport, project: &Project) -> String {
    format!(
        r#"package {package};

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.ingame.HandledScreen;
import net.minecraft.entity.player.PlayerInventory;
import net.minecraft.screen.ScreenHandler;
import net.minecraft.text.Text;

public class {screen_class_name} extends HandledScreen<ScreenHandler> {{
    private GuiLayout layout;

    public {screen_class_name}(ScreenHandler handler, PlayerInventory inventory, Text title) {{
        super(handler, inventory, title);
        this.backgroundWidth = {width};
        this.backgroundHeight = {height};
    }}

    @Override
    protected void init() {{
        super.init();
        this.layout = GuiLayout.load("{mod_id}", "gui/{resource_name}_layout.json", "textures/gui/{resource_name}_gui.png");
    }}

    @Override
    protected void drawBackground(DrawContext context, float delta, int mouseX, int mouseY) {{
        layout.renderTexture(context, x, y);
        layout.renderOverlay(context, x, y);
        layout.renderStaticElements(context, x, y);
{animation_hooks}
    }}

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {{
        this.renderBackground(context, mouseX, mouseY, delta);
        super.render(context, mouseX, mouseY, delta);
        this.drawMouseoverTooltip(context, mouseX, mouseY);
    }}
}}
"#,
        package = export.package,
        screen_class_name = export.screen_class_name,
        width = project.gui_size.width,
        height = project.gui_size.height,
        mod_id = export.mod_id,
        resource_name = export.resource_name,
        animation_hooks = generate_animation_hooks(&project.animations, "context", "x", "y")
    )
}

fn generate_animation_hooks(
    animations: &[Animation],
    graphics_name: &str,
    left_name: &str,
    top_name: &str,
) -> String {
    if animations.is_empty() {
        return "        // No project animations are defined.".to_string();
    }

    let mut hooks = String::new();
    hooks.push_str("        // Bind these generated hooks to your menu/screen-handler data.\n");
    hooks.push_str("        // The default value keeps the exported project compiling before game-state wiring.\n");
    for animation in animations {
        hooks.push_str(&format!(
            "        layout.renderProgress(\"{}\", {}, {}, {}, 0.0F); // data key: {}\n",
            escape_java_string(&animation.id),
            graphics_name,
            left_name,
            top_name,
            escape_java_string(&animation.data_key)
        ));
    }
    hooks.trim_end().to_string()
}

fn generate_client_entrypoint(export: &SanitizedExport, target: ExportTarget) -> String {
    match target {
        ExportTarget::Fabric => format!(
            r#"package {package};

import net.fabricmc.api.ClientModInitializer;

public final class {class_name}Client implements ClientModInitializer {{
    @Override
    public void onInitializeClient() {{
        // Register {screen_class_name} with your ScreenHandlerType here.
    }}
}}
"#,
            package = export.package,
            class_name = export.class_name,
            screen_class_name = export.screen_class_name
        ),
        ExportTarget::Forge => format!(
            r#"package {package};

import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.fml.common.Mod;

@Mod.EventBusSubscriber(modid = "{mod_id}", value = Dist.CLIENT, bus = Mod.EventBusSubscriber.Bus.MOD)
public final class {class_name}Client {{
    private {class_name}Client() {{
    }}

    // Register {screen_class_name} with MenuScreens.register(...) from your client setup event.
}}
"#,
            package = export.package,
            class_name = export.class_name,
            screen_class_name = export.screen_class_name,
            mod_id = export.mod_id
        ),
        ExportTarget::NeoForge => format!(
            r#"package {package};

import net.neoforged.api.distmarker.Dist;
import net.neoforged.fml.common.EventBusSubscriber;

@EventBusSubscriber(modid = "{mod_id}", value = Dist.CLIENT, bus = EventBusSubscriber.Bus.MOD)
public final class {class_name}Client {{
    private {class_name}Client() {{
    }}

    // Register {screen_class_name} with MenuScreens.register(...) from your client setup event.
}}
"#,
            package = export.package,
            class_name = export.class_name,
            screen_class_name = export.screen_class_name,
            mod_id = export.mod_id
        ),
    }
}

fn loader_metadata_path(export: &SanitizedExport, target: ExportTarget) -> PathBuf {
    match target {
        ExportTarget::Fabric => export.output_dir.join("src/main/resources/fabric.mod.json"),
        ExportTarget::Forge => export
            .output_dir
            .join("src/main/resources/META-INF/mods.toml"),
        ExportTarget::NeoForge => export
            .output_dir
            .join("src/main/resources/META-INF/neoforge.mods.toml"),
    }
}

fn generate_loader_metadata(export: &SanitizedExport, target: ExportTarget) -> String {
    match target {
        ExportTarget::Fabric => format!(
            r#"{{
  "schemaVersion": 1,
  "id": "{mod_id}",
  "version": "${{version}}",
  "name": "{class_name} GUI",
  "environment": "client",
  "entrypoints": {{
    "client": ["{package}.{class_name}Client"]
  }},
  "depends": {{
    "fabricloader": ">=0.16.0",
    "minecraft": ">=1.21"
  }}
}}
"#,
            mod_id = export.mod_id,
            package = export.package,
            class_name = export.class_name
        ),
        ExportTarget::Forge => generate_toml_metadata(export, "Forge", "forge", "[52,)"),
        ExportTarget::NeoForge => generate_toml_metadata(export, "NeoForge", "neoforge", "[21,)"),
    }
}

fn generate_toml_metadata(
    export: &SanitizedExport,
    loader_name: &str,
    loader_id: &str,
    loader_range: &str,
) -> String {
    format!(
        r#"modLoader="javafml"
loaderVersion="{loader_range}"
license="MIT"

[[mods]]
modId="{mod_id}"
version="${{file.jarVersion}}"
displayName="{class_name} GUI"
description='''Generated {loader_name} GUI export from MCGUI Crafter.'''

[[dependencies.{mod_id}]]
modId="{loader_id}"
mandatory=true
versionRange="{loader_range}"
ordering="NONE"
side="CLIENT"

[[dependencies.{mod_id}]]
modId="minecraft"
mandatory=true
versionRange="[1.21,)"
ordering="NONE"
side="CLIENT"
"#,
        loader_name = loader_name,
        loader_id = loader_id,
        loader_range = loader_range,
        mod_id = export.mod_id,
        class_name = export.class_name
    )
}

fn generate_readme(export: &SanitizedExport, target: ExportTarget, project: &Project) -> String {
    format!(
        r#"MCGUI Crafter Export - {class_name}
================================

Loader: {loader}
Mod ID: {mod_id}
Package: {package}
GUI size: {width}x{height}

This export is a minimal Minecraft mod project scaffold:
  settings.gradle
  build.gradle
  gradle.properties
  src/main/java/{package_path}/GuiLayout.java
  src/main/java/{package_path}/{screen_class_name}.java
  src/main/java/{package_path}/{class_name}Client.java
  src/main/resources/assets/{mod_id}/textures/gui/{resource_name}_gui.png
  src/main/resources/assets/{mod_id}/gui/{resource_name}_layout.json

Texture elements are composited into the GUI PNG. Referenced source PNG assets are also copied under assets/{mod_id}/textures/... so they are available for later hand edits.

The generated screen and runtime are designed to compile against the listed loader metadata and Gradle dependencies. Menu or ScreenHandler registration remains app-specific, so wire {screen_class_name} into your own menu type and replace generated animation default values with menu data where needed.

Animation hooks:
{animation_summary}
"#,
        class_name = export.class_name,
        screen_class_name = export.screen_class_name,
        loader = target.loader_name(),
        mod_id = export.mod_id,
        package = export.package,
        package_path = export.package_path,
        width = project.gui_size.width,
        height = project.gui_size.height,
        resource_name = export.resource_name,
        animation_summary = animation_summary(&project.animations)
    )
}

fn animation_summary(animations: &[Animation]) -> String {
    if animations.is_empty() {
        return "  none".to_string();
    }
    animations
        .iter()
        .map(|animation| format!("  {} -> {}", animation.id, animation.data_key))
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_java_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{Animation, AnimationType};
    use crate::project::{
        AttachedRegion, AttachedRegionAnchor, AttachedRegionState, Element, ElementType,
        FillDirection, Layer, ModTarget, SemanticGroup, SemanticGroupKind, Size, SlotRole, UvRect,
    };
    use image::{Rgba, RgbaImage};
    use std::collections::HashMap;

    fn sample_project(target: ModTarget) -> Project {
        let texture_asset = "textures/widgets/panel.png".to_string();
        let progress_asset = "textures/widgets/progress.png".to_string();
        let mut texture_data = HashMap::new();
        texture_data.insert(texture_asset.clone(), png_bytes([40, 80, 120, 255]));
        texture_data.insert(progress_asset.clone(), png_bytes([240, 180, 40, 255]));

        Project {
            name: "Sample GUI".to_string(),
            gui_size: Size {
                width: 187,
                height: 173,
            },
            mod_target: target,
            elements: vec![
                Element {
                    id: "background".to_string(),
                    element_type: ElementType::Texture,
                    x: 0,
                    y: 0,
                    width: Some(187),
                    height: Some(173),
                    size: None,
                    asset: Some(texture_asset.clone()),
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
                },
                Element {
                    id: "slot_1".to_string(),
                    element_type: ElementType::Slot,
                    x: 8,
                    y: 18,
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
                },
                Element {
                    id: "progress_arrow".to_string(),
                    element_type: ElementType::Progress,
                    x: 80,
                    y: 35,
                    width: Some(24),
                    height: Some(16),
                    size: None,
                    asset: None,
                    icon: None,
                    icon_uv: None,
                    tooltip: None,
                    direction: Some(FillDirection::LeftToRight),
                    content: None,
                    font: None,
                    color: None,
                    shadow: None,
                    animation: Some("cook_progress".to_string()),
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
                },
            ],
            groups: Vec::new(),
            animations: vec![Animation {
                id: "cook_progress".to_string(),
                animation_type: AnimationType::Fill,
                data_key: "cook_time".to_string(),
                texture: Some(progress_asset),
                direction: Some(FillDirection::LeftToRight),
                frame_count: None,
                fps: None,
                min_value: Some(0.0),
                max_value: Some(100.0),
                triggers_on: None,
            }],
            assets: vec![texture_asset],
            asset_metadata: HashMap::new(),
            semantic_groups: Vec::new(),
            attached_regions: Vec::new(),
            export_settings: crate::project::ProjectExportSettings::default(),
            project_path: None,
            is_dirty: true,
            texture_data,
            fonts: Vec::new(),
        }
    }

    fn layered_project(target: ModTarget) -> Project {
        let mut project = sample_project(target);
        project.elements.push(Element {
            id: "title".to_string(),
            element_type: ElementType::Text,
            x: 8,
            y: 6,
            width: None,
            height: None,
            size: None,
            asset: None,
            icon: None,
            icon_uv: None,
            tooltip: None,
            direction: None,
            content: Some("Layered".to_string()),
            font: Some("minecraft:default".to_string()),
            color: Some(0x404040),
            shadow: Some(true),
            animation: None,
            visible: true,
            uv: None,
            render_mode: crate::project::TextureRenderMode::Plain,
            nine_slice: None,
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
            attached_region: None,
        });
        if let Some(progress) = project
            .elements
            .iter_mut()
            .find(|e| e.id == "progress_arrow")
        {
            progress.layer = Layer::Animatable;
        }
        project
    }

    fn png_bytes(color: [u8; 4]) -> Vec<u8> {
        png_bytes_with_size(2, 2, color)
    }

    fn png_bytes_with_size(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(width, height, Rgba(color));
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }

    fn temp_export_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gui-crafter-export-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct TempExportDir {
        path: PathBuf,
    }

    impl TempExportDir {
        fn new(name: &str) -> Self {
            Self {
                path: temp_export_dir(name),
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempExportDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn export_sample(target: &str, project_target: ModTarget) -> (PathBuf, Vec<String>) {
        let output_dir = temp_export_dir(target);
        let config = ExportConfig {
            mod_id: "Bad Mod.ID".to_string(),
            package: "com.example.1bad.class".to_string(),
            class_name: "123 Furnace GUI".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let files = export_project(&sample_project(project_target), &config, target).unwrap();
        (output_dir, files)
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    fn textures_json_for_test() -> serde_json::Value {
        serde_json::json!({
            "background": "textures/gui/scrollable_gui.png",
        })
    }

    fn button_element(
        id: &str,
        element_type: ElementType,
        x: i32,
        y: i32,
        content: Option<&str>,
    ) -> Element {
        Element {
            id: id.to_string(),
            element_type,
            x,
            y,
            width: Some(40),
            height: Some(20),
            size: None,
            asset: None,
            icon: None,
            icon_uv: None,
            tooltip: None,
            direction: None,
            content: content.map(str::to_string),
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

    fn semantic_slot_element(
        id: &str,
        slot_role: SlotRole,
        inventory_group: &str,
        slot_index: u32,
    ) -> Element {
        Element {
            id: id.to_string(),
            element_type: ElementType::Slot,
            x: 8 + (slot_index as i32 * 18),
            y: 18,
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
            slot_role: Some(slot_role),
            slot_index: Some(slot_index),
            inventory_group: Some(inventory_group.to_string()),
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

    fn scrollbar_element(id: &str, target_group: &str, binding: &str) -> Element {
        Element {
            id: id.to_string(),
            element_type: ElementType::Scrollbar,
            x: 100,
            y: 18,
            width: Some(12),
            height: Some(54),
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
            scroll_binding: Some(binding.to_string()),
            scroll_min: Some(0),
            scroll_max: Some(3),
            visible_rows: Some(3),
            total_rows: Some(6),
            columns: Some(5),
            target_group: Some(target_group.to_string()),
            binding: None,
            dock: None,
            open_width: None,
            open_height: None,
            attached_region: None,
        }
    }

    #[test]
    fn layout_json_contains_semantic_groups_and_export_settings() {
        let mut project = Project::new("Scrollable", 176, 166, ModTarget::Forge);
        crate::templates::apply_template(&mut project, "scrollable_inventory_machine").unwrap();
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.export_settings.generate_semantic_registry = true;

        let layout = layout_json_value(&project, textures_json_for_test());

        assert_eq!(layout["semantic_groups"][0]["id"], "machine_buffer");
        assert_eq!(layout["export_settings"]["codegen_mode"], "modular");
    }

    #[test]
    fn layout_json_includes_visual_bounds_offsets_and_attached_regions() {
        let mut project = Project::new("Returns", 100, 80, ModTarget::Forge);
        project.attached_regions.push(AttachedRegion {
            id: "returns_pocket".to_string(),
            anchor: AttachedRegionAnchor::Right,
            x: 100,
            y: 18,
            width: 54,
            height: 72,
            state: AttachedRegionState::Static,
            kind: Some("returns_pocket".to_string()),
            semantic_group: Some("food_returns".to_string()),
            visible: true,
        });

        let layout = layout_json_value(
            &project,
            serde_json::json!({ "background": "textures/gui/returns_gui.png" }),
        );

        assert_eq!(layout["visual_bounds"]["x"], 0);
        assert_eq!(layout["visual_bounds"]["y"], 0);
        assert_eq!(layout["visual_bounds"]["width"], 154);
        assert_eq!(layout["visual_bounds"]["height"], 90);
        assert_eq!(layout["textures"]["visual_offset_x"], 0);
        assert_eq!(layout["textures"]["visual_offset_y"], 0);
        assert_eq!(layout["attached_regions"][0]["id"], "returns_pocket");
        assert_eq!(layout["attached_regions"][0]["kind"], "returns_pocket");
        assert_eq!(
            layout["attached_regions"][0]["semantic_group"],
            "food_returns"
        );
    }

    #[test]
    fn layout_json_preserves_button_icon_and_tooltip_metadata() {
        let mut project = Project::new(
            "Button Metadata",
            176,
            166,
            crate::project::ModTarget::Forge,
        );
        let mut button = button_element("settings", ElementType::Button, 8, 8, Some("Settings"));
        button.icon = Some("textures/gui/widgets.png".into());
        button.icon_uv = Some(crate::project::UvRect {
            x: 16,
            y: 0,
            width: 16,
            height: 16,
        });
        button.tooltip = Some("Open settings".into());
        project.elements.push(button);

        let layout = layout_json_value(
            &project,
            serde_json::json!({ "background": "textures/gui/button_metadata_gui.png" }),
        );
        let element = &layout["elements"][0];

        assert_eq!(element["icon"], "textures/gui/widgets.png");
        assert_eq!(element["icon_uv"]["x"], 16);
        assert_eq!(element["icon_uv"]["y"], 0);
        assert_eq!(element["icon_uv"]["width"], 16);
        assert_eq!(element["icon_uv"]["height"], 16);
        assert_eq!(element["tooltip"], "Open settings");
    }

    #[test]
    fn layout_json_omits_generated_texture_for_hidden_animatable_elements() {
        let mut project = layered_project(ModTarget::Forge);
        let progress = project
            .elements
            .iter_mut()
            .find(|element| element.id == "progress_arrow")
            .unwrap();
        progress.visible = false;

        let layout = layout_json_value(&project, textures_json_for_test());
        let progress_json = layout["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|element| element["id"] == "progress_arrow")
            .unwrap();

        assert!(progress_json.get("texture").is_none());
    }

    #[test]
    fn loader_metadata_data_preserves_fabric_json_but_trims_toml_text() {
        let metadata = "line with trailing spaces   \nnext line\t\n".to_string();

        assert_eq!(
            loader_metadata_data(metadata.clone(), ExportTarget::Fabric),
            metadata.as_bytes()
        );
        assert_eq!(
            loader_metadata_data(metadata, ExportTarget::Forge),
            b"line with trailing spaces\nnext line\n"
        );
    }

    #[test]
    fn effective_export_settings_normalizes_simple_semantic_registry_flag() {
        let mut project = Project::new("Simple", 176, 166, ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Simple;
        project.export_settings.generate_semantic_registry = true;
        let config = ExportConfig {
            mod_id: "demo".into(),
            package: "com.example.demo".into(),
            class_name: "SimpleGui".into(),
            output_dir: "/tmp/gui-crafter-export-normalized".into(),
            settings_override: None,
            overwrite: false,
        };

        let settings = effective_export_settings(&project, &config);
        let layout_project = project_with_effective_settings(&project, &settings);
        let layout = layout_json_value(&layout_project, textures_json_for_test());

        assert_eq!(settings.codegen_mode, CodegenMode::Simple);
        assert!(!settings.generate_semantic_registry);
        assert_eq!(
            layout["export_settings"]["generate_semantic_registry"],
            false
        );
    }

    #[test]
    fn modular_semantic_warnings_stop_after_no_groups_warning() {
        let mut project = sample_project(ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.elements[0].element_type = ElementType::Panel;
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert_eq!(
            warnings,
            vec!["Modular code generation is enabled, but the project has no semantic groups."]
        );
    }

    #[test]
    fn modular_semantic_warnings_report_slot_count_mismatch() {
        let mut project = sample_project(ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.elements.clear();
        project.semantic_groups.push(SemanticGroup {
            id: "player_inventory".to_string(),
            kind: SemanticGroupKind::PlayerInventory,
            columns: Some(9),
            visible_rows: Some(3),
            total_rows: Some(3),
            slot_count: Some(27),
            member_ids: Vec::new(),
            data_source: Some("player_inventory".to_string()),
            scroll_binding: None,
            dynamic_height: false,
        });
        for index in 0..6 {
            project.elements.push(semantic_slot_element(
                &format!("player_slot_{index}"),
                SlotRole::PlayerInventory,
                "player_inventory",
                index,
            ));
        }
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(warnings.iter().any(|warning| {
            warning.contains("player_inventory")
                && warning.contains("declares 27 slots")
                && warning.contains("6 matching")
        }));

        let output_dir = TempExportDir::new("preview-semantic-slot-count");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "SemanticGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let preview = preview_export(&project, &config, "forge").unwrap();

        assert!(preview.warnings.iter().any(|warning| {
            warning.contains("player_inventory")
                && warning.contains("declares 27 slots")
                && warning.contains("6 matching")
        }));
    }

    #[test]
    fn modular_semantic_warnings_report_extra_slot_count_mismatch() {
        let mut project = sample_project(ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.elements.clear();
        project.semantic_groups.push(SemanticGroup {
            id: "player_inventory".to_string(),
            kind: SemanticGroupKind::PlayerInventory,
            columns: Some(9),
            visible_rows: Some(3),
            total_rows: Some(3),
            slot_count: Some(27),
            member_ids: Vec::new(),
            data_source: Some("player_inventory".to_string()),
            scroll_binding: None,
            dynamic_height: false,
        });
        for index in 0..28 {
            project.elements.push(semantic_slot_element(
                &format!("player_slot_{index}"),
                SlotRole::PlayerInventory,
                "player_inventory",
                index,
            ));
        }
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(warnings.iter().any(|warning| {
            warning.contains("player_inventory")
                && warning.contains("declares 27 slots")
                && warning.contains("28 matching")
        }));

        let output_dir = TempExportDir::new("preview-semantic-extra-slot-count");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "SemanticGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let preview = preview_export(&project, &config, "forge").unwrap();

        assert!(preview.warnings.iter().any(|warning| {
            warning.contains("player_inventory")
                && warning.contains("declares 27 slots")
                && warning.contains("28 matching")
        }));
    }

    #[test]
    fn modular_semantic_warnings_ignore_non_scrollable_group_bindings() {
        let mut project = sample_project(ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.elements.clear();
        project.semantic_groups.extend([
            SemanticGroup {
                id: "search_field".to_string(),
                kind: SemanticGroupKind::SearchField,
                columns: None,
                visible_rows: None,
                total_rows: None,
                slot_count: None,
                member_ids: Vec::new(),
                data_source: Some("query".to_string()),
                scroll_binding: Some("search_scroll_metadata".to_string()),
                dynamic_height: false,
            },
            SemanticGroup {
                id: "control_buttons".to_string(),
                kind: SemanticGroupKind::ControlButtons,
                columns: None,
                visible_rows: None,
                total_rows: None,
                slot_count: None,
                member_ids: Vec::new(),
                data_source: Some("controls".to_string()),
                scroll_binding: Some("button_scroll_metadata".to_string()),
                dynamic_height: false,
            },
        ]);
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(
            warnings
                .iter()
                .all(|warning| !warning.contains("no matching scrollbar")),
            "non-scrollable semantic group metadata should not require scrollbars: {warnings:?}"
        );
    }

    #[test]
    fn modular_semantic_warnings_report_scroll_binding_mismatches() {
        let mut project = sample_project(ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.elements.clear();
        project.semantic_groups.extend([
            SemanticGroup {
                id: "missing_scrollbar".to_string(),
                kind: SemanticGroupKind::VirtualSlotGrid,
                columns: Some(5),
                visible_rows: Some(3),
                total_rows: Some(6),
                slot_count: Some(30),
                member_ids: Vec::new(),
                data_source: Some("missing_scrollbar".to_string()),
                scroll_binding: Some("missing_scroll".to_string()),
                dynamic_height: false,
            },
            SemanticGroup {
                id: "missing_binding".to_string(),
                kind: SemanticGroupKind::VirtualSlotGrid,
                columns: Some(5),
                visible_rows: Some(3),
                total_rows: Some(6),
                slot_count: Some(30),
                member_ids: Vec::new(),
                data_source: Some("missing_binding".to_string()),
                scroll_binding: None,
                dynamic_height: true,
            },
        ]);
        project.elements.push(scrollbar_element(
            "unrelated_scroll",
            "other_group",
            "other_scroll",
        ));
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(warnings.iter().any(|warning| {
            warning.contains("missing_scrollbar")
                && warning.contains("missing_scroll")
                && warning.contains("no matching scrollbar")
        }));
        assert!(warnings.iter().any(|warning| {
            warning.contains("missing_binding") && warning.contains("has no scroll binding")
        }));
    }

    #[test]
    fn preview_warns_for_specific_control_buttons_group_without_buttons() {
        let mut project = Project::new("Control Buttons", 176, 166, ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.semantic_groups.push(SemanticGroup {
            id: "settings".into(),
            kind: SemanticGroupKind::ControlButtons,
            columns: None,
            visible_rows: None,
            total_rows: None,
            slot_count: Some(1),
            member_ids: Vec::new(),
            data_source: Some("settings".into()),
            scroll_binding: None,
            dynamic_height: false,
        });
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(warnings
            .iter()
            .any(|warning| warning.contains("settings") && warning.contains("button")));
    }

    #[test]
    fn preview_warns_for_control_buttons_without_data_source_when_unbound_button_exists() {
        let mut project = Project::new("Control Buttons", 176, 166, ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.semantic_groups.push(SemanticGroup {
            id: "settings".into(),
            kind: SemanticGroupKind::ControlButtons,
            columns: None,
            visible_rows: None,
            total_rows: None,
            slot_count: Some(1),
            member_ids: Vec::new(),
            data_source: None,
            scroll_binding: None,
            dynamic_height: false,
        });
        project.elements.push(button_element(
            "unrelated",
            ElementType::Button,
            8,
            8,
            Some("Unrelated"),
        ));
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("settings") && warning.contains("button")),
            "unbound unrelated buttons must not satisfy an unbound control group: {warnings:?}"
        );
    }

    #[test]
    fn preview_does_not_warn_for_data_source_only_control_buttons_metadata() {
        let mut project = Project::new("Control Buttons", 176, 166, ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.semantic_groups.push(SemanticGroup {
            id: "settings".into(),
            kind: SemanticGroupKind::ControlButtons,
            columns: None,
            visible_rows: None,
            total_rows: None,
            slot_count: None,
            member_ids: Vec::new(),
            data_source: Some("settings".into()),
            scroll_binding: None,
            dynamic_height: false,
        });
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(
            warnings
                .iter()
                .all(|warning| !warning.contains("settings") || !warning.contains("button")),
            "data-source-only control metadata should not require a button: {warnings:?}"
        );
    }

    #[test]
    fn semantic_warnings_report_missing_explicit_members() {
        let mut project = Project::new("Explicit Members", 176, 166, ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.semantic_groups.push(SemanticGroup {
            id: "controls".into(),
            kind: SemanticGroupKind::ControlButtons,
            columns: None,
            visible_rows: None,
            total_rows: None,
            slot_count: None,
            member_ids: vec!["missing_button".into()],
            data_source: None,
            scroll_binding: None,
            dynamic_height: false,
        });
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("references missing element 'missing_button'")),
            "missing explicit member should warn: {warnings:?}"
        );
    }

    #[test]
    fn semantic_warnings_report_non_button_control_members() {
        let mut project = Project::new("Explicit Members", 176, 166, ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.elements.push(semantic_slot_element(
            "slot_as_button",
            SlotRole::PlayerInventory,
            "controls",
            0,
        ));
        project.semantic_groups.push(SemanticGroup {
            id: "controls".into(),
            kind: SemanticGroupKind::ControlButtons,
            columns: None,
            visible_rows: None,
            total_rows: None,
            slot_count: None,
            member_ids: vec!["slot_as_button".into()],
            data_source: None,
            scroll_binding: None,
            dynamic_height: false,
        });
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(
            warnings.iter().any(|warning| {
                warning.contains("references non-button element 'slot_as_button'")
            }),
            "non-button explicit control member should warn: {warnings:?}"
        );
    }

    #[test]
    fn semantic_warnings_report_explicit_control_member_count_mismatch() {
        let mut project = Project::new("Explicit Members", 176, 166, ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.elements.push(button_element(
            "settings_button",
            ElementType::Button,
            8,
            8,
            Some("Settings"),
        ));
        project.semantic_groups.push(SemanticGroup {
            id: "controls".into(),
            kind: SemanticGroupKind::ControlButtons,
            columns: None,
            visible_rows: None,
            total_rows: None,
            slot_count: Some(2),
            member_ids: vec!["settings_button".into()],
            data_source: None,
            scroll_binding: None,
            dynamic_height: false,
        });
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(
            warnings.iter().any(|warning| {
                warning.contains("declares 2")
                    && warning.contains("only 1")
                    && warning.contains("matching")
            }),
            "explicit control member count mismatch should warn: {warnings:?}"
        );
    }

    #[test]
    fn semantic_warnings_report_duplicate_explicit_slot_members_and_count_distinct() {
        let mut project = Project::new("Explicit Members", 176, 166, ModTarget::Forge);
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.elements.push(semantic_slot_element(
            "slot_0",
            SlotRole::PlayerInventory,
            "player_inventory",
            0,
        ));
        project.semantic_groups.push(SemanticGroup {
            id: "player_inventory".into(),
            kind: SemanticGroupKind::PlayerInventory,
            columns: Some(9),
            visible_rows: Some(1),
            total_rows: Some(1),
            slot_count: Some(2),
            member_ids: vec!["slot_0".into(), "slot_0".into()],
            data_source: None,
            scroll_binding: None,
            dynamic_height: false,
        });
        let settings = project.export_settings.clone().normalized();

        let warnings = semantic_warnings(&project, &settings);

        assert!(
            warnings
                .iter()
                .any(|warning| { warning.contains("references duplicate member id 'slot_0'") }),
            "duplicate explicit member should warn: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|warning| {
                warning.contains("declares 2")
                    && warning.contains("only 1")
                    && warning.contains("matching")
            }),
            "duplicate explicit slot member should count once: {warnings:?}"
        );
    }

    #[test]
    fn modular_export_plans_semantic_registry() {
        let output_dir = TempExportDir::new("modular-semantic-registry");
        let mut project = Project::new("Scrollable", 176, 166, ModTarget::Forge);
        crate::templates::apply_template(&mut project, "scrollable_inventory_machine").unwrap();
        project.export_settings.codegen_mode = CodegenMode::Modular;
        project.export_settings.generate_semantic_registry = true;
        let config = ExportConfig {
            mod_id: "demo".into(),
            package: "com.example.demo".into(),
            class_name: "ScrollableGui".into(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&project, &config, "forge").unwrap();

        assert!(preview
            .files
            .iter()
            .any(|path| path.ends_with("GuiSemanticRegistry.java")));
    }

    #[test]
    fn screen_class_name_appends_screen_only_when_missing_across_targets() {
        for (target, project_target) in [
            ("forge", ModTarget::Forge),
            ("fabric", ModTarget::Fabric),
            ("neoforge", ModTarget::NeoForge),
        ] {
            for (class_name, expected_screen_class) in [
                ("AutoCutterGenerated", "AutoCutterGeneratedScreen"),
                ("AutoCutterGeneratedScreen", "AutoCutterGeneratedScreen"),
            ] {
                let output_dir = TempExportDir::new(&format!("{target}-{class_name}"));
                let config = ExportConfig {
                    mod_id: "testmod".to_string(),
                    package: "com.example".to_string(),
                    class_name: class_name.to_string(),
                    output_dir: output_dir.path().to_string_lossy().to_string(),
                    settings_override: None,
                    overwrite: false,
                };

                let plan =
                    plan_export(&sample_project(project_target.clone()), &config, target).unwrap();
                let planned_output = plan
                    .files
                    .iter()
                    .map(|file| {
                        format!(
                            "{}\n{}",
                            file.path.to_string_lossy(),
                            String::from_utf8_lossy(&file.data)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                assert!(planned_output.contains(&format!("{expected_screen_class}.java")));
                assert!(planned_output.contains(&format!("class {expected_screen_class}")));
                assert!(planned_output.contains(&format!("Register {expected_screen_class}")));
                assert!(!planned_output.contains("ScreenScreen"));
            }
        }
    }

    #[test]
    fn forge_export_writes_buildable_project_with_dimensions_assets_and_metadata() {
        let (dir, files) = export_sample("forge", ModTarget::Forge);
        let java_dir = dir.join("src/main/java/com/example/_1bad/class_");
        let asset_dir = dir.join("src/main/resources/assets/bad_mod_id");

        assert!(files.iter().any(|path| path.ends_with("settings.gradle")));
        assert!(dir.join("build.gradle").exists());
        assert!(asset_dir
            .join("textures/gui/123_furnace_gui_gui.png")
            .exists());
        assert!(asset_dir.join("textures/widgets/panel.png").exists());
        assert!(asset_dir.join("textures/widgets/progress.png").exists());
        assert!(asset_dir.join("gui/123_furnace_gui_layout.json").exists());
        assert!(dir.join("src/main/resources/META-INF/mods.toml").exists());

        let screen = read(&java_dir.join("G123FurnaceGUIScreen.java"));
        assert!(screen.contains("this.imageWidth = 187;"));
        assert!(screen.contains("this.imageHeight = 173;"));
        assert!(screen.contains(
            "layout.renderProgress(\"cook_progress\", graphics, leftPos, topPos, 0.0F);"
        ));

        let layout = read(&java_dir.join("GuiLayout.java"));
        assert!(layout.contains("new ResourceLocation(namespace, path)"));
        assert!(!layout.contains("net.minecraft.client.gui.DrawContext"));
        assert!(!layout.contains("switch (animation.directionOrDefault()) {{"));
        assert!(layout.contains(
            "new GuiLayout(data.elements, data.animations, bgId, visualOffsetX, visualOffsetY, backgroundWidth, backgroundHeight);"
        ));
        assert!(!layout.contains("overlayId"));

        let layout_json = read(&asset_dir.join("gui/123_furnace_gui_layout.json"));
        assert!(layout_json.contains("\"width\": 187"));
        assert!(layout_json.contains("\"textures/widgets/panel.png\""));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn generated_runtime_draws_background_at_visual_offset_but_keeps_main_size() {
        let output_dir = TempExportDir::new("visual-offset-runtime");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "VisualOffsetGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let mut project = Project::new("Visual Offset", 100, 80, ModTarget::Forge);
        project.texture_data.insert(
            "textures/widgets/panel.png".to_string(),
            png_bytes_with_size(32, 32, [0xd7, 0xa3, 0x39, 0xff]),
        );
        project
            .assets
            .push("textures/widgets/panel.png".to_string());
        project.elements.push(Element {
            id: "flair".to_string(),
            element_type: ElementType::Texture,
            x: 84,
            y: -16,
            width: Some(32),
            height: Some(32),
            size: None,
            asset: Some("textures/widgets/panel.png".to_string()),
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
        });

        export_project(&project, &config, "forge").unwrap();
        let java_dir = output_dir.path().join("src/main/java/com/example");
        let screen = read(&java_dir.join("VisualOffsetGuiScreen.java"));
        let layout = read(&java_dir.join("GuiLayout.java"));

        assert!(screen.contains("this.imageWidth = 100;"));
        assert!(screen.contains("this.imageHeight = 80;"));
        assert!(layout.contains("left + visualOffsetX"));
        assert!(layout.contains("top + visualOffsetY"));
        assert!(layout.contains("backgroundWidth"));
        assert!(layout.contains("backgroundHeight"));
    }

    #[test]
    fn fabric_layered_export_defines_overlay_method_and_loads_overlay_texture() {
        let output_dir = TempExportDir::new("fabric-layered");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "LayeredGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let files = export_project(&layered_project(ModTarget::Fabric), &config, "fabric").unwrap();
        let layout_path = output_dir
            .path()
            .join("src/main/java/com/example/GuiLayout.java");
        let screen_path = output_dir
            .path()
            .join("src/main/java/com/example/LayeredGuiScreen.java");
        let layout = read(&layout_path);
        let screen = read(&screen_path);

        assert!(files
            .iter()
            .any(|path| path.ends_with("textures/gui/layeredgui_overlay.png")));
        assert!(layout.contains("private final Identifier overlay;"));
        assert!(
            layout.contains("public void renderOverlay(DrawContext context, int left, int top)")
        );
        assert!(layout.contains("data.textures.overlay"));
        assert!(!layout.contains("switch (animation.directionOrDefault()) {{"));
        assert!(screen.contains("layout.renderOverlay(context, x, y);"));
    }

    #[test]
    fn animatable_layer_export_uses_generated_sprite_textures_in_runtime() {
        let output_dir = TempExportDir::new("animatable-runtime");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "LayeredGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&layered_project(ModTarget::Forge), &config, "forge").unwrap();
        assert!(preview
            .files
            .iter()
            .any(|path| path.ends_with("textures/gui/progress_arrow.png")));

        export_project(&layered_project(ModTarget::Forge), &config, "forge").unwrap();
        let layout = read(
            &output_dir
                .path()
                .join("src/main/java/com/example/GuiLayout.java"),
        );

        assert!(layout
            .contains("ResourceLocation spriteTexture = resource(namespace, element.texture);"));
        assert!(layout.contains("if (element.texture != null)"));
        assert!(layout.contains("graphics.blit(spriteTexture"));
        assert!(layout
            .contains("width - Math.round(width * ratio), 0, Math.round(width * ratio), height"));
        assert!(layout.contains(
            "String direction = element.directionOrDefault(animation.directionOrDefault());"
        ));
        assert!(layout.contains("String direction;"));
        assert!(!layout.contains("switch (animation.directionOrDefault()) {{"));
    }

    #[test]
    fn animatable_sprite_export_uses_source_texture_pixels() {
        let output_dir = TempExportDir::new("animatable-sprite-texture");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "LayeredGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        export_project(&layered_project(ModTarget::Forge), &config, "forge").unwrap();
        let sprite_path = output_dir
            .path()
            .join("src/main/resources/assets/testmod/textures/gui/progress_arrow.png");
        let sprite = image::open(sprite_path).unwrap().to_rgba8();

        assert_eq!(sprite.dimensions(), (24, 16));
        assert_eq!(sprite.get_pixel(0, 0).0, [240, 180, 40, 255]);
    }

    #[test]
    fn hidden_animatable_export_does_not_reference_missing_generated_sprite_texture() {
        let output_dir = TempExportDir::new("hidden-animatable-sprite");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "LayeredGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let mut project = layered_project(ModTarget::Forge);
        let progress = project
            .elements
            .iter_mut()
            .find(|element| element.id == "progress_arrow")
            .unwrap();
        progress.visible = false;

        export_project(&project, &config, "forge").unwrap();
        let layout_json = read(
            &output_dir
                .path()
                .join("src/main/resources/assets/testmod/gui/layeredgui_layout.json"),
        );
        let sprite_path = output_dir
            .path()
            .join("src/main/resources/assets/testmod/textures/gui/progress_arrow.png");

        assert!(!sprite_path.exists());
        assert!(!layout_json.contains(r#""texture": "textures/gui/progress_arrow.png""#));
    }

    #[test]
    fn animatable_sprite_export_crops_progress_uv_region() {
        let output_dir = TempExportDir::new("animatable-progress-uv");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "LayeredGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let mut project = layered_project(ModTarget::Forge);
        let mut atlas = RgbaImage::from_pixel(16, 8, Rgba([0x10, 0x20, 0x30, 0xff]));
        for x in 8..16 {
            for y in 0..8 {
                atlas.put_pixel(x, y, Rgba([0xf0, 0xb4, 0x28, 0xff]));
            }
        }
        let mut bytes = Vec::new();
        atlas
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        project
            .texture_data
            .insert("textures/widgets/progress.png".into(), bytes);
        let progress = project
            .elements
            .iter_mut()
            .find(|element| element.id == "progress_arrow")
            .unwrap();
        progress.asset = Some("textures/widgets/progress.png".into());
        progress.uv = Some(UvRect {
            x: 8,
            y: 0,
            width: 8,
            height: 8,
        });

        export_project(&project, &config, "forge").unwrap();
        let sprite_path = output_dir
            .path()
            .join("src/main/resources/assets/testmod/textures/gui/progress_arrow.png");
        let sprite = image::open(sprite_path).unwrap().to_rgba8();

        assert_eq!(sprite.get_pixel(0, 0).0, [0xf0, 0xb4, 0x28, 0xff]);
    }

    #[test]
    fn background_export_uses_visual_bounds_when_empty_layer_is_visually_expanded() {
        let output_dir = TempExportDir::new("empty-background-visual-bounds");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "ReturnsGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let mut project = Project::new("Returns", 100, 80, ModTarget::Forge);
        project.attached_regions.push(AttachedRegion {
            id: "returns_pocket".to_string(),
            anchor: AttachedRegionAnchor::Right,
            x: 100,
            y: 18,
            width: 54,
            height: 72,
            state: AttachedRegionState::Static,
            kind: Some("returns_pocket".to_string()),
            semantic_group: Some("food_returns".to_string()),
            visible: true,
        });

        export_project(&project, &config, "forge").unwrap();
        let background_path = output_dir
            .path()
            .join("src/main/resources/assets/testmod/textures/gui/returnsgui_gui.png");
        let background = image::open(background_path).unwrap().to_rgba8();
        let screen = read(
            &output_dir
                .path()
                .join("src/main/java/com/example/ReturnsGuiScreen.java"),
        );

        assert_eq!(background.dimensions(), (154, 90));
        assert!(screen.contains("this.imageWidth = 100;"));
        assert!(screen.contains("this.imageHeight = 80;"));
    }

    #[test]
    fn background_export_bakes_slot_texture_pixels() {
        let output_dir = TempExportDir::new("slot-baked-background");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "SlotBakedGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        export_project(&sample_project(ModTarget::Forge), &config, "forge").unwrap();
        let background_path = output_dir
            .path()
            .join("src/main/resources/assets/testmod/textures/gui/slotbakedgui_gui.png");
        let background = image::open(background_path).unwrap().to_rgba8();
        let layout = read(
            &output_dir
                .path()
                .join("src/main/java/com/example/GuiLayout.java"),
        );

        assert_eq!(background.get_pixel(8, 18).0, [0x37, 0x37, 0x37, 0xff]);
        assert!(!layout.contains("renderSlot("));
    }

    #[test]
    fn background_export_bakes_button_texture_pixels() {
        let output_dir = TempExportDir::new("button-baked-background");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "ButtonBakedGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let mut project = sample_project(ModTarget::Forge);
        project.elements.push(button_element(
            "start_button",
            ElementType::Button,
            40,
            60,
            Some("Start"),
        ));
        project.elements.push(button_element(
            "mode_toggle",
            ElementType::ToggleButton,
            84,
            60,
            Some("Mode"),
        ));

        export_project(&project, &config, "forge").unwrap();
        let background_path = output_dir
            .path()
            .join("src/main/resources/assets/testmod/textures/gui/buttonbakedgui_gui.png");
        let background = image::open(background_path).unwrap().to_rgba8();

        assert_eq!(background.get_pixel(40, 60).0, [0x37, 0x37, 0x37, 0xff]);
        assert_eq!(background.get_pixel(84, 60).0, [0x37, 0x37, 0x37, 0xff]);
    }

    #[test]
    fn forge_like_export_renders_centered_button_and_toggle_labels() {
        for (target, project_target, resource_factory) in [
            (
                "forge",
                ModTarget::Forge,
                "new ResourceLocation(namespace, path)",
            ),
            (
                "neoforge",
                ModTarget::NeoForge,
                "ResourceLocation.fromNamespaceAndPath(namespace, path)",
            ),
        ] {
            let output_dir = TempExportDir::new(&format!("{target}-button-labels"));
            let config = ExportConfig {
                mod_id: "testmod".to_string(),
                package: "com.example".to_string(),
                class_name: "ButtonLabelsGui".to_string(),
                output_dir: output_dir.path().to_string_lossy().to_string(),
                settings_override: None,
                overwrite: false,
            };
            let mut project = sample_project(project_target);
            project.elements.push(button_element(
                "start_button",
                ElementType::Button,
                40,
                60,
                Some("Start"),
            ));
            project.elements.push(button_element(
                "mode_toggle",
                ElementType::ToggleButton,
                84,
                60,
                Some("Mode"),
            ));
            project.elements.push(button_element(
                "empty_button",
                ElementType::Button,
                128,
                60,
                Some(""),
            ));

            export_project(&project, &config, target).unwrap();
            let layout = read(
                &output_dir
                    .path()
                    .join("src/main/java/com/example/GuiLayout.java"),
            );
            let layout_json = read(
                &output_dir
                    .path()
                    .join("src/main/resources/assets/testmod/gui/buttonlabelsgui_layout.json"),
            );

            assert!(layout.contains(resource_factory));
            assert!(layout.contains(
                "case \"button\", \"toggle_button\" -> renderButtonLabel(graphics, font, element, x, y);"
            ));
            assert!(layout.contains("if (label.isEmpty() || element.hasIcon()) {"));
            assert!(layout.contains("int labelX = x + (width - font.width(label)) / 2;"));
            assert!(layout.contains("int labelY = y + (height - 8) / 2;"));
            assert!(layout.contains(
                "graphics.drawString(font, label, labelX, labelY, element.colorOrDefault(), element.shadowOrDefault());"
            ));
            assert!(layout_json.contains(r#""content": "Start""#));
            assert!(layout_json.contains(r#""content": "Mode""#));
        }
    }

    #[test]
    fn fabric_export_renders_centered_button_and_toggle_labels() {
        let output_dir = TempExportDir::new("fabric-button-labels");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "ButtonLabelsGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let mut project = sample_project(ModTarget::Fabric);
        project.elements.push(button_element(
            "start_button",
            ElementType::Button,
            40,
            60,
            Some("Start"),
        ));
        project.elements.push(button_element(
            "mode_toggle",
            ElementType::ToggleButton,
            84,
            60,
            Some("Mode"),
        ));

        export_project(&project, &config, "fabric").unwrap();
        let layout = read(
            &output_dir
                .path()
                .join("src/main/java/com/example/GuiLayout.java"),
        );
        let layout_json = read(
            &output_dir
                .path()
                .join("src/main/resources/assets/testmod/gui/buttonlabelsgui_layout.json"),
        );

        assert!(layout.contains(
            "case \"button\", \"toggle_button\" -> renderButtonLabel(context, textRenderer, element, x, y);"
        ));
        assert!(layout.contains("if (label.isEmpty() || element.hasIcon()) {"));
        assert!(layout.contains("int labelX = x + (width - textRenderer.getWidth(label)) / 2;"));
        assert!(layout.contains("int labelY = y + (height - 8) / 2;"));
        assert!(layout.contains(
            "context.drawText(textRenderer, label, labelX, labelY, element.colorOrDefault(), element.shadowOrDefault());"
        ));
        assert!(layout_json.contains(r#""content": "Start""#));
        assert!(layout_json.contains(r#""content": "Mode""#));
    }

    #[test]
    fn forge_like_export_skips_runtime_labels_for_icon_buttons() {
        for (target, project_target) in [
            ("forge", ModTarget::Forge),
            ("neoforge", ModTarget::NeoForge),
        ] {
            let output_dir = TempExportDir::new(&format!("{target}-icon-button-labels"));
            let config = ExportConfig {
                mod_id: "testmod".to_string(),
                package: "com.example".to_string(),
                class_name: "IconButtonLabelsGui".to_string(),
                output_dir: output_dir.path().to_string_lossy().to_string(),
                settings_override: None,
                overwrite: false,
            };
            let mut project = sample_project(project_target);
            project.texture_data.insert(
                "textures/gui/settings_icon.png".to_string(),
                png_bytes([80, 120, 220, 255]),
            );
            let mut icon_button = button_element(
                "settings_button",
                ElementType::Button,
                40,
                60,
                Some("Button"),
            );
            icon_button.icon = Some("textures/gui/settings_icon.png".to_string());
            project.elements.push(icon_button);
            project.elements.push(button_element(
                "start_button",
                ElementType::Button,
                84,
                60,
                Some("Start"),
            ));

            export_project(&project, &config, target).unwrap();
            let layout = read(
                &output_dir
                    .path()
                    .join("src/main/java/com/example/GuiLayout.java"),
            );
            let layout_json = read(
                &output_dir
                    .path()
                    .join("src/main/resources/assets/testmod/gui/iconbuttonlabelsgui_layout.json"),
            );

            assert!(layout.contains("if (label.isEmpty() || element.hasIcon()) {"));
            assert!(
                layout.contains("boolean hasIcon() { return icon != null && !icon.isEmpty(); }")
            );
            assert!(layout.contains(
                "graphics.drawString(font, label, labelX, labelY, element.colorOrDefault(), element.shadowOrDefault());"
            ));
            assert!(layout_json.contains(r#""id": "settings_button""#));
            assert!(layout_json.contains(r#""content": "Button""#));
            assert!(layout_json.contains(r#""icon": "textures/gui/settings_icon.png""#));
            assert!(layout_json.contains(r#""id": "start_button""#));
            assert!(layout_json.contains(r#""content": "Start""#));
        }
    }

    #[test]
    fn fabric_export_skips_runtime_labels_for_icon_buttons() {
        let output_dir = TempExportDir::new("fabric-icon-button-labels");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "IconButtonLabelsGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let mut project = sample_project(ModTarget::Fabric);
        project.texture_data.insert(
            "textures/gui/settings_icon.png".to_string(),
            png_bytes([80, 120, 220, 255]),
        );
        let mut icon_toggle = button_element(
            "settings_toggle",
            ElementType::ToggleButton,
            40,
            60,
            Some("Button"),
        );
        icon_toggle.icon = Some("textures/gui/settings_icon.png".to_string());
        project.elements.push(icon_toggle);
        project.elements.push(button_element(
            "start_button",
            ElementType::Button,
            84,
            60,
            Some("Start"),
        ));

        export_project(&project, &config, "fabric").unwrap();
        let layout = read(
            &output_dir
                .path()
                .join("src/main/java/com/example/GuiLayout.java"),
        );
        let layout_json = read(
            &output_dir
                .path()
                .join("src/main/resources/assets/testmod/gui/iconbuttonlabelsgui_layout.json"),
        );

        assert!(layout.contains("if (label.isEmpty() || element.hasIcon()) {"));
        assert!(layout.contains("boolean hasIcon() { return icon != null && !icon.isEmpty(); }"));
        assert!(layout.contains(
            "context.drawText(textRenderer, label, labelX, labelY, element.colorOrDefault(), element.shadowOrDefault());"
        ));
        assert!(layout_json.contains(r#""id": "settings_toggle""#));
        assert!(layout_json.contains(r#""content": "Button""#));
        assert!(layout_json.contains(r#""icon": "textures/gui/settings_icon.png""#));
        assert!(layout_json.contains(r#""id": "start_button""#));
        assert!(layout_json.contains(r#""content": "Start""#));
    }

    #[test]
    fn mixed_progress_export_keeps_sprite_and_fill_runtime_paths() {
        let output_dir = TempExportDir::new("mixed-progress-runtime");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "LayeredGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };
        let mut project = layered_project(ModTarget::Forge);
        project.elements.push(Element {
            id: "burn_time".to_string(),
            element_type: ElementType::Progress,
            x: 52,
            y: 38,
            width: Some(14),
            height: Some(14),
            size: None,
            asset: None,
            icon: None,
            icon_uv: None,
            tooltip: None,
            direction: Some(FillDirection::LeftToRight),
            content: None,
            font: None,
            color: None,
            shadow: None,
            animation: Some("burn_time".to_string()),
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
        });
        project.animations.push(Animation {
            id: "burn_time".to_string(),
            animation_type: AnimationType::Fill,
            data_key: "burn_time".to_string(),
            texture: None,
            direction: Some(FillDirection::LeftToRight),
            frame_count: None,
            fps: None,
            min_value: Some(0.0),
            max_value: Some(100.0),
            triggers_on: None,
        });

        export_project(&project, &config, "forge").unwrap();
        let layout = read(
            &output_dir
                .path()
                .join("src/main/java/com/example/GuiLayout.java"),
        );

        assert!(layout.contains("if (element.texture != null)"));
        assert!(layout.contains("graphics.blit(spriteTexture"));
        assert!(layout.contains(
            "graphics.fill(x, y, x + Math.round(width * ratio), y + height, 0xFFE9A23B);"
        ));
        assert!(!layout.contains("findElementByAnimation"));
    }

    #[test]
    fn fabric_export_uses_fabric_classes_and_background_dimensions() {
        let (dir, _files) = export_sample("fabric", ModTarget::Fabric);
        let java_dir = dir.join("src/main/java/com/example/_1bad/class_");

        let screen = read(&java_dir.join("G123FurnaceGUIScreen.java"));
        assert!(screen.contains("extends HandledScreen<ScreenHandler>"));
        assert!(screen.contains("this.backgroundWidth = 187;"));
        assert!(screen.contains("this.backgroundHeight = 173;"));
        assert!(screen.contains("layout.renderProgress(\"cook_progress\", context, x, y, 0.0F);"));
        assert!(!screen.contains("AbstractContainerScreen"));

        let layout = read(&java_dir.join("GuiLayout.java"));
        assert!(layout.contains("net.minecraft.client.gui.DrawContext"));
        assert!(layout.contains("Identifier.of(namespace, path)"));
        assert!(layout
            .contains("width - Math.round(width * ratio), 0, Math.round(width * ratio), height"));
        assert!(layout.contains(
            "String direction = element.directionOrDefault(animation.directionOrDefault());"
        ));
        assert!(layout.contains("String direction;"));
        assert!(!layout.contains("net.minecraft.resources.ResourceLocation"));
        assert!(!layout.contains("switch (animation.directionOrDefault()) {{"));
        assert!(dir.join("src/main/resources/fabric.mod.json").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn neoforge_export_uses_neoforge_metadata_and_resource_location_factory() {
        let (dir, _files) = export_sample("neoforge", ModTarget::NeoForge);
        let java_dir = dir.join("src/main/java/com/example/_1bad/class_");

        let screen = read(&java_dir.join("G123FurnaceGUIScreen.java"));
        assert!(screen.contains("this.imageWidth = 187;"));
        assert!(screen.contains("this.imageHeight = 173;"));

        let layout = read(&java_dir.join("GuiLayout.java"));
        assert!(layout.contains("ResourceLocation.fromNamespaceAndPath(namespace, path)"));
        assert!(dir
            .join("src/main/resources/META-INF/neoforge.mods.toml")
            .exists());
        assert!(read(&dir.join("build.gradle")).contains("net.neoforged.gradle.userdev"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn export_rejects_missing_referenced_texture() {
        let output_dir = temp_export_dir("missing");
        let mut project = sample_project(ModTarget::Forge);
        project.texture_data.remove("textures/widgets/panel.png");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "TestGui".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let error = export_project(&project, &config, "forge").unwrap_err();

        assert!(error.contains("textures/widgets/panel.png"));
        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn export_rejects_missing_button_icon_texture() {
        let output_dir = temp_export_dir("missing-icon");
        let mut project = sample_project(ModTarget::Forge);
        project.elements.push(Element {
            icon: Some("textures/icons/start.png".to_string()),
            ..button_element("start_button", ElementType::Button, 12, 42, Some("Start"))
        });
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "TestGui".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let error = export_project(&project, &config, "forge").unwrap_err();

        assert!(error.contains("textures/icons/start.png"));
        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn preview_includes_button_icon_texture_in_planned_files() {
        let output_dir = temp_export_dir("preview-icon");
        let icon_asset = "textures/icons/start.png".to_string();
        let mut project = sample_project(ModTarget::Forge);
        project
            .texture_data
            .insert(icon_asset.clone(), png_bytes([20, 200, 80, 255]));
        project.elements.push(Element {
            icon: Some(icon_asset),
            ..button_element("start_button", ElementType::Button, 12, 42, Some("Start"))
        });
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "TestGui".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&project, &config, "forge").unwrap();

        assert!(preview.files.iter().any(|path| {
            path.ends_with("src/main/resources/assets/testmod/textures/icons/start.png")
        }));
        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn export_ignores_missing_texture_references_on_hidden_elements() {
        let output_dir = TempExportDir::new("hidden-missing-texture");
        let mut project = sample_project(ModTarget::Forge);
        project.elements.push(Element {
            asset: Some("textures/missing/hidden.png".to_string()),
            visible: false,
            ..button_element("hidden_missing_texture", ElementType::Texture, 12, 42, None)
        });
        project.elements.push(Element {
            icon: Some("textures/missing/hidden_icon.png".to_string()),
            visible: false,
            ..button_element(
                "hidden_missing_icon",
                ElementType::Button,
                36,
                42,
                Some("Hidden"),
            )
        });
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "HiddenMissingGui".to_string(),
            output_dir: output_dir.path().to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        export_project(&project, &config, "forge").unwrap();

        let _ = fs::remove_dir_all(output_dir.path());
    }

    #[test]
    fn preview_uses_sanitized_names_and_lists_planned_java_and_resource_files() {
        let output_dir = temp_export_dir("preview");
        let config = ExportConfig {
            mod_id: "Bad Mod.ID".to_string(),
            package: "com.example.1bad.class".to_string(),
            class_name: "123 Furnace GUI".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&sample_project(ModTarget::Forge), &config, "forge").unwrap();

        assert_eq!(preview.target, "forge");
        assert_eq!(preview.mod_id, "bad_mod_id");
        assert_eq!(preview.package, "com.example._1bad.class_");
        assert_eq!(preview.class_name, "G123FurnaceGUI");
        assert_eq!(preview.output_dir, output_dir.to_string_lossy());
        assert!(preview.errors.is_empty());
        assert!(preview.files.iter().any(|path| {
            path.ends_with("src/main/java/com/example/_1bad/class_/GuiLayout.java")
        }));
        assert!(preview.files.iter().any(|path| {
            path.ends_with("src/main/java/com/example/_1bad/class_/G123FurnaceGUIScreen.java")
        }));
        assert!(preview.files.iter().any(|path| {
            path.ends_with("src/main/resources/assets/bad_mod_id/gui/123_furnace_gui_layout.json")
        }));
        assert!(preview.files.iter().any(|path| {
            path.ends_with(
                "src/main/resources/assets/bad_mod_id/textures/gui/123_furnace_gui_gui.png",
            )
        }));
        assert!(!output_dir.join("settings.gradle").exists());
        assert!(!output_dir.join("src").exists());

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn preview_reports_missing_textures_as_errors_without_writing_files() {
        let output_dir = temp_export_dir("preview-missing");
        let mut project = sample_project(ModTarget::Forge);
        project.texture_data.remove("textures/widgets/panel.png");
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "TestGui".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&project, &config, "forge").unwrap();

        assert!(preview
            .errors
            .iter()
            .any(|error| error.contains("textures/widgets/panel.png")));
        assert!(!output_dir.join("settings.gradle").exists());
        assert!(!output_dir.join("src").exists());

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn preview_reports_existing_target_files_as_warnings_without_overwriting() {
        let output_dir = temp_export_dir("preview-existing");
        let existing_path = output_dir.join("settings.gradle");
        fs::write(&existing_path, "existing settings").unwrap();
        let config = ExportConfig {
            mod_id: "testmod".to_string(),
            package: "com.example".to_string(),
            class_name: "TestGui".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&sample_project(ModTarget::Forge), &config, "forge").unwrap();

        assert!(preview.warnings.iter().any(|warning| {
            warning.contains("settings.gradle") && warning.contains("already exists")
        }));
        assert_eq!(read(&existing_path), "existing settings");
        assert!(!output_dir.join("src").exists());

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn preview_overwrite_suppresses_existing_target_warnings() {
        let output_dir = TempExportDir::new("preview-overwrite-existing");
        let project = Project::new("Overwrite", 176, 166, ModTarget::Forge);
        let config = ExportConfig {
            mod_id: "overwrite_test".into(),
            package: "net.inkyquill.overwrite".into(),
            class_name: "OverwriteScreen".into(),
            output_dir: output_dir.path().to_string_lossy().into_owned(),
            settings_override: None,
            overwrite: false,
        };
        let first = preview_export(&project, &config, "forge").unwrap();
        fs::create_dir_all(Path::new(&first.files[0]).parent().unwrap()).unwrap();
        fs::write(&first.files[0], "existing").unwrap();

        let warning_preview = preview_export(&project, &config, "forge").unwrap();
        assert!(warning_preview
            .warnings
            .iter()
            .any(|warning| warning.contains("already exists")));

        let overwrite_preview = preview_export(
            &project,
            &ExportConfig {
                overwrite: true,
                ..config
            },
            "forge",
        )
        .unwrap();
        assert!(!overwrite_preview
            .warnings
            .iter()
            .any(|warning| warning.contains("already exists")));
    }

    #[test]
    fn generated_java_files_have_no_trailing_whitespace() {
        let output_dir = TempExportDir::new("java-whitespace");
        let project = Project::new("Whitespace", 176, 166, ModTarget::Forge);
        let config = ExportConfig {
            mod_id: "whitespace_test".into(),
            package: "net.inkyquill.whitespace".into(),
            class_name: "WhitespaceScreen".into(),
            output_dir: output_dir.path().to_string_lossy().into_owned(),
            settings_override: None,
            overwrite: false,
        };
        let plan = plan_export(&project, &config, "forge").unwrap();

        for file in plan.files {
            if file
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("java")
            {
                continue;
            }
            let text = String::from_utf8(file.data).unwrap();
            for (index, line) in text.lines().enumerate() {
                assert_eq!(
                    line.trim_end(),
                    line,
                    "{}:{} has trailing whitespace",
                    file.path.display(),
                    index + 1
                );
            }
        }
    }

    #[test]
    fn preview_warns_when_progress_element_stretches_referenced_texture() {
        let output_dir = TempExportDir::new("progress-stretch-preview");
        let mut project = sample_project(ModTarget::Forge);
        let progress = project
            .elements
            .iter_mut()
            .find(|element| element.id == "progress_arrow")
            .unwrap();
        progress.asset = Some("textures/widgets/progress.png".into());
        progress.width = Some(40);
        progress.height = Some(20);
        project.export_settings.codegen_mode = CodegenMode::Simple;
        let config = ExportConfig {
            mod_id: "progress_stretch".into(),
            package: "net.inkyquill.progress".into(),
            class_name: "ProgressStretchScreen".into(),
            output_dir: output_dir.path().to_string_lossy().into_owned(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&project, &config, "forge").unwrap();

        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("progress_arrow") && warning.contains("stretched")));
    }

    #[test]
    fn preview_does_not_warn_when_progress_matches_uv_source_size() {
        let output_dir = TempExportDir::new("progress-uv-size-preview");
        let mut project = sample_project(ModTarget::Forge);
        project.texture_data.insert(
            "textures/widgets/progress.png".into(),
            png_bytes_with_size(32, 32, [240, 180, 40, 255]),
        );
        let progress = project
            .elements
            .iter_mut()
            .find(|element| element.id == "progress_arrow")
            .unwrap();
        progress.asset = Some("textures/widgets/progress.png".into());
        progress.width = Some(14);
        progress.height = Some(14);
        progress.uv = Some(UvRect {
            x: 4,
            y: 4,
            width: 14,
            height: 14,
        });
        project.export_settings.codegen_mode = CodegenMode::Simple;
        let config = ExportConfig {
            mod_id: "progress_uv_size".into(),
            package: "net.inkyquill.progress".into(),
            class_name: "ProgressUvSizeScreen".into(),
            output_dir: output_dir.path().to_string_lossy().into_owned(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&project, &config, "forge").unwrap();

        assert!(
            preview.warnings.iter().all(
                |warning| !warning.contains("progress_arrow") || !warning.contains("stretched")
            ),
            "progress matching UV source size should not warn: {:?}",
            preview.warnings
        );
    }

    #[test]
    fn preview_warns_when_progress_differs_from_uv_source_size() {
        let output_dir = TempExportDir::new("progress-uv-stretch-preview");
        let mut project = sample_project(ModTarget::Forge);
        project.texture_data.insert(
            "textures/widgets/progress.png".into(),
            png_bytes_with_size(32, 32, [240, 180, 40, 255]),
        );
        let progress = project
            .elements
            .iter_mut()
            .find(|element| element.id == "progress_arrow")
            .unwrap();
        progress.asset = Some("textures/widgets/progress.png".into());
        progress.width = Some(16);
        progress.height = Some(14);
        progress.uv = Some(UvRect {
            x: 4,
            y: 4,
            width: 14,
            height: 14,
        });
        project.export_settings.codegen_mode = CodegenMode::Simple;
        let config = ExportConfig {
            mod_id: "progress_uv_stretch".into(),
            package: "net.inkyquill.progress".into(),
            class_name: "ProgressUvStretchScreen".into(),
            output_dir: output_dir.path().to_string_lossy().into_owned(),
            settings_override: None,
            overwrite: false,
        };

        let preview = preview_export(&project, &config, "forge").unwrap();

        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("progress_arrow") && warning.contains("stretched")));
    }
}
