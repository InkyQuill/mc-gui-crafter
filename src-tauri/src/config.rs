use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_MCP_PORT: u16 = 47_381;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AppConfig {
    pub mcp_port: Option<u16>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mcp_port: Some(DEFAULT_MCP_PORT),
        }
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
    serde_json::from_str(&json).map_err(|error| format!("Failed to parse app config: {error}"))
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
}
