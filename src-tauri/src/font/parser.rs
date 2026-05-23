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
                    chars: chars.clone(),
                    image_data: vec![],
                    image_width: 0,
                    image_height: 0,
                };

                // Build glyph map from character grid
                for (row, line) in chars.iter().enumerate() {
                    let column_count = line.chars().count().max(1) as u32;
                    let glyph_width = 128 / column_count;
                    for (col, ch) in line.chars().enumerate() {
                        glyph_map.insert(
                            ch,
                            crate::project::GlyphInfo {
                                x: col as u32 * glyph_width,
                                y: row as u32 * height,
                                width: glyph_width,
                                height,
                                ascent,
                                advance: glyph_width,
                                bearing_x: 0,
                                bearing_y: 0,
                            },
                        );
                    }
                }

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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(glyph_map.contains_key(&'A'));
        assert!(glyph_map.contains_key(&'H'));
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
