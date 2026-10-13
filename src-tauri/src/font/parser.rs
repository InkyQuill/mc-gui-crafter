use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FontJson {
    providers: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct BitmapProviderJson {
    #[serde(rename = "type")]
    provider_type: String,
    file: Option<String>,
    ascent: Option<i32>,
    height: Option<u32>,
    chars: Option<Vec<String>>,
    advances: Option<std::collections::HashMap<char, u32>>,
}

/// Parse a Minecraft font JSON file into a list of BitmapProviders and a GlyphMap.
pub fn parse_font_json(
    json: &str,
) -> Result<
    (
        Vec<crate::project::BitmapProvider>,
        crate::project::GlyphMap,
    ),
    String,
> {
    let font: FontJson =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse font JSON: {e}"))?;

    let mut providers = Vec::new();
    let mut glyph_map = crate::project::GlyphMap::new();

    for provider_value in &font.providers {
        let provider: BitmapProviderJson = serde_json::from_value(provider_value.clone())
            .map_err(|e| format!("Failed to parse provider: {e}"))?;

        match provider.provider_type.as_str() {
            "bitmap" => {
                let file = provider.file.clone().unwrap_or_default();
                let ascent = provider.ascent.unwrap_or(8);
                let height = provider.height.unwrap_or(8);
                let chars = provider.chars.clone().unwrap_or_default();

                let bp = crate::project::BitmapProvider {
                    file,
                    ascent,
                    height,
                    chars: chars.clone(),
                    image_data: vec![],
                    image_width: 0,
                    image_height: 0,
                };

                providers.push(bp);
            }
            "reference" => {
                // References are resolved by the caller
            }
            "space" => {
                // Space providers add whitespace mappings
                let default_advances = [
                    (' ', 4),
                    ('\u{2003}', 8),
                    ('\u{2002}', 4),
                    ('\u{2004}', 2),
                    ('\u{2005}', 2),
                    ('\u{2006}', 1),
                    ('\u{2007}', 4),
                    ('\u{2008}', 2),
                    ('\u{2009}', 1),
                    ('\u{200a}', 1),
                ];
                for (ch, advance) in provider.advances.unwrap_or_else(|| {
                    default_advances
                        .into_iter()
                        .collect::<std::collections::HashMap<_, _>>()
                }) {
                    glyph_map.entry(ch).or_insert(crate::project::GlyphInfo {
                        x: 0,
                        y: 0,
                        width: 4,
                        height: 0,
                        ascent: 0,
                        advance,
                        bearing_x: 0,
                        bearing_y: 0,
                    });
                }
            }
            _ => {} // skip unknown providers
        }
    }

    Ok((providers, glyph_map))
}

/// Resolve texture coordinates and advances from the original PNG, as BitmapProvider does.
pub fn bitmap_glyphs(
    provider: &crate::project::BitmapProvider,
    image: &image::RgbaImage,
) -> Result<crate::project::GlyphMap, String> {
    let columns = provider.chars.first().map_or(0, |row| row.chars().count());
    if columns == 0
        || provider
            .chars
            .iter()
            .any(|row| row.chars().count() != columns)
    {
        return Err("Bitmap font requires a nonempty rectangular character grid".into());
    }
    let width = image.width() / columns as u32;
    let height = image.height() / provider.chars.len() as u32;
    if width == 0 || height == 0 || provider.height == 0 || provider.ascent > provider.height as i32
    {
        return Err("Invalid bitmap font cell dimensions or ascent".into());
    }
    let bearing_y = 7_i32
        .checked_sub(provider.ascent)
        .ok_or("Bitmap font ascent is out of range")?;
    let scale = provider.height as f32 / height as f32;
    let mut glyphs = crate::project::GlyphMap::new();
    for (row, chars) in provider.chars.iter().enumerate() {
        for (column, ch) in chars.chars().enumerate() {
            if ch == '\0' {
                continue;
            }
            let x = column as u32 * width;
            let y = row as u32 * height;
            let ink_width = (0..width)
                .rev()
                .find(|dx| (0..height).any(|dy| image.get_pixel(x + dx, y + dy)[3] != 0))
                .map_or(0, |dx| dx + 1);
            glyphs.insert(
                ch,
                crate::project::GlyphInfo {
                    x,
                    y,
                    width,
                    height,
                    ascent: provider.ascent,
                    advance: (ink_width as f32 * scale + 0.5) as u32 + 1,
                    bearing_x: 0,
                    bearing_y,
                },
            );
        }
    }
    Ok(glyphs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_metrics_use_source_cells_alpha_and_display_scale() {
        let (providers, _) = parse_font_json(
            r#"{"providers":[{
            "type":"bitmap", "file":"test.png", "height":8, "ascent":7,
            "chars":["iW", "😀\u0000"]
        }]}"#,
        )
        .unwrap();
        let mut image = image::RgbaImage::new(32, 32);
        // RGB in transparent pixels must not contribute to the advance.
        image.put_pixel(15, 0, image::Rgba([255, 255, 255, 0]));
        image.put_pixel(1, 3, image::Rgba([255, 255, 255, 255]));
        image.put_pixel(26, 5, image::Rgba([255, 255, 255, 1]));
        image.put_pixel(4, 20, image::Rgba([255, 255, 255, 255]));
        let glyphs = bitmap_glyphs(&providers[0], &image).unwrap();
        assert_eq!(
            (
                glyphs[&'i'].width,
                glyphs[&'i'].height,
                glyphs[&'i'].advance
            ),
            (16, 16, 2)
        );
        assert_eq!((glyphs[&'W'].x, glyphs[&'W'].advance), (16, 7));
        assert_eq!((glyphs[&'😀'].y, glyphs[&'😀'].advance), (16, 4));
        assert!(!glyphs.contains_key(&'\0'));
    }

    #[test]
    fn bitmap_rejects_ragged_grid() {
        let (providers, _) = parse_font_json(
            r#"{"providers":[{
            "type":"bitmap", "chars":["AB", "C"]
        }]}"#,
        )
        .unwrap();
        assert!(bitmap_glyphs(&providers[0], &image::RgbaImage::new(16, 16)).is_err());
    }

    #[test]
    fn parse_bitmap_provider() {
        let json = r#"{
            "providers": [{
                "type": "bitmap",
                "file": "minecraft:font/ascii.png",
                "ascent": 7,
                "chars": ["ABCD", "EFGH"]
            }]
        }"#;

        let (providers, glyph_map) = parse_font_json(json).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].file, "minecraft:font/ascii.png");
        assert!(glyph_map.is_empty()); // Bitmap metrics require the decoded image.
    }

    #[test]
    fn parse_space_provider() {
        let json = r#"{
            "providers": [{
                "type": "space",
                "chars": [" "]
            }]
        }"#;

        let (providers, glyph_map) = parse_font_json(json).unwrap();
        assert!(providers.is_empty());
        assert!(glyph_map.contains_key(&' '));
    }

    #[test]
    fn parse_reference_is_skipped() {
        let json = r#"{
            "providers": [{
                "type": "reference",
                "id": "minecraft:include/default"
            }]
        }"#;

        let (providers, glyph_map) = parse_font_json(json).unwrap();
        assert!(providers.is_empty());
        assert!(glyph_map.is_empty());
    }
}
