use crate::project::{Element, ElementType, Layer, Project};
use image::{GenericImageView, Rgba, RgbaImage};

pub fn composite_atlas(project: &Project) -> Result<Vec<u8>, String> {
    composite_atlas_for_layer(project, Layer::Background)
}

pub fn composite_atlas_for_layer(project: &Project, layer: Layer) -> Result<Vec<u8>, String> {
    let w = project.gui_size.width;
    let h = project.gui_size.height;

    let mut img = RgbaImage::new(w, h);
    let has_elements = project.elements.iter().any(|el| {
        el.element_type == ElementType::Texture && el.layer == layer
    });

    if !has_elements {
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {e}"))?;
        return Ok(buf);
    }

    for el in &project.elements {
        if el.layer != layer || el.element_type != ElementType::Texture {
            continue;
        }
        if let Some(asset_name) = &el.asset {
            if let Some(data) = project.texture_data.get(asset_name) {
                let tex = image::load_from_memory(data)
                    .map_err(|e| format!("Failed to load texture '{}': {e}", asset_name))?;
                let tex = tex.to_rgba8();

                let tw = el.width.unwrap_or(tex.width());
                let th = el.height.unwrap_or(tex.height());

                let source = if let Some(uv) = &el.uv {
                    let x = uv.x.min(tex.width());
                    let y = uv.y.min(tex.height());
                    let width = uv.width.min(tex.width().saturating_sub(x));
                    let height = uv.height.min(tex.height().saturating_sub(y));
                    if width == 0 || height == 0 {
                        continue;
                    }
                    tex.view(x, y, width, height).to_image()
                } else {
                    tex
                };

                let resized =
                    image::imageops::resize(&source, tw, th, image::imageops::FilterType::Nearest);
                image::imageops::overlay(&mut img, &resized, el.x as i64, el.y as i64);
            }
        }
    }

    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {e}"))?;

    Ok(buf)
}

pub fn composite_single_element(element: &Element, _project: &Project) -> Result<Vec<u8>, String> {
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
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
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
        });

        let atlas = composite_atlas(&project).unwrap();
        let pixel = image::load_from_memory(&atlas)
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)
            .0;

        assert_eq!(pixel, [0, 255, 0, 255]);
    }
}
