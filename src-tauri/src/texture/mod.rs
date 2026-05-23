use crate::project::{Element, ElementType, Layer, Project};
use image::{GenericImageView, Rgba, RgbaImage};

pub fn composite_atlas_for_layer(project: &Project, layer: Layer) -> Result<Vec<u8>, String> {
    let w = project.gui_size.width;
    let h = project.gui_size.height;

    let mut img = RgbaImage::new(w, h);
    let has_elements = project
        .elements
        .iter()
        .any(|el| el.layer == layer && is_baked_atlas_element(el));

    if !has_elements {
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {e}"))?;
        return Ok(buf);
    }

    for el in &project.elements {
        if el.layer != layer || !is_baked_atlas_element(el) {
            continue;
        }

        match el.element_type {
            ElementType::Texture => {
                if let Some(asset_name) = &el.asset {
                    overlay_asset(&mut img, project, el, asset_name)?;
                }
            }
            ElementType::Slot | ElementType::VirtualSlotCell => {
                overlay_slot(&mut img, project, el)?;
            }
            ElementType::Button | ElementType::ToggleButton => {
                overlay_button(&mut img, project, el)?;
            }
            ElementType::Scrollbar => {
                let w = el.width.or(el.size).unwrap_or(12);
                let h = el.height.or(el.size).unwrap_or(54);
                let data = generated_scrollbar(w, h)?;
                overlay_texture_data(&mut img, el, &data, "generated scrollbar")?;
            }
            _ => {}
        }
    }

    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {e}"))?;

    Ok(buf)
}

pub fn composite_project_preview(project: &Project) -> Result<Vec<u8>, String> {
    composite_atlas_for_layer(project, Layer::Background)
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

fn overlay_slot(img: &mut RgbaImage, project: &Project, element: &Element) -> Result<(), String> {
    if let Some(asset_name) = element.asset.as_deref().or_else(|| {
        project
            .texture_data
            .contains_key("textures/generated/slot.png")
            .then_some("textures/generated/slot.png")
    }) {
        return overlay_asset(img, project, element, asset_name);
    }

    let data = generated_slot()?;
    overlay_texture_data(img, element, &data, "generated slot")
}

fn overlay_button(img: &mut RgbaImage, project: &Project, element: &Element) -> Result<(), String> {
    if let Some(asset_name) = element.asset.as_deref().or_else(|| {
        project
            .texture_data
            .contains_key("textures/generated/button.png")
            .then_some("textures/generated/button.png")
    }) {
        overlay_asset(img, project, element, asset_name)?;
    } else {
        let data = generated_button()?;
        overlay_texture_data(img, element, &data, "generated button")?;
    }

    overlay_button_icon(img, project, element)
}

fn overlay_button_icon(
    img: &mut RgbaImage,
    project: &Project,
    element: &Element,
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
    let element_w = element.width.or(element.size).unwrap_or(20);
    let element_h = element.height.or(element.size).unwrap_or(20);
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
    let x = element.x + (element_w.saturating_sub(target_w) / 2) as i32;
    let y = element.y + (element_h.saturating_sub(target_h) / 2) as i32;
    image::imageops::overlay(img, &resized, x as i64, y as i64);
    Ok(())
}

fn overlay_asset(
    img: &mut RgbaImage,
    project: &Project,
    element: &Element,
    asset_name: &str,
) -> Result<(), String> {
    let Some(data) = project.texture_data.get(asset_name) else {
        return Ok(());
    };
    overlay_texture_data(img, element, data, asset_name)
}

fn overlay_texture_data(
    img: &mut RgbaImage,
    element: &Element,
    data: &[u8],
    asset_name: &str,
) -> Result<(), String> {
    let tex = image::load_from_memory(data)
        .map_err(|e| format!("Failed to load texture '{}': {e}", asset_name))?
        .to_rgba8();

    let tw = element.width.or(element.size).unwrap_or(tex.width());
    let th = element.height.or(element.size).unwrap_or(tex.height());

    let source = cropped_source(&tex, element.uv.as_ref());
    if source.width() == 0 || source.height() == 0 {
        return Ok(());
    }

    let resized = image::imageops::resize(&source, tw, th, image::imageops::FilterType::Nearest);
    image::imageops::overlay(img, &resized, element.x as i64, element.y as i64);
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
    let w = element.width.unwrap_or(16);
    let h = element.height.unwrap_or(16);

    if let Some(asset_name) = sprite_asset_name(element, project) {
        if let Some(data) = project.texture_data.get(asset_name) {
            let tex = image::load_from_memory(data)
                .map_err(|e| format!("Failed to load texture '{}': {e}", asset_name))?
                .to_rgba8();
            let source = cropped_source(&tex, element.uv.as_ref());
            if source.width() == 0 || source.height() == 0 {
                return encode_png(RgbaImage::new(w, h));
            }
            let resized =
                image::imageops::resize(&source, w, h, image::imageops::FilterType::Nearest);
            return encode_png(resized);
        }
    }

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

fn noise(x: u32, y: u32) -> u8 {
    let n = x
        .wrapping_mul(37)
        .wrapping_add(y.wrapping_mul(17))
        .wrapping_add(13);
    (n % 9) as u8
}

pub fn generated_gui_panel(width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::new(width.max(1), height.max(1));
    let w = img.width();
    let h = img.height();

    for y in 0..h {
        for x in 0..w {
            let shade = 0xb8u8.saturating_add(noise(x, y));
            img.put_pixel(x, y, Rgba([shade, shade, shade, 0xff]));
        }
    }

    for x in 0..w {
        img.put_pixel(x, 0, Rgba([0xff, 0xff, 0xff, 0xff]));
        img.put_pixel(x, h - 1, Rgba([0x55, 0x55, 0x55, 0xff]));
    }
    for y in 0..h {
        img.put_pixel(0, y, Rgba([0xff, 0xff, 0xff, 0xff]));
        img.put_pixel(w - 1, y, Rgba([0x55, 0x55, 0x55, 0xff]));
    }

    encode_png(img)
}

pub fn generated_slot() -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::from_pixel(18, 18, Rgba([0x8b, 0x8b, 0x8b, 0xff]));
    for i in 0..18 {
        img.put_pixel(i, 0, Rgba([0x37, 0x37, 0x37, 0xff]));
        img.put_pixel(0, i, Rgba([0x37, 0x37, 0x37, 0xff]));
        img.put_pixel(i, 17, Rgba([0xff, 0xff, 0xff, 0xff]));
        img.put_pixel(17, i, Rgba([0xff, 0xff, 0xff, 0xff]));
    }
    for y in 2..16 {
        for x in 2..16 {
            img.put_pixel(x, y, Rgba([0x70, 0x70, 0x70, 0xff]));
        }
    }
    encode_png(img)
}

pub fn generated_button() -> Result<Vec<u8>, String> {
    let width = 200;
    let height = 20;
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0x9a, 0x9a, 0x9a, 0xff]));

    for x in 0..width {
        img.put_pixel(x, 0, Rgba([0x37, 0x37, 0x37, 0xff]));
        img.put_pixel(x, height - 1, Rgba([0x55, 0x55, 0x55, 0xff]));
    }
    for y in 0..height {
        img.put_pixel(0, y, Rgba([0x37, 0x37, 0x37, 0xff]));
        img.put_pixel(width - 1, y, Rgba([0x55, 0x55, 0x55, 0xff]));
    }

    for x in 1..width - 1 {
        img.put_pixel(x, 1, Rgba([0xff, 0xff, 0xff, 0xff]));
    }
    for y in 1..height - 1 {
        img.put_pixel(1, y, Rgba([0xff, 0xff, 0xff, 0xff]));
    }
    for x in 1..width - 1 {
        img.put_pixel(x, height - 2, Rgba([0x6b, 0x6b, 0x6b, 0xff]));
    }
    for y in 1..height - 1 {
        img.put_pixel(width - 2, y, Rgba([0x6b, 0x6b, 0x6b, 0xff]));
    }

    encode_png(img)
}

