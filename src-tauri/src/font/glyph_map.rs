use crate::project::{GlyphInfo, GlyphMap};
use std::collections::HashMap;

/// Build a GlyphMap from a list of BitmapProviders.
pub fn build_glyph_map(providers: &[crate::project::BitmapProvider]) -> GlyphMap {
    let mut map = GlyphMap::new();

    for (_provider_idx, provider) in providers.iter().enumerate() {
        for (row, line) in provider.chars.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                let glyph_w = 8u32;
                let glyph_h = 8u32;
                map.insert(
                    ch,
                    GlyphInfo {
                        x: col as u32 * glyph_w,
                        y: row as u32 * glyph_h,
                        width: glyph_w,
                        height: glyph_h,
                        ascent: provider.ascent,
                        advance: glyph_w,
                        bearing_x: 0,
                        bearing_y: 0,
                    },
                );
            }
        }
    }

    map
}

/// Look up a character in the glyph map, returning a fallback if missing.
pub fn lookup_glyph(map: &GlyphMap, ch: char) -> GlyphInfo {
    map.get(&ch).cloned().unwrap_or_else(|| {
        map.get(&'\u{FFFD}').cloned().unwrap_or(GlyphInfo {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
            ascent: 0,
            advance: 8,
            bearing_x: 0,
            bearing_y: 0,
        })
    })
}
