use image::{Rgba, RgbaImage};
use std::collections::HashMap;

pub fn generate_background(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0xC6, 0xC6, 0xC6, 0xFF]));
    for x in 0..width {
        for y in 0..height {
            if x < 4 || x >= width - 4 || y < 4 || y >= height - 4 {
                img.put_pixel(x, y, Rgba([0x55, 0x55, 0x55, 0xFF]));
            }
        }
    }
    for x in 0..width {
        for y in 0..height {
            if (x < 8 && y >= 4 && y < height - 4) || (y < 8 && x >= 4 && x < width - 4) {
                img.put_pixel(x, y, Rgba([0x00, 0x00, 0x00, 0xFF]));
            }
        }
    }
    img
}

pub fn generate_slot_frame() -> RgbaImage {
    let size: u32 = 18;
    let mut img = RgbaImage::from_pixel(size, size, Rgba([0x8B, 0x8B, 0x8B, 0xFF]));
    for i in 0..size {
        img.put_pixel(0, i, Rgba([0x37, 0x37, 0x37, 0xFF]));
        img.put_pixel(size - 1, i, Rgba([0x37, 0x37, 0x37, 0xFF]));
        img.put_pixel(i, 0, Rgba([0x37, 0x37, 0x37, 0xFF]));
        img.put_pixel(i, size - 1, Rgba([0x37, 0x37, 0x37, 0xFF]));
    }
    for i in 0..size - 2 {
        img.put_pixel(1, i + 1, Rgba([0xFF, 0xFF, 0xFF, 0x4D]));
        img.put_pixel(i + 1, 1, Rgba([0xFF, 0xFF, 0xFF, 0x4D]));
    }
    for i in 0..size - 2 {
        img.put_pixel(size - 2, i + 1, Rgba([0x00, 0x00, 0x00, 0x66]));
        img.put_pixel(i + 1, size - 2, Rgba([0x00, 0x00, 0x00, 0x66]));
    }
    img
}

pub fn generate_progress_arrow() -> RgbaImage {
    let mut img = RgbaImage::from_pixel(22, 16, Rgba([0x00, 0x00, 0x00, 0x00]));
    for x in 2..20 {
        for y in 5..11 {
            img.put_pixel(x, y, Rgba([0xE9, 0xA2, 0x3B, 0xFF]));
        }
    }
    for d in 0..5 {
        for y in (8 - d)..(8 + d) {
            img.put_pixel(20 - d, y, Rgba([0xE9, 0xA2, 0x3B, 0xFF]));
        }
    }
    img
}

pub fn generate_flame() -> RgbaImage {
    let mut img = RgbaImage::from_pixel(14, 14, Rgba([0x00, 0x00, 0x00, 0x00]));
    let flame_pixels: [(u32, u32, Rgba<u8>); 30] = [
        (6, 1, Rgba([0xFF, 0xDD, 0x00, 0xFF])),
        (5, 2, Rgba([0xFF, 0xCC, 0x00, 0xFF])),
        (6, 2, Rgba([0xFF, 0xDD, 0x00, 0xFF])),
        (7, 2, Rgba([0xFF, 0xDD, 0x00, 0xFF])),
        (4, 3, Rgba([0xFF, 0xAA, 0x00, 0xFF])),
        (5, 3, Rgba([0xFF, 0xBB, 0x00, 0xFF])),
        (6, 3, Rgba([0xFF, 0xCC, 0x00, 0xFF])),
        (7, 3, Rgba([0xFF, 0xCC, 0x00, 0xFF])),
        (8, 3, Rgba([0xFF, 0xBB, 0x00, 0xFF])),
        (4, 4, Rgba([0xFF, 0x88, 0x00, 0xFF])),
        (5, 4, Rgba([0xFF, 0x99, 0x00, 0xFF])),
        (6, 4, Rgba([0xFF, 0xBB, 0x00, 0xFF])),
        (7, 4, Rgba([0xFF, 0xBB, 0x00, 0xFF])),
        (8, 4, Rgba([0xFF, 0x99, 0x00, 0xFF])),
        (4, 5, Rgba([0xFF, 0x66, 0x00, 0xFF])),
        (5, 5, Rgba([0xFF, 0x88, 0x00, 0xFF])),
        (6, 5, Rgba([0xFF, 0x99, 0x00, 0xFF])),
        (7, 5, Rgba([0xFF, 0x99, 0x00, 0xFF])),
        (8, 5, Rgba([0xFF, 0x88, 0x00, 0xFF])),
        (4, 6, Rgba([0xFF, 0x55, 0x00, 0xFF])),
        (5, 6, Rgba([0xFF, 0x77, 0x00, 0xFF])),
        (6, 6, Rgba([0xFF, 0x88, 0x00, 0xFF])),
        (7, 6, Rgba([0xFF, 0x88, 0x00, 0xFF])),
        (5, 7, Rgba([0xFF, 0x55, 0x00, 0xFF])),
        (6, 7, Rgba([0xFF, 0x66, 0x00, 0xFF])),
        (4, 8, Rgba([0xFF, 0x44, 0x00, 0xFF])),
        (5, 8, Rgba([0xFF, 0x55, 0x00, 0xFF])),
        (6, 8, Rgba([0xFF, 0x55, 0x00, 0xFF])),
        (5, 9, Rgba([0xFF, 0x33, 0x00, 0xFF])),
        (6, 9, Rgba([0xFF, 0x44, 0x00, 0xFF])),
    ];
    for (x, y, color) in &flame_pixels {
        if *x < 14 && *y < 14 {
            img.put_pixel(*x, *y, *color);
        }
    }
    img
}

