use crate::project::{AssetMetadata, NineSlice, NineSliceMode};

pub const MINECRAFT_GUI_PANEL: &str = "textures/minecraft/gui_panel.png";
pub const MINECRAFT_SLOT: &str = "textures/minecraft/slot.png";
pub const MINECRAFT_BUTTON: &str = "textures/minecraft/button.png";
pub const MINECRAFT_BUTTON_DISABLED: &str = "textures/minecraft/button_disabled.png";
pub const MINECRAFT_BUTTON_HIGHLIGHTED: &str = "textures/minecraft/button_highlighted.png";
pub const MINECRAFT_BURN_BACK: &str = "textures/minecraft/burn_back.png";
pub const MINECRAFT_BURN_PROGRESS: &str = "textures/minecraft/burn_progress.png";
pub const MINECRAFT_LIT_PROGRESS: &str = "textures/minecraft/lit_progress.png";
pub const MINECRAFT_PROGRESS_ARROW_BACK: &str = "textures/minecraft/progress_arrow_back.png";
pub const MINECRAFT_SCROLLER: &str = "textures/minecraft/scroller.png";
pub const MINECRAFT_SCROLLER_BACKGROUND: &str = "textures/minecraft/scroller_background.png";

#[derive(Debug, Clone)]
pub struct BundledTextureAsset {
    pub path: &'static str,
    pub bytes: &'static [u8],
    pub metadata: AssetMetadata,
}

fn plain_metadata(width: u32, height: u32) -> AssetMetadata {
    AssetMetadata {
        width: Some(width),
        height: Some(height),
        nine_slice: None,
    }
}

pub fn nine_slice(left: u32, right: u32, top: u32, bottom: u32) -> NineSlice {
    NineSlice {
        left,
        right,
        top,
        bottom,
        edge_mode: NineSliceMode::Tile,
        center_mode: NineSliceMode::Tile,
    }
}

fn nine_slice_metadata(width: u32, height: u32, border: u32) -> AssetMetadata {
    AssetMetadata {
        width: Some(width),
        height: Some(height),
        nine_slice: Some(nine_slice(border, border, border, border)),
    }
}

pub fn minecraft_default_assets() -> Vec<BundledTextureAsset> {
    vec![
        BundledTextureAsset {
            path: MINECRAFT_GUI_PANEL,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/gui_panel.png"),
            metadata: nine_slice_metadata(25, 25, 4),
        },
        BundledTextureAsset {
            path: MINECRAFT_SLOT,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/slot.png"),
            metadata: nine_slice_metadata(18, 18, 1),
        },
        BundledTextureAsset {
            path: MINECRAFT_BUTTON,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/button.png"),
            metadata: nine_slice_metadata(200, 20, 3),
        },
        BundledTextureAsset {
            path: MINECRAFT_BUTTON_DISABLED,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/button_disabled.png"),
            metadata: nine_slice_metadata(200, 20, 1),
        },
        BundledTextureAsset {
            path: MINECRAFT_BUTTON_HIGHLIGHTED,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/button_highlighted.png"),
            metadata: nine_slice_metadata(200, 20, 3),
        },
        BundledTextureAsset {
            path: MINECRAFT_BURN_BACK,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/burn_back.png"),
            metadata: plain_metadata(14, 14),
        },
        BundledTextureAsset {
            path: MINECRAFT_BURN_PROGRESS,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/burn_progress.png"),
            metadata: plain_metadata(24, 16),
        },
        BundledTextureAsset {
            path: MINECRAFT_LIT_PROGRESS,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/lit_progress.png"),
            metadata: plain_metadata(14, 14),
        },
        BundledTextureAsset {
            path: MINECRAFT_PROGRESS_ARROW_BACK,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/progress_arrow_back.png"),
            metadata: plain_metadata(24, 16),
        },
        BundledTextureAsset {
            path: MINECRAFT_SCROLLER,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/scroller.png"),
            metadata: nine_slice_metadata(6, 32, 1),
        },
        BundledTextureAsset {
            path: MINECRAFT_SCROLLER_BACKGROUND,
            bytes: include_bytes!("../bundled/texture_packs/minecraft/scroller_background.png"),
            metadata: nine_slice_metadata(6, 32, 1),
        },
    ]
}
