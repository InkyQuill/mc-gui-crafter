pub mod parser;
pub mod rasterizer;

use crate::project::{FontAsset, FontSource, GlyphMap};

/// Load the bundled Minecraft default font.
pub fn load_bundled_font() -> FontAsset {
    let default_json = include_str!("../../bundled/minecraft/font/default.json");
    let include_default_json = include_str!("../../bundled/minecraft/font/include/default.json");
    let space_json = include_str!("../../bundled/minecraft/font/include/space.json");

    let mut all_providers = Vec::new();
    let mut all_glyph_map = GlyphMap::new();

    // Parse each font file and merge
    for json_str in &[default_json, include_default_json, space_json] {
        if let Ok((providers, glyph_map)) = parser::parse_font_json(json_str) {
            all_providers.extend(providers);
            all_glyph_map.extend(glyph_map);
        }
    }

    // Load bundled PNG data for bitmap providers
    for provider in &mut all_providers {
        let png_data = match provider.file.as_str() {
            "minecraft:font/ascii.png" => {
                include_bytes!("../../bundled/minecraft/textures/font/ascii.png").to_vec()
            }
            "minecraft:font/accented.png" => {
                include_bytes!("../../bundled/minecraft/textures/font/accented.png").to_vec()
            }
            "minecraft:font/nonlatin_european.png" => {
                include_bytes!("../../bundled/minecraft/textures/font/nonlatin_european.png")
                    .to_vec()
            }
            "minecraft:font/ascii_sga.png" => {
                include_bytes!("../../bundled/minecraft/textures/font/ascii_sga.png").to_vec()
            }
            "minecraft:font/asciillager.png" => {
                include_bytes!("../../bundled/minecraft/textures/font/asciillager.png").to_vec()
            }
            _ => continue,
        };

        if let Ok(img) = image::load_from_memory(&png_data) {
            provider.image_width = img.width();
            provider.image_height = img.height();
        }
        provider.image_data = png_data;
    }

    FontAsset {
        id: "minecraft:default".to_string(),
        source: FontSource::Minecraft {
            providers: all_providers,
            glyph_map: all_glyph_map,
        },
    }
}