pub fn generate_fluid_frame() -> RgbaImage {
    let w: u32 = 20;
    let h: u32 = 64;
    let mut img = RgbaImage::from_pixel(w, h, Rgba([0x00, 0x00, 0x00, 0x00]));
    for i in 0..w {
        img.put_pixel(i, 0, Rgba([0x2C, 0x2C, 0x2C, 0xFF]));
        img.put_pixel(i, h - 1, Rgba([0x2C, 0x2C, 0x2C, 0xFF]));
    }
    for i in 0..h {
        img.put_pixel(0, i, Rgba([0x2C, 0x2C, 0x2C, 0xFF]));
        img.put_pixel(w - 1, i, Rgba([0x2C, 0x2C, 0x2C, 0xFF]));
    }
    img
}

pub fn generate_energy_frame() -> RgbaImage {
    let w: u32 = 14;
    let h: u32 = 48;
    let mut img = RgbaImage::from_pixel(w, h, Rgba([0x00, 0x00, 0x00, 0x00]));
    for i in 0..w {
        img.put_pixel(i, 0, Rgba([0x2C, 0x2C, 0x2C, 0xFF]));
        img.put_pixel(i, h - 1, Rgba([0x2C, 0x2C, 0x2C, 0xFF]));
    }
    for i in 0..h {
        img.put_pixel(0, i, Rgba([0x2C, 0x2C, 0x2C, 0xFF]));
        img.put_pixel(w - 1, i, Rgba([0x2C, 0x2C, 0x2C, 0xFF]));
    }
    img
}

fn image_to_png(img: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("Failed to encode generated texture");
    buf
}

pub fn generate_all_defaults(gui_width: u32, gui_height: u32) -> HashMap<String, Vec<u8>> {
    let mut map = HashMap::new();
    map.insert(
        "textures/background.png".to_string(),
        image_to_png(&generate_background(gui_width, gui_height)),
    );
    map.insert(
        "textures/slot_frame.png".to_string(),
        image_to_png(&generate_slot_frame()),
    );
    map.insert(
        "textures/progress_arrow.png".to_string(),
        image_to_png(&generate_progress_arrow()),
    );
    map.insert(
        "textures/flame.png".to_string(),
        image_to_png(&generate_flame()),
    );
    map.insert(
        "textures/fluid_frame.png".to_string(),
        image_to_png(&generate_fluid_frame()),
    );
    map.insert(
        "textures/energy_frame.png".to_string(),
        image_to_png(&generate_energy_frame()),
    );
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_has_correct_size() {
        let img = generate_background(176, 166);
        assert_eq!(img.width(), 176);
        assert_eq!(img.height(), 166);
    }

    #[test]
    fn background_corners_are_border_not_fill() {
        let img = generate_background(176, 166);
        assert_eq!(img.get_pixel(0, 0).0, [0x55, 0x55, 0x55, 0xFF]);
        assert_eq!(img.get_pixel(175, 0).0, [0x55, 0x55, 0x55, 0xFF]);
    }

    #[test]
    fn background_center_is_fill() {
        let img = generate_background(176, 166);
        assert_eq!(img.get_pixel(88, 83).0, [0xC6, 0xC6, 0xC6, 0xFF]);
    }

    #[test]
    fn slot_frame_is_18x18() {
        let img = generate_slot_frame();
        assert_eq!(img.width(), 18);
        assert_eq!(img.height(), 18);
    }

    #[test]
    fn generate_all_defaults_contains_six_textures() {
        let map = generate_all_defaults(176, 166);
        assert!(map.contains_key("textures/background.png"));
        assert!(map.contains_key("textures/slot_frame.png"));
        assert!(map.contains_key("textures/progress_arrow.png"));
        assert!(map.contains_key("textures/flame.png"));
        assert!(map.contains_key("textures/fluid_frame.png"));
        assert!(map.contains_key("textures/energy_frame.png"));
    }
}
