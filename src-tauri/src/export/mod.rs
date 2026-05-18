use crate::animation::Animation;
use crate::project::{ElementType, Layer, Project};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ExportConfig {
    pub mod_id: String,
    pub package: String,
    pub class_name: String,
    pub output_dir: String,
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
    let plan = plan_export(project, config, target)?;
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
        warnings: existing_file_warnings(&plan.files),
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
    validate_project_dimensions(project)?;

    let mut files = Vec::new();
    let errors = missing_texture_errors(project);

    let settings_path = export.output_dir.join("settings.gradle");
    plan_file(
        &mut files,
        settings_path,
        generate_settings_gradle(&export, target).into_bytes(),
    )?;

    let gradle_path = export.output_dir.join("build.gradle");
    plan_file(
        &mut files,
        gradle_path,
        generate_build_gradle(&export, target).into_bytes(),
    )?;

    let properties_path = export.output_dir.join("gradle.properties");
    plan_file(
        &mut files,
        properties_path,
        generate_gradle_properties(&export, target).into_bytes(),
    )?;

    // Background atlas
    let bg_atlas = crate::texture::composite_atlas_for_layer(project, Layer::Background)?;
    let bg_texture_path = export
        .asset_dir()
        .join(format!("textures/gui/{}_gui.png", export.resource_name));
    plan_file(&mut files, bg_texture_path, bg_atlas)?;

    // Overlay atlas (only if overlay elements exist)
    let has_overlay = project
        .elements
        .iter()
        .any(|e| e.layer == Layer::Overlay);
    if has_overlay {
        let overlay_atlas =
            crate::texture::composite_atlas_for_layer(project, Layer::Overlay)?;
        let overlay_texture_path = export
            .asset_dir()
            .join(format!("textures/gui/{}_overlay.png", export.resource_name));
        plan_file(&mut files, overlay_texture_path, overlay_atlas)?;
    }

    // Animatable sprites
    for element in &project.elements {
        if element.layer == Layer::Animatable {
            let sprite =
                crate::texture::composite_single_element(element, project)?;
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
        textures_json["overlay"] = serde_json::json!(format!(
            "textures/gui/{}_overlay.png",
            export.resource_name
        ));
    }

    let elements_json: Vec<serde_json::Value> = project
        .elements
        .iter()
        .map(|e| {
            let mut val = serde_json::to_value(e).unwrap();
            if e.layer == Layer::Animatable {
                val["texture"] =
                    serde_json::json!(format!("textures/gui/{}.png", e.id));
            }
            val
        })
        .collect();

    let layout = serde_json::json!({
        "gui_size": project.gui_size,
        "textures": textures_json,
        "elements": elements_json,
        "groups": project.groups,
        "animations": project.animations,
    });
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
        generate_gui_layout_java(&export, target, project).into_bytes(),
    )?;

    let screen_path = export
        .java_dir()
        .join(format!("{}Screen.java", export.class_name));
    let screen_code = match target {
        ExportTarget::Forge => generate_forge_screen(&export, project),
        ExportTarget::Fabric => generate_fabric_screen(&export, project),
        ExportTarget::NeoForge => generate_neoforge_screen(&export, project),
    };
    plan_file(&mut files, screen_path, screen_code.into_bytes())?;

    let mod_entry_path = export
        .java_dir()
        .join(format!("{}Client.java", export.class_name));
    plan_file(
        &mut files,
        mod_entry_path,
        generate_client_entrypoint(&export, target).into_bytes(),
    )?;

    let metadata_path = loader_metadata_path(&export, target);
    plan_file(
        &mut files,
        metadata_path,
        generate_loader_metadata(&export, target).into_bytes(),
    )?;

    let readme_path = export.output_dir.join("README.txt");
    plan_file(
        &mut files,
        readme_path,
        generate_readme(&export, target, project).into_bytes(),
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

fn referenced_texture_assets(project: &Project) -> Vec<Cow<'_, str>> {
    let mut assets = Vec::new();
    for element in &project.elements {
        if element.element_type == ElementType::Texture {
            if let Some(asset) = element.asset.as_deref() {
                if !assets.iter().any(|known: &Cow<'_, str>| known == asset) {
                    assets.push(Cow::Borrowed(asset));
                }
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

fn sanitize_package(value: &str, mod_id: &str) -> String {
    let segments: Vec<String> = value
        .split('.')
        .filter_map(|segment| sanitize_package_segment(segment))
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

fn generate_forge_like_layout_java(
    export: &SanitizedExport,
    target: ExportTarget,
    project: &Project,
) -> String {
    let resource_location_ctor = match target {
        ExportTarget::NeoForge => "ResourceLocation.fromNamespaceAndPath(namespace, path)",
        _ => "new ResourceLocation(namespace, path)",
    };

    let has_overlay = project.elements.iter().any(|e| e.layer == Layer::Overlay);
    let has_animatable = project
        .elements
        .iter()
        .any(|e| e.layer == Layer::Animatable);

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
    let overlay_render = if has_overlay {
        r#"
    public void renderOverlay(GuiGraphics graphics, int left, int top) {{
        if (overlay != null) {{
            graphics.blit(overlay, left, top, 0, 0, WIDTH, HEIGHT, WIDTH, HEIGHT);
        }}
    }}
"#
    } else {
        r#"
    public void renderOverlay(GuiGraphics graphics, int left, int top) {{
        // No overlay elements in this layout
    }}
"#
    };

    let progress_body = if has_animatable {
        r#"        Element element = findElementByAnimation(animationId);
        if (element == null || element.texture == null) {{
            return;
        }}
        ResourceLocation spriteTexture = resource(namespace, element.texture);
        int x = left + element.x;
        int y = top + element.y;
        int width = element.widthOrDefault(22);
        int height = element.heightOrDefault(15);
        float ratio = animation.normalize(value);
        switch (animation.directionOrDefault()) {{
            case "right_to_left" -> graphics.blit(spriteTexture, x + width - Math.round(width * ratio), y, 0, 0, Math.round(width * ratio), height, width, height);
            case "bottom_to_top" -> graphics.blit(spriteTexture, x, y + height - Math.round(height * ratio), 0, height - Math.round(height * ratio), width, Math.round(height * ratio), width, height);
            case "top_to_bottom" -> graphics.blit(spriteTexture, x, y, 0, 0, width, Math.round(height * ratio), width, height);
            default -> graphics.blit(spriteTexture, x, y, 0, 0, Math.round(width * ratio), height, width, height);
        }}"#
    } else {
        r#"        for (Element element : elements) {{
            if (!element.isVisible() || !animationId.equals(element.animation)) {{
                continue;
            }}
            int x = left + element.x;
            int y = top + element.y;
            int width = element.widthOrDefault(22);
            int height = element.heightOrDefault(15);
            float ratio = animation.normalize(value);
            switch (animation.directionOrDefault()) {{
                case "right_to_left" -> graphics.fill(x + width - Math.round(width * ratio), y, x + width, y + height, 0xFFE9A23B);
                case "bottom_to_top" -> graphics.fill(x, y + height - Math.round(height * ratio), x + width, y + height, 0xFF3B82E9);
                case "top_to_bottom" -> graphics.fill(x, y, x + width, y + Math.round(height * ratio), 0xFF3B82E9);
                default -> graphics.fill(x, y, x + Math.round(width * ratio), y + height, 0xFFE9A23B);
            }}
        }}"#
    };

    let find_element_method = if has_animatable {
        r#"    private Element findElementByAnimation(String animationId) {{
        for (Element element : elements) {{
            if (animationId.equals(element.animation)) {{
                return element;
            }}
        }}
        return null;
    }}

"#
    } else {
        ""
    };

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
    private String namespace;

    private GuiLayout(List<Element> elements, List<Animation> animations, ResourceLocation texture{overlay_ctor}) {{
        this.elements = elements == null ? List.of() : elements;
        this.animations = animations == null ? List.of() : animations;
        this.texture = texture;
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
            ResourceLocation overlayId = data.textures.overlay != null ? resource(namespace, data.textures.overlay) : null;
            GuiLayout layout = new GuiLayout(data.elements, data.animations, bgId, overlayId);
            layout.namespace = namespace;
            return layout;
        }} catch (Exception error) {{
            throw new IllegalStateException("Failed to load GUI layout " + layoutId, error);
        }}
    }}

    public void renderTexture(GuiGraphics graphics, int left, int top) {{
        graphics.blit(texture, left, top, 0, 0, WIDTH, HEIGHT, WIDTH, HEIGHT);
    }}
{overlay_render}
    public void renderStaticElements(GuiGraphics graphics, int left, int top) {{
        Font font = Minecraft.getInstance().font;
        for (Element element : elements) {{
            if (!element.isVisible() || "texture".equals(element.type) || "progress".equals(element.type)) {{
                continue;
            }}
            int x = left + element.x;
            int y = top + element.y;
            switch (element.type) {{
                case "slot" -> renderSlot(graphics, x, y, element.sizeOrDefault());
                case "text" -> graphics.drawString(font, element.contentOrEmpty(), x, y, element.colorOrDefault(), element.shadowOrDefault());
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
        for (Element element : elements) {{
            if (!element.isVisible() || !animationId.equals(element.animation)) {{
                continue;
            }}
            int x = left + element.x;
            int y = top + element.y;
            int width = element.widthOrDefault(22);
            int height = element.heightOrDefault(15);
            float ratio = animation.normalize(value);
            switch (animation.directionOrDefault()) {{
                case "right_to_left" -> graphics.fill(x + width - Math.round(width * ratio), y, x + width, y + height, 0xFFE9A23B);
                case "bottom_to_top" -> graphics.fill(x, y + height - Math.round(height * ratio), x + width, y + height, 0xFF3B82E9);
                case "top_to_bottom" -> graphics.fill(x, y, x + width, y + Math.round(height * ratio), 0xFF3B82E9);
                default -> graphics.fill(x, y, x + Math.round(width * ratio), y + height, 0xFFE9A23B);
            }}
        }}
    }}

    private Animation findAnimation(String id) {{
        for (Animation animation : animations) {{
            if (id.equals(animation.id)) {{
                return animation;
            }}
        }}
        return null;
    }}

{find_element_method}    private static void renderSlot(GuiGraphics graphics, int x, int y, int size) {{
        graphics.fill(x, y, x + size, y + size, 0xFF8B8B8B);
        graphics.fill(x + 1, y + 1, x + size - 1, y + size - 1, 0xFF373737);
        graphics.fill(x + 2, y + 2, x + size - 2, y + size - 2, 0xFFC6C6C6);
    }}

    private static void renderMeterShell(GuiGraphics graphics, int x, int y, int width, int height) {{
        graphics.fill(x, y, x + width, y + height, 0xAA000000);
        graphics.fill(x + 1, y + 1, x + width - 1, y + height - 1, 0x55333333);
    }}

    private static final class LayoutData {{
        TexturesData textures;
        List<Element> elements;
        List<Animation> animations;
    }}

    private static final class TexturesData {{
        String background;
        String overlay;
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

        boolean isVisible() {{ return visible == null || visible; }}
        int widthOrDefault(int fallback) {{ return width == null ? fallback : width; }}
        int heightOrDefault(int fallback) {{ return height == null ? fallback : height; }}
        int sizeOrDefault() {{ return size == null ? 18 : size; }}
        String contentOrEmpty() {{ return content == null ? "" : content; }}
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
        resource_location_ctor = resource_location_ctor
    )
}

fn generate_fabric_layout_java(export: &SanitizedExport, project: &Project) -> String {
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

    private GuiLayout(List<Element> elements, List<Animation> animations, Identifier texture) {{
        this.elements = elements == null ? List.of() : elements;
        this.animations = animations == null ? List.of() : animations;
        this.texture = texture;
    }}

    public static Identifier resource(String namespace, String path) {{
        return Identifier.of(namespace, path);
    }}

    public static GuiLayout load(String namespace, String layoutPath, String texturePath) {{
        Identifier layoutId = resource(namespace, layoutPath);
        Identifier textureId = resource(namespace, texturePath);
        String classpathResource = "assets/" + layoutId.getNamespace() + "/" + layoutId.getPath();
        try (InputStreamReader reader = new InputStreamReader(
                GuiLayout.class.getClassLoader().getResourceAsStream(classpathResource),
                StandardCharsets.UTF_8)) {{
            Gson gson = new Gson();
            LayoutData data = gson.fromJson(reader, new TypeToken<LayoutData>() {{}}.getType());
            return new GuiLayout(data.elements, data.animations, textureId);
        }} catch (Exception error) {{
            throw new IllegalStateException("Failed to load GUI layout " + layoutId, error);
        }}
    }}

    public void renderTexture(DrawContext context, int left, int top) {{
        context.drawTexture(texture, left, top, 0, 0, WIDTH, HEIGHT, WIDTH, HEIGHT);
    }}

    public void renderStaticElements(DrawContext context, int left, int top) {{
        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;
        for (Element element : elements) {{
            if (!element.isVisible() || "texture".equals(element.type) || "progress".equals(element.type)) {{
                continue;
            }}
            int x = left + element.x;
            int y = top + element.y;
            switch (element.type) {{
                case "slot" -> renderSlot(context, x, y, element.sizeOrDefault());
                case "text" -> context.drawText(textRenderer, element.contentOrEmpty(), x, y, element.colorOrDefault(), element.shadowOrDefault());
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
        for (Element element : elements) {{
            if (!element.isVisible() || !animationId.equals(element.animation)) {{
                continue;
            }}
            int x = left + element.x;
            int y = top + element.y;
            int width = element.widthOrDefault(22);
            int height = element.heightOrDefault(15);
            float ratio = animation.normalize(value);
            switch (animation.directionOrDefault()) {{
                case "right_to_left" -> context.fill(x + width - Math.round(width * ratio), y, x + width, y + height, 0xFFE9A23B);
                case "bottom_to_top" -> context.fill(x, y + height - Math.round(height * ratio), x + width, y + height, 0xFF3B82E9);
                case "top_to_bottom" -> context.fill(x, y, x + width, y + Math.round(height * ratio), 0xFF3B82E9);
                default -> context.fill(x, y, x + Math.round(width * ratio), y + height, 0xFFE9A23B);
            }}
        }}
    }}

    private Animation findAnimation(String id) {{
        for (Animation animation : animations) {{
            if (id.equals(animation.id)) {{
                return animation;
            }}
        }}
        return null;
    }}

    private static void renderSlot(DrawContext context, int x, int y, int size) {{
        context.fill(x, y, x + size, y + size, 0xFF8B8B8B);
        context.fill(x + 1, y + 1, x + size - 1, y + size - 1, 0xFF373737);
        context.fill(x + 2, y + 2, x + size - 2, y + size - 2, 0xFFC6C6C6);
    }}

    private static void renderMeterShell(DrawContext context, int x, int y, int width, int height) {{
        context.fill(x, y, x + width, y + height, 0xAA000000);
        context.fill(x + 1, y + 1, x + width - 1, y + height - 1, 0x55333333);
    }}

    private static final class LayoutData {{
        List<Element> elements;
        List<Animation> animations;
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

        boolean isVisible() {{ return visible == null || visible; }}
        int widthOrDefault(int fallback) {{ return width == null ? fallback : width; }}
        int heightOrDefault(int fallback) {{ return height == null ? fallback : height; }}
        int sizeOrDefault() {{ return size == null ? 18 : size; }}
        String contentOrEmpty() {{ return content == null ? "" : content; }}
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
        height = project.gui_size.height
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

public class {class_name}Screen extends AbstractContainerScreen<AbstractContainerMenu> {{
    private GuiLayout layout;

    public {class_name}Screen(AbstractContainerMenu menu, Inventory inventory, Component title) {{
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
        class_name = export.class_name,
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

public class {class_name}Screen extends AbstractContainerScreen<AbstractContainerMenu> {{
    private GuiLayout layout;

    public {class_name}Screen(AbstractContainerMenu menu, Inventory inventory, Component title) {{
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
        class_name = export.class_name,
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

public class {class_name}Screen extends HandledScreen<ScreenHandler> {{
    private GuiLayout layout;

    public {class_name}Screen(ScreenHandler handler, PlayerInventory inventory, Text title) {{
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
        class_name = export.class_name,
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
        // Register {class_name}Screen with your ScreenHandlerType here.
    }}
}}
"#,
            package = export.package,
            class_name = export.class_name
        ),
        ExportTarget::Forge => format!(
            r#"package {package};

import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.fml.common.Mod;

@Mod.EventBusSubscriber(modid = "{mod_id}", value = Dist.CLIENT, bus = Mod.EventBusSubscriber.Bus.MOD)
public final class {class_name}Client {{
    private {class_name}Client() {{
    }}

    // Register {class_name}Screen with MenuScreens.register(...) from your client setup event.
}}
"#,
            package = export.package,
            class_name = export.class_name,
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

    // Register {class_name}Screen with MenuScreens.register(...) from your client setup event.
}}
"#,
            package = export.package,
            class_name = export.class_name,
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
  src/main/java/{package_path}/{class_name}Screen.java
  src/main/java/{package_path}/{class_name}Client.java
  src/main/resources/assets/{mod_id}/textures/gui/{resource_name}_gui.png
  src/main/resources/assets/{mod_id}/gui/{resource_name}_layout.json

Texture elements are composited into the GUI PNG. Referenced source PNG assets are also copied under assets/{mod_id}/textures/... so they are available for later hand edits.

The generated screen and runtime are designed to compile against the listed loader metadata and Gradle dependencies. Menu or ScreenHandler registration remains app-specific, so wire {class_name}Screen into your own menu type and replace generated animation default values with menu data where needed.

Animation hooks:
{animation_summary}
"#,
        class_name = export.class_name,
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
    use crate::project::{Element, FillDirection, Layer, ModTarget, Size};
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
                    direction: None,
                    content: None,
                    font: None,
                    color: None,
                    shadow: None,
                    animation: None,
                    visible: true,
                    uv: None,
                    layer: Layer::Background,
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
                    direction: None,
                    content: None,
                    font: None,
                    color: None,
                    shadow: None,
                    animation: None,
                    visible: true,
                    uv: None,
                    layer: Layer::Background,
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
                    direction: Some(FillDirection::LeftToRight),
                    content: None,
                    font: None,
                    color: None,
                    shadow: None,
                    animation: Some("cook_progress".to_string()),
                    visible: true,
                    uv: None,
                    layer: Layer::Background,
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
            project_path: None,
            is_dirty: true,
            texture_data,
            fonts: Vec::new(),
        }
    }

    fn png_bytes(color: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(2, 2, Rgba(color));
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

    fn export_sample(target: &str, project_target: ModTarget) -> (PathBuf, Vec<String>) {
        let output_dir = temp_export_dir(target);
        let config = ExportConfig {
            mod_id: "Bad Mod.ID".to_string(),
            package: "com.example.1bad.class".to_string(),
            class_name: "123 Furnace GUI".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
        };
        let files = export_project(&sample_project(project_target), &config, target).unwrap();
        (output_dir, files)
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
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

        let layout_json = read(&asset_dir.join("gui/123_furnace_gui_layout.json"));
        assert!(layout_json.contains("\"width\": 187"));
        assert!(layout_json.contains("\"textures/widgets/panel.png\""));

        let _ = fs::remove_dir_all(dir);
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
        assert!(!layout.contains("net.minecraft.resources.ResourceLocation"));
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
        };

        let error = export_project(&project, &config, "forge").unwrap_err();

        assert!(error.contains("textures/widgets/panel.png"));
        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn preview_uses_sanitized_names_and_lists_planned_java_and_resource_files() {
        let output_dir = temp_export_dir("preview");
        let config = ExportConfig {
            mod_id: "Bad Mod.ID".to_string(),
            package: "com.example.1bad.class".to_string(),
            class_name: "123 Furnace GUI".to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
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
        };

        let preview = preview_export(&sample_project(ModTarget::Forge), &config, "forge").unwrap();

        assert!(preview.warnings.iter().any(|warning| {
            warning.contains("settings.gradle") && warning.contains("already exists")
        }));
        assert_eq!(read(&existing_path), "existing settings");
        assert!(!output_dir.join("src").exists());

        let _ = fs::remove_dir_all(output_dir);
    }
}
