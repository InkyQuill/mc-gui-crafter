use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_MCP_PORT: u16 = 47_381;
pub const DEFAULT_RIGHT_DOCK_WIDTH: u32 = 520;
pub const DEFAULT_PROPERTIES_WIDTH: u32 = 300;
pub const MIN_RIGHT_DOCK_WIDTH: u32 = 360;
pub const MAX_RIGHT_DOCK_WIDTH: u32 = 900;
pub const MIN_PROPERTIES_WIDTH: u32 = 240;
pub const DEFAULT_WINDOW_WIDTH: u32 = 1280;
pub const DEFAULT_WINDOW_HEIGHT: u32 = 800;
pub const MIN_WINDOW_WIDTH: u32 = 900;
pub const MIN_WINDOW_HEIGHT: u32 = 600;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EditorLayoutConfig {
    pub version: u32,
    pub right_dock_width: u32,
    pub properties_width: u32,
    pub browser_tab: String,
}

impl Default for EditorLayoutConfig {
    fn default() -> Self {
        Self {
            version: 1,
            right_dock_width: DEFAULT_RIGHT_DOCK_WIDTH,
            properties_width: DEFAULT_PROPERTIES_WIDTH,
            browser_tab: "layers".into(),
        }
    }
}

impl EditorLayoutConfig {
    pub fn clamped(self) -> Self {
        let right_dock_width =
            if (MIN_RIGHT_DOCK_WIDTH..=MAX_RIGHT_DOCK_WIDTH).contains(&self.right_dock_width) {
                self.right_dock_width
            } else {
                DEFAULT_RIGHT_DOCK_WIDTH
            };
        let max_properties = right_dock_width
            .saturating_sub(160)
            .max(MIN_PROPERTIES_WIDTH);
        let properties_width =
            if (MIN_PROPERTIES_WIDTH..=max_properties).contains(&self.properties_width) {
                self.properties_width
            } else {
                DEFAULT_PROPERTIES_WIDTH.min(max_properties)
            };
        let browser_tab = match self.browser_tab.as_str() {
            "layers" | "assets" => self.browser_tab,
            _ => "layers".into(),
        };
        Self {
            version: 1,
            right_dock_width,
            properties_width,
            browser_tab,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WINDOW_WIDTH,
            height: DEFAULT_WINDOW_HEIGHT,
            x: None,
            y: None,
        }
    }
}

impl WindowConfig {
    pub fn clamped(self) -> Self {
        let valid_size = self.width >= MIN_WINDOW_WIDTH && self.height >= MIN_WINDOW_HEIGHT;
        let valid_position = self
            .x
            .zip(self.y)
            .is_some_and(|(x, y)| x.abs() < 20_000 && y.abs() < 20_000);
        if valid_size {
            Self {
                width: self.width,
                height: self.height,
                x: valid_position.then_some(self.x).flatten(),
                y: valid_position.then_some(self.y).flatten(),
            }
        } else {
            Self::default()
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AppConfig {
    pub mcp_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_layout: Option<EditorLayoutConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mcp_port: Some(DEFAULT_MCP_PORT),
            editor_layout: Some(EditorLayoutConfig::default()),
            window: Some(WindowConfig::default()),
        }
    }
}

impl AppConfig {
    pub fn clamped(self) -> Self {
        Self {
            mcp_port: self.mcp_port,
            editor_layout: Some(self.editor_layout.unwrap_or_default().clamped()),
            window: Some(self.window.unwrap_or_default().clamped()),
        }
    }

    pub fn with_reset_ui_layout(mut self) -> Self {
        self.editor_layout = Some(EditorLayoutConfig::default());
        self.window = Some(WindowConfig::default());
        self
    }
}

pub fn config_dir_from_home(home: &Path) -> PathBuf {
    home.join(".config").join("mc-gui-crafter")
}

pub fn config_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME is not set; cannot locate app config directory".to_string())?;
    Ok(config_dir_from_home(&home))
}

pub fn load_from_dir(config_dir: &Path) -> Result<AppConfig, String> {
    std::fs::create_dir_all(config_dir)
        .map_err(|error| format!("Failed to create app config directory: {error}"))?;
    let path = config_dir.join("config.json");
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let json = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read app config: {error}"))?;
    serde_json::from_str::<AppConfig>(&json)
        .map(AppConfig::clamped)
        .map_err(|error| format!("Failed to parse app config: {error}"))
}

