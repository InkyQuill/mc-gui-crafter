use crate::project::{FontAsset, FontSource, GlyphInfo, GlyphMap};
use ab_glyph::{Font, ScaleFont};
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

/// Rasterize a TTF/OTF font into a FontAsset with glyph map.
pub fn rasterize_ttf(font_data: &[u8], font_size: u32, id: &str) -> Result<FontAsset, String> {
    let font = ab_glyph::FontArc::try_from_vec(font_data.to_vec())
        .map_err(|e| format!("Failed to parse font: {e}"))?;

    let mut glyph_map = GlyphMap::new();
    let scale = ab_glyph::PxScale {
        x: font_size as f32,
        y: font_size as f32,
    };

    let atlas_width = 256u32;
    let atlas_height = 256u32;
    let mut atlas = RgbaImage::from_pixel(atlas_width, atlas_height, Rgba([0, 0, 0, 0]));
    let scaled_font = font.as_scaled(scale);
    let mut x_offset = 0u32;
    let mut y_offset = 0u32;
    let mut row_height = 0u32;

    // Rasterize ASCII range
    for c in ' '..='~' {
        let glyph_id: ab_glyph::GlyphId = font.glyph_id(c);
        let advance = scaled_font.h_advance(glyph_id).ceil().max(0.0) as u32;
        let glyph: ab_glyph::Glyph = ab_glyph::Glyph {
            id: glyph_id,
            scale,
            position: ab_glyph::point(0.0, 0.0),
        };
        let outlined: Option<ab_glyph::OutlinedGlyph> = font.outline_glyph(glyph);
        if let Some(outlined) = outlined {
            let bounds = outlined.px_bounds();
            let w = (bounds.width().ceil() as u32).max(1);
            let h = (bounds.height().ceil() as u32).max(1);
            let ascent = (-bounds.min.y).ceil() as i32;
            let bearing_x = bounds.min.x.floor() as i32;
            let bearing_y = bounds.min.y.floor() as i32;

            if x_offset + w > atlas_width {
                x_offset = 0;
                y_offset += row_height;
                row_height = 0;
            }
            if y_offset + h > atlas_height {
                return Err("Font atlas is too small for ASCII glyph range".to_string());
            }

            glyph_map.insert(
                c,
                GlyphInfo {
                    x: x_offset,
                    y: y_offset,
                    width: w,
                    height: h,
                    ascent,
                    advance,
                    bearing_x,
                    bearing_y,
                },
            );

            let draw_x = x_offset;
            let draw_y = y_offset;
            outlined.draw(|x, y, coverage| {
                let px = draw_x + x;
                let py = draw_y + y;
                if px < atlas_width && py < atlas_height {
                    let alpha = (coverage * 255.0).round() as u8;
                    atlas.put_pixel(px, py, Rgba([255, 255, 255, alpha]));
                }
            });

            x_offset += w;
            row_height = row_height.max(h);
        } else {
            glyph_map.insert(
                c,
                GlyphInfo {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                    ascent: 0,
                    advance,
                    bearing_x: 0,
                    bearing_y: 0,
                },
            );
        }
    }

    let mut atlas_png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(atlas)
        .write_to(&mut atlas_png, ImageFormat::Png)
        .map_err(|e| format!("Failed to encode font atlas: {e}"))?;

    Ok(FontAsset {
        id: id.to_string(),
        source: FontSource::Ttf {
            atlas_png: atlas_png.into_inner(),
            font_size,
            glyph_map,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterize_invalid_data_returns_error() {
        let result = rasterize_ttf(b"not a font", 16, "test");
        assert!(result.is_err());
    }
}
