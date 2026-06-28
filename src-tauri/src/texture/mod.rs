#[allow(dead_code)]
pub mod generator;
use crate::project::{
    Element, ElementType, Layer, NineSlice, NineSliceMode, Project, TextureRenderMode,
};
use crate::texture_pack;
use image::{GenericImageView, Rgba, RgbaImage};

const MAX_COMPOSITE_DIMENSION: u32 = 4096;
const MAX_COMPOSITE_PIXELS: u64 = 16_777_216;
const GENERATED_GUI_PANEL: &str = "textures/generated/gui_panel.png";

pub fn composite_atlas_for_layer(project: &Project, layer: Layer) -> Result<Vec<u8>, String> {
    let has_elements = project
        .elements
        .iter()
        .any(|el| el.visible && el.layer == layer && is_baked_atlas_element(el));

    if !has_elements {
        validate_composite_size("atlas", project.gui_size.width, project.gui_size.height)?;
        return encode_png(RgbaImage::new(
            project.gui_size.width,
            project.gui_size.height,
        ));
    }

    let bounds = project.visual_bounds();
    validate_composite_size("atlas", bounds.width, bounds.height)?;
    let mut img = RgbaImage::new(bounds.width, bounds.height);

    for el in &project.elements {
        if !el.visible || el.layer != layer || !is_baked_atlas_element(el) {
            continue;
        }

        overlay_baked_element(&mut img, project, el, bounds.x, bounds.y)?;
    }

    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {e}"))?;

    Ok(buf)
}

pub fn composite_atlas_for_layer_with_visual_empty(
    project: &Project,
    layer: Layer,
) -> Result<Vec<u8>, String> {
    let has_elements = project
        .elements
        .iter()
        .any(|el| el.visible && el.layer == layer && is_baked_atlas_element(el));

    if has_elements {
        return composite_atlas_for_layer(project, layer);
    }

    let bounds = project.visual_bounds();
    validate_composite_size("atlas", bounds.width, bounds.height)?;
    encode_png(RgbaImage::new(bounds.width, bounds.height))
}

pub fn composite_project_preview(project: &Project) -> Result<Vec<u8>, String> {
    let bounds = project.visual_bounds();
    validate_composite_size("preview", bounds.width, bounds.height)?;
    let mut preview = RgbaImage::new(bounds.width, bounds.height);

    for layer in [Layer::Background, Layer::Overlay, Layer::Animatable] {
        for element in project
            .elements
            .iter()
            .filter(|element| element.visible && element.layer == layer)
        {
            if is_baked_atlas_element(element) {
                overlay_baked_element(&mut preview, project, element, bounds.x, bounds.y)?;
            } else {
                let element_png = composite_single_element(element, project)?;
                overlay_png(
                    &mut preview,
                    &element_png,
                    i64::from(element.x) - i64::from(bounds.x),
                    i64::from(element.y) - i64::from(bounds.y),
                    &element.id,
                )?;
            }
        }
    }

    encode_png(preview)
}

fn validate_composite_size(label: &str, width: u32, height: u32) -> Result<(), String> {
    let pixels = u64::from(width) * u64::from(height);
    if width > MAX_COMPOSITE_DIMENSION
        || height > MAX_COMPOSITE_DIMENSION
        || pixels > MAX_COMPOSITE_PIXELS
    {
        return Err(format!(
            "Cannot render {label}: unrenderable {label} size {width}x{height}; maximum dimension is {MAX_COMPOSITE_DIMENSION} and maximum pixels is {MAX_COMPOSITE_PIXELS}"
        ));
    }

    Ok(())
}

fn is_baked_atlas_element(element: &Element) -> bool {
    matches!(
        element.element_type,
        ElementType::Texture
            | ElementType::Slot
            | ElementType::VirtualSlotCell
            | ElementType::Button
            | ElementType::ToggleButton
            | ElementType::Scrollbar
    )
}

fn overlay_baked_element(
    img: &mut RgbaImage,
    project: &Project,
    element: &Element,
    offset_x: i32,
    offset_y: i32,
) -> Result<(), String> {
    match element.element_type {
        ElementType::Texture => {
            if let Some(asset_name) = &element.asset {
                overlay_asset(img, project, element, asset_name, offset_x, offset_y)?;
            }
        }
        ElementType::Slot | ElementType::VirtualSlotCell => {
            overlay_slot(img, project, element, offset_x, offset_y)?;
        }
        ElementType::Button | ElementType::ToggleButton => {
            overlay_button(img, project, element, offset_x, offset_y)?;
        }
        ElementType::Scrollbar => {
            let size = element.render_size();
            let data = generated_scrollbar(size.width, size.height)?;
            overlay_texture_data(
                img,
                project,
                element,
                &data,
                "generated scrollbar",
                offset_x,
                offset_y,
            )?;
        }
        _ => {}
    }

    Ok(())
}

fn overlay_slot(
    img: &mut RgbaImage,
    project: &Project,
    element: &Element,
    offset_x: i32,
    offset_y: i32,
) -> Result<(), String> {
    if let Some(asset_name) = element.asset.as_deref().or_else(|| {
        project
            .texture_data
            .contains_key(texture_pack::MINECRAFT_SLOT)
            .then_some(texture_pack::MINECRAFT_SLOT)
            .or_else(|| {
                project
                    .texture_data
                    .contains_key("textures/generated/slot.png")
                    .then_some("textures/generated/slot.png")
            })
    }) {
        return overlay_asset(img, project, element, asset_name, offset_x, offset_y);
    }

    let data = generated_slot()?;
    overlay_texture_data(
        img,
        project,
        element,
        &data,
        "generated slot",
        offset_x,
        offset_y,
    )
}