pub fn generated_progress_arrow() -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::from_pixel(22, 15, Rgba([0x00, 0x00, 0x00, 0x00]));
    for y in 4..11 {
        for x in 0..14 {
            img.put_pixel(x, y, Rgba([0x8a, 0x8a, 0x8a, 0xff]));
        }
    }
    for offset in 0..7 {
        for y in (4 + offset)..=(10 - offset) {
            img.put_pixel(14 + offset, y, Rgba([0x8a, 0x8a, 0x8a, 0xff]));
        }
    }
    encode_png(img)
}

pub fn generated_fluid_tank() -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::from_pixel(18, 54, Rgba([0x22, 0x2a, 0x33, 0xff]));
    for x in 0..18 {
        img.put_pixel(x, 0, Rgba([0xd8, 0xe8, 0xff, 0xff]));
        img.put_pixel(x, 53, Rgba([0x28, 0x32, 0x3c, 0xff]));
    }
    for y in 0..54 {
        img.put_pixel(0, y, Rgba([0xd8, 0xe8, 0xff, 0xff]));
        img.put_pixel(17, y, Rgba([0x28, 0x32, 0x3c, 0xff]));
    }
    encode_png(img)
}

pub fn generated_energy_bar() -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::from_pixel(12, 54, Rgba([0x28, 0x18, 0x18, 0xff]));
    for x in 0..12 {
        img.put_pixel(x, 0, Rgba([0xa8, 0x54, 0x54, 0xff]));
        img.put_pixel(x, 53, Rgba([0x38, 0x10, 0x10, 0xff]));
    }
    for y in 0..54 {
        img.put_pixel(0, y, Rgba([0xa8, 0x54, 0x54, 0xff]));
        img.put_pixel(11, y, Rgba([0x38, 0x10, 0x10, 0xff]));
    }
    encode_png(img)
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
    use crate::project::{Element, ElementType, ModTarget, Project, UvRect};
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
        });

        let png = composite_atlas_for_layer(&project, Layer::Background).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_ne!(img.get_pixel(130, 54).0[3], 0);
    }
}
