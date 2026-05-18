use crate::project::{FontAsset, FontSource, GlyphInfo, GlyphMap};
use ab_glyph::Font;

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
    let mut x_offset = 0u32;
    let mut y_offset = 0u32;
    let mut row_height = 0u32;

    // Rasterize ASCII range
    for c in ' '..='~' {
        let glyph_id: ab_glyph::GlyphId = font.glyph_id(c);
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

            if x_offset + w > atlas_width {
                x_offset = 0;
                y_offset += row_height;
                row_height = 0;
            }

            glyph_map.insert(
                c,
                GlyphInfo {
                    x: x_offset,
                    y: y_offset,
                    width: w,
                    height: h,
                    ascent,
                },
            );

            x_offset += w;
            row_height = row_height.max(h);
        }
    }

    Ok(FontAsset {
        id: id.to_string(),
        source: FontSource::Ttf {
            font_data: font_data.to_vec(),
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