fn overlay_button(
    img: &mut RgbaImage,
    project: &Project,
    element: &Element,
    offset_x: i32,
    offset_y: i32,
) -> Result<(), String> {
    if let Some(asset_name) = element.asset.as_deref().or_else(|| {
        project
            .texture_data
            .contains_key(texture_pack::MINECRAFT_BUTTON)
            .then_some(texture_pack::MINECRAFT_BUTTON)
            .or_else(|| {
                project
                    .texture_data
                    .contains_key("textures/generated/button.png")
                    .then_some("textures/generated/button.png")
            })
    }) {
        overlay_asset(img, project, element, asset_name, offset_x, offset_y)?;
    } else {
        let data = generated_button()?;
        overlay_texture_data(
            img,
            project,
            element,
            &data,
            "generated button",
            offset_x,
            offset_y,
        )?;
    }

    overlay_button_icon(img, project, element, offset_x, offset_y)
}

fn overlay_button_icon(
    img: &mut RgbaImage,
    project: &Project,
    element: &Element,
    offset_x: i32,
    offset_y: i32,
) -> Result<(), String> {
    let Some(icon_name) = element.icon.as_deref() else {
        return Ok(());
    };
    let Some(data) = project.texture_data.get(icon_name) else {
        return Ok(());
    };
    let texture = image::load_from_memory(data)
        .map_err(|error| format!("Failed to load button icon '{}': {error}", icon_name))?
        .to_rgba8();
    let source = cropped_source(&texture, element.icon_uv.as_ref());
    if source.width() == 0 || source.height() == 0 {
        return Ok(());
    }
    let element_size = element.render_size();
    let element_w = element_size.width;
    let element_h = element_size.height;
    if element_w == 0 || element_h == 0 {
        return Ok(());
    }
    let max_w = element_w.saturating_sub(4).max(1);
    let max_h = element_h.saturating_sub(4).max(1);
    let scale = 1.0_f64
        .min(max_w as f64 / source.width() as f64)
        .min(max_h as f64 / source.height() as f64);
    let target_w = ((source.width() as f64 * scale).floor() as u32).max(1);
    let target_h = ((source.height() as f64 * scale).floor() as u32).max(1);
    let resized = image::imageops::resize(
        &source,
        target_w,
        target_h,
        image::imageops::FilterType::Nearest,
    );
    let x = i64::from(element.x) - i64::from(offset_x)
        + i64::from(element_w.saturating_sub(target_w) / 2);
    let y = i64::from(element.y) - i64::from(offset_y)
        + i64::from(element_h.saturating_sub(target_h) / 2);
    image::imageops::overlay(img, &resized, x, y);
    Ok(())
}

fn overlay_asset(
    img: &mut RgbaImage,
    project: &Project,
    element: &Element,
    asset_name: &str,
    offset_x: i32,
    offset_y: i32,
) -> Result<(), String> {
    let Some(data) = project.texture_data.get(asset_name) else {
        if element.element_type == ElementType::Texture
            && element.render_mode != TextureRenderMode::NineSlice
            && asset_name == GENERATED_GUI_PANEL
        {
            let (width, height) = generated_panel_target_size(project, element);
            let data = generated_gui_panel(width, height)?;
            return overlay_texture_data(
                img, project, element, &data, asset_name, offset_x, offset_y,
            );
        }
        return Ok(());
    };

    if element.element_type == ElementType::Texture
        && element.render_mode != TextureRenderMode::NineSlice
        && asset_name == GENERATED_GUI_PANEL
        && generated_panel_asset_is_stale(data, project, element)
    {
        let (width, height) = generated_panel_target_size(project, element);
        let data = generated_gui_panel(width, height)?;
        return overlay_texture_data(img, project, element, &data, asset_name, offset_x, offset_y);
    }

    overlay_texture_data(img, project, element, data, asset_name, offset_x, offset_y)
}

fn generated_panel_target_size(project: &Project, element: &Element) -> (u32, u32) {
    (
        element
            .width
            .or(element.size)
            .unwrap_or(project.gui_size.width),
        element
            .height
            .or(element.size)
            .unwrap_or(project.gui_size.height),
    )
}

fn generated_panel_asset_is_stale(data: &[u8], project: &Project, element: &Element) -> bool {
    let (target_width, target_height) = generated_panel_target_size(project, element);
    image::load_from_memory(data)
        .map(|texture| texture.dimensions() != (target_width, target_height))
        .unwrap_or(true)
}

fn overlay_texture_data(
    img: &mut RgbaImage,
    project: &Project,
    element: &Element,
    data: &[u8],
    asset_name: &str,
    offset_x: i32,
    offset_y: i32,
) -> Result<(), String> {
    let tex = image::load_from_memory(data)
        .map_err(|e| format!("Failed to load texture '{}': {e}", asset_name))?
        .to_rgba8();

    let (tw, th) = texture_target_size(element, &tex);

    let source = cropped_source(&tex, element.uv.as_ref());
    if source.width() == 0 || source.height() == 0 {
        return Ok(());
    }

    let rendered = if element.element_type == ElementType::Texture
        && element.render_mode == TextureRenderMode::NineSlice
    {
        let guides = resolved_nine_slice(project, element).ok_or_else(|| {
            format!(
                "Texture element '{}' uses nine_slice without guides",
                element.id
            )
        })?;
        render_nine_slice(&source, tw, th, guides)?
    } else {
        image::imageops::resize(&source, tw, th, image::imageops::FilterType::Nearest)
    };
    image::imageops::overlay(
        img,
        &rendered,
        i64::from(element.x) - i64::from(offset_x),
        i64::from(element.y) - i64::from(offset_y),
    );
    Ok(())
}

fn resolved_nine_slice<'a>(project: &'a Project, element: &'a Element) -> Option<&'a NineSlice> {
    element.nine_slice.as_ref().or_else(|| {
        let asset = element.asset.as_ref()?;
        project.asset_metadata.get(asset)?.nine_slice.as_ref()
    })
}