pub fn load() -> Result<AppConfig, String> {
    load_from_dir(&config_dir()?)
}

pub fn save_to_dir(config_dir: &Path, config: &AppConfig) -> Result<(), String> {
    std::fs::create_dir_all(config_dir)
        .map_err(|error| format!("Failed to create app config directory: {error}"))?;
    let path = config_dir.join("config.json");
    let json = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Failed to serialize app config: {error}"))?;
    std::fs::write(&path, json).map_err(|error| format!("Failed to write app config: {error}"))
}

pub fn save(config: &AppConfig) -> Result<(), String> {
    save_to_dir(&config_dir()?, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gui-crafter-config-{name}-{}",
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn config_dir_from_home_uses_xdg_config_location() {
        let home = PathBuf::from("/home/example");

        assert_eq!(
            config_dir_from_home(&home),
            PathBuf::from("/home/example/.config/mc-gui-crafter")
        );
    }

    #[test]
    fn save_to_dir_creates_directory_and_writes_mcp_port() {
        let config_dir = temp_config_dir("save");
        let config = AppConfig {
            mcp_port: Some(49_152),
            editor_layout: None,
            window: None,
        };

        save_to_dir(&config_dir, &config).unwrap();

        let written = std::fs::read_to_string(config_dir.join("config.json")).unwrap();
        assert!(written.contains("\"mcp_port\": 49152"));
        let _ = std::fs::remove_dir_all(config_dir);
    }

    #[test]
    fn load_from_dir_returns_default_when_config_file_is_missing() {
        let config_dir = temp_config_dir("missing");

        let config = load_from_dir(&config_dir).unwrap();

        assert_eq!(config.mcp_port, Some(DEFAULT_MCP_PORT));
        assert!(config_dir.exists());
        let _ = std::fs::remove_dir_all(config_dir);
    }

    #[test]
    fn layout_config_defaults_and_clamps_invalid_values() {
        let config = AppConfig {
            mcp_port: Some(49_152),
            editor_layout: Some(EditorLayoutConfig {
                version: 1,
                right_dock_width: 9_999,
                properties_width: 10,
                browser_tab: "unknown".into(),
            }),
            window: Some(WindowConfig {
                width: 100,
                height: 100,
                x: Some(-50_000),
                y: Some(50_000),
            }),
        };

        let clamped = config.clamped();

        assert_eq!(clamped.mcp_port, Some(49_152));
        assert_eq!(
            clamped.editor_layout.as_ref().unwrap().right_dock_width,
            DEFAULT_RIGHT_DOCK_WIDTH
        );
        assert_eq!(
            clamped.editor_layout.as_ref().unwrap().properties_width,
            DEFAULT_PROPERTIES_WIDTH
        );
        assert_eq!(
            clamped.editor_layout.as_ref().unwrap().browser_tab,
            "layers"
        );
        assert_eq!(clamped.window.as_ref().unwrap().width, DEFAULT_WINDOW_WIDTH);
        assert_eq!(
            clamped.window.as_ref().unwrap().height,
            DEFAULT_WINDOW_HEIGHT
        );
        assert_eq!(clamped.window.as_ref().unwrap().x, None);
        assert_eq!(clamped.window.as_ref().unwrap().y, None);
    }

    #[test]
    fn reset_layout_preserves_mcp_port_and_clears_window_position() {
        let config = AppConfig {
            mcp_port: Some(49_152),
            editor_layout: Some(EditorLayoutConfig {
                version: 1,
                right_dock_width: 600,
                properties_width: 330,
                browser_tab: "assets".into(),
            }),
            window: Some(WindowConfig {
                width: 1440,
                height: 900,
                x: Some(200),
                y: Some(120),
            }),
        };

        let reset = config.with_reset_ui_layout();

        assert_eq!(reset.mcp_port, Some(49_152));
        assert_eq!(reset.editor_layout, Some(EditorLayoutConfig::default()));
        assert_eq!(reset.window, Some(WindowConfig::default()));
    }
}