fn validate_nine_slice_guides(
    guides: &NineSlice,
    source_width: u32,
    source_height: u32,
) -> Result<(), String> {
    let horizontal_guides = u64::from(guides.left) + u64::from(guides.right);
    let vertical_guides = u64::from(guides.top) + u64::from(guides.bottom);
    if horizontal_guides >= u64::from(source_width) || vertical_guides >= u64::from(source_height) {
        return Err("Nine-slice guides leave no center region".to_string());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SliceRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn render_nine_slice(
    source: &RgbaImage,
    target_width: u32,
    target_height: u32,
    guides: &NineSlice,
) -> Result<RgbaImage, String> {
    validate_nine_slice_guides(guides, source.width(), source.height())?;
    validate_nine_slice_guides(guides, target_width, target_height)?;

    let source_x = [0, guides.left, source.width() - guides.right];
    let source_y = [0, guides.top, source.height() - guides.bottom];
    let source_w = [
        guides.left,
        source.width() - guides.left - guides.right,
        guides.right,
    ];
    let source_h = [
        guides.top,
        source.height() - guides.top - guides.bottom,
        guides.bottom,
    ];
    let target_x = [0, guides.left, target_width - guides.right];
    let target_y = [0, guides.top, target_height - guides.bottom];
    let target_w = [
        guides.left,
        target_width - guides.left - guides.right,
        guides.right,
    ];
    let target_h = [
        guides.top,
        target_height - guides.top - guides.bottom,
        guides.bottom,
    ];

    let mut output = RgbaImage::new(target_width, target_height);
    for row in 0..3 {
        for column in 0..3 {
            let source_rect = SliceRect {
                x: source_x[column],
                y: source_y[row],
                width: source_w[column],
                height: source_h[row],
            };
            let target_rect = SliceRect {
                x: target_x[column],
                y: target_y[row],
                width: target_w[column],
                height: target_h[row],
            };
            if source_rect.width == 0
                || source_rect.height == 0
                || target_rect.width == 0
                || target_rect.height == 0
            {
                continue;
            }

            match (row, column) {
                (0 | 2, 0 | 2) => copy_patch(&mut output, source, source_rect, target_rect),
                (1, 1) => render_patch(
                    &mut output,
                    source,
                    source_rect,
                    target_rect,
                    guides.center_mode,
                ),
                _ => render_patch(
                    &mut output,
                    source,
                    source_rect,
                    target_rect,
                    guides.edge_mode,
                ),
            }
        }
    }

    Ok(output)
}

fn copy_patch(
    output: &mut RgbaImage,
    source: &RgbaImage,
    source_rect: SliceRect,
    target_rect: SliceRect,
) {
    for y in 0..source_rect.height.min(target_rect.height) {
        for x in 0..source_rect.width.min(target_rect.width) {
            let pixel = source.get_pixel(source_rect.x + x, source_rect.y + y);
            output.put_pixel(target_rect.x + x, target_rect.y + y, *pixel);
        }
    }
}

fn render_patch(
    output: &mut RgbaImage,
    source: &RgbaImage,
    source_rect: SliceRect,
    target_rect: SliceRect,
    mode: NineSliceMode,
) {
    match mode {
        NineSliceMode::Stretch => stretch_patch(output, source, source_rect, target_rect),
        NineSliceMode::Tile => tile_patch(output, source, source_rect, target_rect),
    }
}

fn stretch_patch(
    output: &mut RgbaImage,
    source: &RgbaImage,
    source_rect: SliceRect,
    target_rect: SliceRect,
) {
    let patch = source
        .view(
            source_rect.x,
            source_rect.y,
            source_rect.width,
            source_rect.height,
        )
        .to_image();
    let resized = image::imageops::resize(
        &patch,
        target_rect.width,
        target_rect.height,
        image::imageops::FilterType::Nearest,
    );
    image::imageops::overlay(
        output,
        &resized,
        i64::from(target_rect.x),
        i64::from(target_rect.y),
    );
}

fn tile_patch(
    output: &mut RgbaImage,
    source: &RgbaImage,
    source_rect: SliceRect,
    target_rect: SliceRect,
) {
    for y in 0..target_rect.height {
        for x in 0..target_rect.width {
            let source_x = source_rect.x + x % source_rect.width;
            let source_y = source_rect.y + y % source_rect.height;
            let pixel = source.get_pixel(source_x, source_y);
            output.put_pixel(target_rect.x + x, target_rect.y + y, *pixel);
        }
    }
}

fn texture_target_size(element: &Element, tex: &RgbaImage) -> (u32, u32) {
    if element.element_type == ElementType::Texture {
        (
            element.width.or(element.size).unwrap_or(tex.width()),
            element.height.or(element.size).unwrap_or(tex.height()),
        )
    } else {
        let size = element.render_size();
        (size.width, size.height)
    }
}

fn overlay_png(img: &mut RgbaImage, png: &[u8], x: i64, y: i64, label: &str) -> Result<(), String> {
    let overlay = image::load_from_memory(png)
        .map_err(|error| format!("Failed to load preview image '{}': {error}", label))?
        .to_rgba8();
    image::imageops::overlay(img, &overlay, x, y);
    Ok(())
}

fn cropped_source(tex: &RgbaImage, uv: Option<&crate::project::UvRect>) -> RgbaImage {
    if let Some(uv) = uv {
        let x = uv.x.min(tex.width());
        let y = uv.y.min(tex.height());
        let width = uv.width.min(tex.width().saturating_sub(x));
        let height = uv.height.min(tex.height().saturating_sub(y));
        if width == 0 || height == 0 {
            return RgbaImage::new(0, 0);
        }
        tex.view(x, y, width, height).to_image()
    } else {
        tex.clone()
    }
}

pub fn composite_single_element(element: &Element, project: &Project) -> Result<Vec<u8>, String> {
    if let Some(asset_name) = sprite_asset_name(element, project) {
        if let Some(data) = project.texture_data.get(asset_name) {
            let tex = image::load_from_memory(data)
                .map_err(|e| format!("Failed to load texture '{}': {e}", asset_name))?
                .to_rgba8();
            let (w, h) = texture_target_size(element, &tex);
            let mut img = RgbaImage::new(w, h);
            overlay_texture_data(
                &mut img, project, element, data, asset_name, element.x, element.y,
            )?;
            return encode_png(img);
        }
    }

    let w = element.width.unwrap_or(16);
    let h = element.height.unwrap_or(16);
    let color = match element.element_type {
        ElementType::Progress => Rgba([0xE9, 0xA2, 0x3B, 0xFF]),
        ElementType::FluidTank => Rgba([0x3B, 0x82, 0xE9, 0xFF]),
        ElementType::EnergyBar => Rgba([0xEF, 0x44, 0x44, 0xFF]),
        _ => Rgba([0xFF, 0xFF, 0xFF, 0xFF]),
    };

    let img = RgbaImage::from_pixel(w, h, color);
    let mut bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn sprite_asset_name<'a>(element: &'a Element, project: &'a Project) -> Option<&'a str> {
    element.asset.as_deref().or_else(|| {
        let animation_id = element.animation.as_deref()?;
        project
            .animations
            .iter()
            .find(|animation| animation.id == animation_id)
            .and_then(|animation| animation.texture.as_deref())
    })
}

fn encode_png(img: RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .map_err(|e| format!("Failed to encode generated PNG: {e}"))?;
    Ok(bytes)
}

pub fn generated_gui_panel(width: u32, height: u32) -> Result<Vec<u8>, String> {
    encode_png(generator::generate_background(width, height))
}

pub fn generated_slot() -> Result<Vec<u8>, String> {
    encode_png(generator::generate_slot_frame())
}

pub fn generated_button() -> Result<Vec<u8>, String> {
    // The new generator doesn't have a 200x20 button yet, so we'll use a fixed size for now or adapt it
    // Actually generator.rs has generate_all_defaults which uses textures/button.png? No, it's textures/slot_frame.png etc.
    // Wait, let's look at generator.rs again.
    let mut img = image::RgbaImage::from_pixel(200, 20, image::Rgba([0x9a, 0x9a, 0x9a, 0xff]));
    // ... we can implement it here or in generator.rs
    // For now, let's just keep the existing button generator or improve it slightly to match the style
    for x in 0..200 {
        img.put_pixel(x, 0, image::Rgba([0x37, 0x37, 0x37, 0xff]));
        img.put_pixel(x, 19, image::Rgba([0x55, 0x55, 0x55, 0xff]));
    }
    for y in 0..20 {
        img.put_pixel(0, y, image::Rgba([0x37, 0x37, 0x37, 0xff]));
        img.put_pixel(199, y, image::Rgba([0x55, 0x55, 0x55, 0xff]));
    }
    for x in 1..199 {
        img.put_pixel(x, 1, image::Rgba([0xff, 0xff, 0xff, 0xff]));
        img.put_pixel(x, 18, image::Rgba([0x6b, 0x6b, 0x6b, 0xff]));
    }
    for y in 1..19 {
        img.put_pixel(1, y, image::Rgba([0xff, 0xff, 0xff, 0xff]));
        img.put_pixel(198, y, image::Rgba([0x6b, 0x6b, 0x6b, 0xff]));
    }
    encode_png(img)
}

#[allow(dead_code)]
pub fn generated_progress_arrow() -> Result<Vec<u8>, String> {
    encode_png(generator::generate_progress_arrow())
}

#[allow(dead_code)]
pub fn generated_fluid_tank() -> Result<Vec<u8>, String> {
    encode_png(generator::generate_fluid_frame())
}

#[allow(dead_code)]
pub fn generated_energy_bar() -> Result<Vec<u8>, String> {
    encode_png(generator::generate_energy_frame())
}

pub fn generated_scrollbar(width: u32, height: u32) -> Result<Vec<u8>, String> {
    let w = width.max(5);
    let h = height.max(9);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([0x6f, 0x6f, 0x6f, 0xff]));

    for x in 0..w {
        img.put_pixel(x, 0, Rgba([0x2f, 0x2f, 0x2f, 0xff]));
        img.put_pixel(x, h - 1, Rgba([0x2f, 0x2f, 0x2f, 0xff]));
    }
    for y in 0..h {
        img.put_pixel(0, y, Rgba([0x2f, 0x2f, 0x2f, 0xff]));
        img.put_pixel(w - 1, y, Rgba([0x2f, 0x2f, 0x2f, 0xff]));
    }

    let thumb_x = 2.min(w - 1);
    let thumb_y = 2.min(h - 1);
    let thumb_w = w.saturating_sub(4).max(1);
    let thumb_h = ((h - 4) / 3).max(5).min(h.saturating_sub(4).max(1));
    for y in thumb_y..(thumb_y + thumb_h).min(h - 1) {
        for x in thumb_x..(thumb_x + thumb_w).min(w - 1) {
            img.put_pixel(x, y, Rgba([0xb8, 0xb8, 0xb8, 0xff]));
        }
    }

    encode_png(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{
        AssetMetadata, Element, ElementType, ModTarget, NineSlice, NineSliceMode, Project,
        TextureRenderMode, UvRect,
    };
    use image::{Rgba, RgbaImage};

    fn png_bytes() -> Vec<u8> {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
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

    fn grid_png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> Rgba<u8>) -> Vec<u8> {
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(x, y, pixel(x, y));
            }
        }
        let mut bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    fn test_nine_slice(edge_mode: NineSliceMode, center_mode: NineSliceMode) -> NineSlice {
        NineSlice {
            left: 1,
            right: 1,
            top: 1,
            bottom: 1,
            edge_mode,
            center_mode,
        }
    }

    fn button_element(id: &str, x: i32, y: i32) -> Element {
        Element {
            id: id.to_string(),
            element_type: ElementType::Button,
            x,
            y,
            width: Some(20),
            height: Some(20),
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

    fn texture_element(id: &str, asset: &str, width: u32, height: u32) -> Element {
        let mut element = button_element(id, 0, 0);
        element.element_type = ElementType::Texture;
        element.width = Some(width);
        element.height = Some(height);
        element.asset = Some(asset.to_string());
        element
    }

    #[test]
    fn validate_nine_slice_guides_rejects_large_values_without_panicking() {
        let guides = NineSlice {
            left: u32::MAX,
            right: 1,
            top: 0,
            bottom: 0,
            edge_mode: NineSliceMode::Tile,
            center_mode: NineSliceMode::Tile,
        };

        let result = std::panic::catch_unwind(|| validate_nine_slice_guides(&guides, 3, 3));

        assert!(matches!(
            result,
            Ok(Err(error)) if error == "Nine-slice guides leave no center region"
        ));
    }

    #[test]
    fn composite_single_element_uses_natural_texture_size_when_dimensions_are_omitted() {
        let asset = "textures/gui/large_single.png";
        let mut project = Project::new("Single Natural Texture", 1, 1, ModTarget::Forge);
        project.texture_data.insert(
            asset.into(),
            grid_png(24, 20, |x, y| Rgba([x as u8, y as u8, 0, 0xff])),
        );
        let mut element = button_element("single", 0, 0);
        element.element_type = ElementType::Texture;
        element.width = None;
        element.height = None;
        element.asset = Some(asset.into());

        let png = composite_single_element(&element, &project).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();

        assert_eq!(image.dimensions(), (24, 20));
        assert_eq!(image.get_pixel(23, 19).0, [23, 19, 0, 0xff]);
    }

    #[test]
    fn composite_atlas_tiles_nine_slice_center_across_partial_repeat() {
        let asset = "textures/gui/tiled_panel.png";
        let mut project = Project::new("Nine Slice Tile Partial", 7, 7, ModTarget::Forge);
        project.texture_data.insert(
            asset.into(),
            grid_png(4, 4, |x, y| Rgba([x as u8, y as u8, 0, 0xff])),
        );
        let mut panel = texture_element("panel", asset, 7, 7);
        panel.render_mode = TextureRenderMode::NineSlice;
        panel.nine_slice = Some(test_nine_slice(NineSliceMode::Tile, NineSliceMode::Tile));
        project.elements.push(panel);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(5, 5).0, [1, 1, 0, 0xff]);
    }

    #[test]
    fn composite_atlas_allows_zero_nine_slice_guides() {
        let asset = "textures/gui/zero_guides.png";
        let mut project = Project::new("Nine Slice Zero Guides", 5, 5, ModTarget::Forge);
        project.texture_data.insert(
            asset.into(),
            grid_png(2, 2, |x, y| Rgba([x as u8, y as u8, 0, 0xff])),
        );
        let mut panel = texture_element("panel", asset, 5, 5);
        panel.render_mode = TextureRenderMode::NineSlice;
        panel.nine_slice = Some(NineSlice {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
            edge_mode: NineSliceMode::Tile,
            center_mode: NineSliceMode::Tile,
        });
        project.elements.push(panel);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(4, 4).0, [0, 0, 0, 0xff]);
    }

    #[test]
    fn composite_atlas_applies_nine_slice_guides_after_uv_crop() {
        let asset = "textures/gui/uv_panel.png";
        let mut project = Project::new("Nine Slice UV", 5, 5, ModTarget::Forge);
        project.texture_data.insert(
            asset.into(),
            grid_png(5, 3, |x, y| Rgba([x as u8, y as u8, 0, 0xff])),
        );
        let mut panel = texture_element("panel", asset, 5, 5);
        panel.render_mode = TextureRenderMode::NineSlice;
        panel.uv = Some(UvRect {
            x: 1,
            y: 0,
            width: 3,
            height: 3,
        });
        panel.nine_slice = Some(test_nine_slice(
            NineSliceMode::Stretch,
            NineSliceMode::Stretch,
        ));
        project.elements.push(panel);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(0, 0).0, [1, 0, 0, 0xff]);
        assert_eq!(image.get_pixel(3, 3).0, [2, 1, 0, 0xff]);
    }

    #[test]
    fn composite_atlas_uses_element_nine_slice_before_asset_metadata() {
        let asset = "textures/gui/panel.png";
        let mut project = Project::new("Nine Slice Override", 6, 6, ModTarget::Forge);
        project.texture_data.insert(
            asset.into(),
            grid_png(4, 4, |x, y| Rgba([x as u8, y as u8, 0, 0xff])),
        );
        project.asset_metadata.insert(
            asset.into(),
            AssetMetadata {
                width: None,
                height: None,
                nine_slice: Some(test_nine_slice(
                    NineSliceMode::Stretch,
                    NineSliceMode::Stretch,
                )),
            },
        );
        let mut panel = texture_element("panel", asset, 6, 6);
        panel.render_mode = TextureRenderMode::NineSlice;
        panel.nine_slice = Some(test_nine_slice(NineSliceMode::Tile, NineSliceMode::Tile));
        project.elements.push(panel);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(2, 2).0, [2, 2, 0, 0xff]);
    }

    #[test]
    fn composite_atlas_stretches_nine_slice_center_from_asset_metadata() {
        let asset = "textures/gui/stretch_panel.png";
        let mut project = Project::new("Nine Slice Stretch", 9, 9, ModTarget::Forge);
        project.texture_data.insert(
            asset.into(),
            grid_png(3, 3, |x, y| Rgba([x as u8, y as u8, 0, 0xff])),
        );
        project.asset_metadata.insert(
            asset.into(),
            AssetMetadata {
                width: None,
                height: None,
                nine_slice: Some(NineSlice {
                    left: 1,
                    right: 1,
                    top: 1,
                    bottom: 1,
                    edge_mode: NineSliceMode::Stretch,
                    center_mode: NineSliceMode::Stretch,
                }),
            },
        );
        let mut panel = texture_element("panel", asset, 9, 9);
        panel.render_mode = TextureRenderMode::NineSlice;
        project.elements.push(panel);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(7, 7).0, [1, 1, 0, 0xff]);
    }

    #[test]
    fn composite_atlas_rejects_nine_slice_without_guides() {
        let asset = "textures/gui/missing_guides.png";
        let mut project = Project::new("Nine Slice Missing Guides", 6, 6, ModTarget::Forge);
        project
            .texture_data
            .insert(asset.into(), test_png(3, 3, Rgba([0xff, 0, 0, 0xff])));
        let mut panel = texture_element("panel", asset, 6, 6);
        panel.render_mode = TextureRenderMode::NineSlice;
        project.elements.push(panel);

        let err = composite_atlas_for_layer(&project, Layer::Background).unwrap_err();

        assert_eq!(
            err,
            "Texture element 'panel' uses nine_slice without guides"
        );
    }

    #[test]
    fn composite_atlas_rejects_nine_slice_guides_that_leave_no_center_region() {
        let asset = "textures/gui/invalid_guides.png";
        let mut project = Project::new("Nine Slice Invalid Guides", 6, 6, ModTarget::Forge);
        project
            .texture_data
            .insert(asset.into(), test_png(3, 3, Rgba([0xff, 0, 0, 0xff])));
        let mut panel = texture_element("panel", asset, 6, 6);
        panel.render_mode = TextureRenderMode::NineSlice;
        panel.nine_slice = Some(NineSlice {
            left: 2,
            right: 1,
            top: 1,
            bottom: 1,
            edge_mode: NineSliceMode::Tile,
            center_mode: NineSliceMode::Tile,
        });
        project.elements.push(panel);

        let err = composite_atlas_for_layer(&project, Layer::Background).unwrap_err();

        assert!(err.contains("leave no center region"));
    }

    #[test]
    fn background_export_bakes_button_standalone_icon_pixels() {
        let mut project = Project::new("Icon Button", 64, 32, crate::project::ModTarget::Forge);
        project.texture_data.insert(
            "textures/gui/icons/settings.png".into(),
            test_png(8, 8, Rgba([0x11, 0x22, 0x33, 0xff])),
        );
        project
            .assets
            .push("textures/gui/icons/settings.png".into());
        let mut button = button_element("button", 8, 6);
        button.icon = Some("textures/gui/icons/settings.png".into());
        project.elements.push(button);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(14, 12), &Rgba([0x11, 0x22, 0x33, 0xff]));
    }

    #[test]
    fn background_export_bakes_button_icon_uv_pixels() {
        let mut project = Project::new("Icon UV Button", 64, 32, crate::project::ModTarget::Forge);
        let mut atlas = RgbaImage::from_pixel(16, 8, Rgba([0x00, 0x00, 0x00, 0xff]));
        for x in 8..16 {
            for y in 0..8 {
                atlas.put_pixel(x, y, Rgba([0xaa, 0x44, 0x11, 0xff]));
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
            .insert("textures/gui/widgets.png".into(), bytes);
        project.assets.push("textures/gui/widgets.png".into());
        let mut button = button_element("button", 8, 6);
        button.icon = Some("textures/gui/widgets.png".into());
        button.icon_uv = Some(crate::project::UvRect {
            x: 8,
            y: 0,
            width: 8,
            height: 8,
        });
        project.elements.push(button);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(14, 12), &Rgba([0xaa, 0x44, 0x11, 0xff]));
    }

    #[test]
    fn background_export_preserves_rectangular_button_icon_aspect_ratio() {
        let mut project = Project::new(
            "Rectangular Icon Button",
            64,
            32,
            crate::project::ModTarget::Forge,
        );
        project.texture_data.insert(
            "textures/gui/icons/wide.png".into(),
            test_png(32, 24, Rgba([0x11, 0xaa, 0xee, 0xff])),
        );
        project.assets.push("textures/gui/icons/wide.png".into());
        let mut button = button_element("button", 8, 6);
        button.icon = Some("textures/gui/icons/wide.png".into());
        project.elements.push(button);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(12, 11), &Rgba([0x11, 0xaa, 0xee, 0xff]));
        assert_ne!(image.get_pixel(12, 9), &Rgba([0x11, 0xaa, 0xee, 0xff]));
    }

    #[test]
    fn background_export_expands_to_visual_bounds_for_outside_elements() {
        let mut project = Project::new("Outside", 100, 80, ModTarget::Forge);
        project.texture_data.insert(
            "textures/flair.png".into(),
            test_png(32, 32, Rgba([0xd7, 0xa3, 0x39, 0xff])),
        );
        let mut flair = button_element("flair", 84, -16);
        flair.element_type = ElementType::Texture;
        flair.width = Some(32);
        flair.height = Some(32);
        flair.asset = Some("textures/flair.png".into());
        project.elements.push(flair);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.dimensions(), (116, 96));
        assert_eq!(image.get_pixel(84, 0).0, [0xd7, 0xa3, 0x39, 0xff]);
    }

    #[test]
    fn background_export_remains_main_size_when_elements_stay_inside() {
        let mut project = Project::new("Inside", 100, 80, ModTarget::Forge);
        project.texture_data.insert(
            "textures/panel.png".into(),
            test_png(10, 10, Rgba([0x11, 0x22, 0x33, 0xff])),
        );
        let mut panel = button_element("panel", 10, 10);
        panel.element_type = ElementType::Texture;
        panel.width = Some(10);
        panel.height = Some(10);
        panel.asset = Some("textures/panel.png".into());
        project.elements.push(panel);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.dimensions(), (100, 80));
    }

    #[test]
    fn composite_atlas_rejects_unrenderable_visual_bounds() {
        let mut project = Project::new("Huge", 4097, 1, ModTarget::Forge);
        project.elements.push(button_element("button", 0, 0));

        let err = composite_atlas_for_layer(&project, Layer::Background).unwrap_err();

        assert!(err.contains("unrenderable atlas size 4097x20"));
    }

    #[test]
    fn empty_layer_atlas_uses_main_gui_size_not_expanded_visual_bounds() {
        let mut project = Project::new("Empty Overlay", 100, 80, ModTarget::Forge);
        project.texture_data.insert(
            "textures/flair.png".into(),
            test_png(32, 32, Rgba([0xd7, 0xa3, 0x39, 0xff])),
        );
        let mut flair = button_element("flair", 120, 0);
        flair.element_type = ElementType::Texture;
        flair.width = Some(32);
        flair.height = Some(32);
        flair.asset = Some("textures/flair.png".into());
        project.elements.push(flair);

        let atlas = composite_atlas_for_layer(&project, Layer::Overlay).unwrap();
        let image = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(image.dimensions(), (100, 80));
        assert!(image.pixels().all(|pixel| pixel.0 == [0, 0, 0, 0]));
    }

    #[test]
    fn composite_atlas_crops_texture_uv_before_scaling() {
        let asset = "textures/sheet.png".to_string();
        let mut project = Project::new("UV", 1, 1, ModTarget::Forge);
        project.texture_data.insert(asset.clone(), png_bytes());
        project.elements.push(Element {
            id: "texture_1".to_string(),
            element_type: ElementType::Texture,
            x: 0,
            y: 0,
            width: Some(1),
            height: Some(1),
            size: None,
            asset: Some(asset),
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
            uv: Some(UvRect {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
            }),
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

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let pixel = image::load_from_memory(&atlas)
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)
            .0;

        assert_eq!(pixel, [0, 255, 0, 255]);
    }

    #[test]
    fn composite_atlas_skips_uv_with_no_remaining_source_area() {
        let cases = [
            UvRect {
                x: 2,
                y: 0,
                width: 1,
                height: 1,
            },
            UvRect {
                x: 0,
                y: 1,
                width: 1,
                height: 1,
            },
            UvRect {
                x: 5,
                y: 5,
                width: 1,
                height: 1,
            },
        ];

        for uv in cases {
            let asset = "textures/sheet.png".to_string();
            let mut project = Project::new("UV edge", 1, 1, ModTarget::Forge);
            project.texture_data.insert(asset.clone(), png_bytes());
            project.elements.push(Element {
                id: "texture_1".to_string(),
                element_type: ElementType::Texture,
                x: 0,
                y: 0,
                width: Some(1),
                height: Some(1),
                size: None,
                asset: Some(asset),
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
                uv: Some(uv),
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

            let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
            let pixel = image::load_from_memory(&atlas)
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0;

            assert_eq!(pixel, [0, 0, 0, 0]);
        }
    }

    #[test]
    fn generated_gui_panel_is_deterministic_and_requested_size() {
        let first = generated_gui_panel(176, 166).unwrap();
        let second = generated_gui_panel(176, 166).unwrap();

        assert_eq!(first, second);

        let decoded = image::load_from_memory(&first).unwrap().to_rgba8();
        assert_eq!(decoded.width(), 176);
        assert_eq!(decoded.height(), 166);
    }

    #[test]
    fn generated_gui_panel_export_uses_element_size_not_stale_asset_size() {
        let mut project = Project::new("Generated Panel", 232, 242, ModTarget::Forge);
        let asset = "textures/generated/gui_panel.png";
        project.assets.push(asset.into());
        project.texture_data.insert(
            asset.into(),
            test_png(230, 258, Rgba([0xff, 0x00, 0xff, 0xff])),
        );
        project
            .elements
            .push(texture_element("background", asset, 232, 242));

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let decoded = image::load_from_memory(&atlas).unwrap().to_rgba8();
        let expected = image::load_from_memory(&generated_gui_panel(232, 242).unwrap())
            .unwrap()
            .to_rgba8();

        assert_eq!(decoded.dimensions(), expected.dimensions());
        assert_eq!(decoded.get_pixel(0, 0), expected.get_pixel(0, 0));
        assert_eq!(decoded.get_pixel(8, 8), expected.get_pixel(8, 8));
        assert_ne!(decoded.get_pixel(8, 8).0, [0xff, 0x00, 0xff, 0xff]);
    }

    #[test]
    fn generated_gui_panel_nine_slice_uses_source_asset() {
        let mut project = Project::new("Generated Panel Source", 32, 32, ModTarget::Forge);
        let asset = "textures/generated/gui_panel.png";
        project.assets.push(asset.into());
        project.texture_data.insert(
            asset.into(),
            test_png(16, 16, Rgba([0xff, 0x00, 0xff, 0xff])),
        );
        let mut background = texture_element("background", asset, 32, 32);
        background.render_mode = TextureRenderMode::NineSlice;
        background.nine_slice = Some(test_nine_slice(NineSliceMode::Tile, NineSliceMode::Tile));
        project.elements.push(background);

        let atlas = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let decoded = image::load_from_memory(&atlas).unwrap().to_rgba8();

        assert_eq!(decoded.dimensions(), (32, 32));
        assert_eq!(decoded.get_pixel(8, 8).0, [0xff, 0x00, 0xff, 0xff]);
    }

    #[test]
    fn generated_slot_is_eighteen_pixels_square() {
        let decoded = image::load_from_memory(&generated_slot().unwrap())
            .unwrap()
            .to_rgba8();

        assert_eq!(decoded.width(), 18);
        assert_eq!(decoded.height(), 18);
        assert_eq!(decoded.get_pixel(0, 0).0, [0x37, 0x37, 0x37, 0xff]);
    }

    #[test]
    fn generated_scrollbar_has_expected_size() {
        let png = generated_scrollbar(12, 54).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.width(), 12);
        assert_eq!(img.height(), 54);
    }

    #[test]
    fn background_export_bakes_default_button_pixels() {
        let mut project = Project::new("Button", 176, 166, ModTarget::Forge);
        project.elements.push(Element {
            id: "button".into(),
            element_type: ElementType::Button,
            x: 24,
            y: 48,
            width: Some(40),
            height: Some(20),
            size: None,
            asset: None,
            icon: None,
            icon_uv: None,
            tooltip: None,
            direction: None,
            content: Some("Start".into()),
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
        project.elements.push(Element {
            id: "toggle".into(),
            element_type: ElementType::ToggleButton,
            x: 72,
            y: 48,
            width: Some(40),
            height: Some(20),
            size: None,
            asset: None,
            icon: None,
            icon_uv: None,
            tooltip: None,
            direction: None,
            content: Some("Mode".into()),
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

        let png = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(24, 48).0, [0x37, 0x37, 0x37, 0xff]);
        assert_eq!(img.get_pixel(72, 48).0, [0x37, 0x37, 0x37, 0xff]);
    }

    #[test]
    fn background_export_bakes_scrollbar_pixels() {
        let mut project = Project::new("Scroll", 176, 166, ModTarget::Forge);
        project.elements.push(Element {
            id: "scroll".into(),
            element_type: ElementType::Scrollbar,
            x: 130,
            y: 54,
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
            scroll_binding: None,
            scroll_min: Some(0),
            scroll_max: Some(3),
            visible_rows: Some(3),
            total_rows: Some(6),
            columns: Some(5),
            target_group: Some("machine_buffer".into()),
            binding: None,
            dock: None,
            open_width: None,
            open_height: None,
            attached_region: None,
        });

        let png = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_ne!(img.get_pixel(130, 54).0[3], 0);
    }

    #[test]
    fn project_preview_composites_visible_layers_and_non_baked_elements() {
        let mut project = Project::new("Preview", 4, 4, ModTarget::Forge);
        project.texture_data.insert(
            "textures/background.png".into(),
            test_png(4, 4, Rgba([0x11, 0x22, 0x33, 0xff])),
        );
        project.texture_data.insert(
            "textures/overlay.png".into(),
            test_png(1, 1, Rgba([0x44, 0xaa, 0x66, 0xff])),
        );
        project.texture_data.insert(
            "textures/hidden.png".into(),
            test_png(1, 1, Rgba([0xff, 0x00, 0xff, 0xff])),
        );

        let mut background = button_element("background", 0, 0);
        background.element_type = ElementType::Texture;
        background.width = Some(4);
        background.height = Some(4);
        background.asset = Some("textures/background.png".into());
        project.elements.push(background);

        let mut overlay = button_element("overlay", 1, 1);
        overlay.element_type = ElementType::Texture;
        overlay.width = Some(1);
        overlay.height = Some(1);
        overlay.asset = Some("textures/overlay.png".into());
        overlay.layer = Layer::Overlay;
        project.elements.push(overlay);

        let mut hidden_overlay = button_element("hidden", 0, 0);
        hidden_overlay.element_type = ElementType::Texture;
        hidden_overlay.width = Some(1);
        hidden_overlay.height = Some(1);
        hidden_overlay.asset = Some("textures/hidden.png".into());
        hidden_overlay.layer = Layer::Overlay;
        hidden_overlay.visible = false;
        project.elements.push(hidden_overlay);

        let mut progress = button_element("progress", 2, 2);
        progress.element_type = ElementType::Progress;
        progress.width = Some(1);
        progress.height = Some(1);
        progress.layer = Layer::Animatable;
        project.elements.push(progress);

        let png = composite_project_preview(&project).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(0, 0).0, [0x11, 0x22, 0x33, 0xff]);
        assert_eq!(image.get_pixel(1, 1).0, [0x44, 0xaa, 0x66, 0xff]);
        assert_eq!(image.get_pixel(2, 2).0, [0xe9, 0xa2, 0x3b, 0xff]);
    }

    #[test]
    fn background_export_skips_hidden_baked_elements() {
        let mut project = Project::new("Hidden Baked", 2, 2, ModTarget::Forge);
        project.texture_data.insert(
            "textures/visible.png".into(),
            test_png(2, 2, Rgba([0x11, 0x22, 0x33, 0xff])),
        );
        project.texture_data.insert(
            "textures/hidden.png".into(),
            test_png(1, 1, Rgba([0xff, 0x00, 0xff, 0xff])),
        );

        let mut visible = button_element("visible", 0, 0);
        visible.element_type = ElementType::Texture;
        visible.width = Some(2);
        visible.height = Some(2);
        visible.asset = Some("textures/visible.png".into());
        project.elements.push(visible);

        let mut hidden = button_element("hidden", 0, 0);
        hidden.element_type = ElementType::Texture;
        hidden.width = Some(1);
        hidden.height = Some(1);
        hidden.asset = Some("textures/hidden.png".into());
        hidden.visible = false;
        project.elements.push(hidden);

        let png = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(0, 0).0, [0x11, 0x22, 0x33, 0xff]);
    }

    #[test]
    fn project_preview_preserves_element_order_when_baked_follows_non_baked() {
        let mut project = Project::new("Preview Order", 2, 2, ModTarget::Forge);
        project.texture_data.insert(
            "textures/later.png".into(),
            test_png(1, 1, Rgba([0x10, 0x80, 0xf0, 0xff])),
        );

        let mut progress = button_element("progress", 0, 0);
        progress.element_type = ElementType::Progress;
        progress.width = Some(1);
        progress.height = Some(1);
        project.elements.push(progress);

        let mut later_texture = button_element("later_texture", 0, 0);
        later_texture.element_type = ElementType::Texture;
        later_texture.width = Some(1);
        later_texture.height = Some(1);
        later_texture.asset = Some("textures/later.png".into());
        project.elements.push(later_texture);

        let png = composite_project_preview(&project).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();

        assert_eq!(image.get_pixel(0, 0).0, [0x10, 0x80, 0xf0, 0xff]);
    }
}
