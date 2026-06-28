use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_DETAIL_STRING: usize = 2_000;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionLogEntry {
    pub level: SessionLogLevel,
    pub source: String,
    pub category: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

pub struct SessionLogger {
    path: PathBuf,
}

impl SessionLogger {
    pub fn new(config_dir: &Path) -> Result<Self, String> {
        let dir = config_dir.join("logs");
        fs::create_dir_all(&dir)
            .map_err(|error| format!("Failed to create session log directory: {error}"))?;
        let path = dir.join(format!("session-{}.jsonl", timestamp_millis()));
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, entry: SessionLogEntry) -> Result<(), String> {
        let line = json!({
            "timestamp_ms": timestamp_millis(),
            "level": entry.level,
            "source": sanitize_label(&entry.source),
            "category": sanitize_label(&entry.category),
            "message": truncate(entry.message, MAX_DETAIL_STRING),
            "details": entry.details.map(compact_value),
        });
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| format!("Failed to open session log: {error}"))?;
        serde_json::to_writer(&mut file, &line)
            .map_err(|error| format!("Failed to encode session log entry: {error}"))?;
        file.write_all(b"\n")
            .map_err(|error| format!("Failed to write session log entry: {error}"))
    }
}

pub fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn sanitize_label(value: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .collect::<String>();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        truncate(sanitized, 80)
    }
}

fn compact_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(compact_string(text)),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .take(25)
                .map(compact_value)
                .collect::<Vec<_>>(),
        ),
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .into_iter()
                .take(80)
                .map(|(key, value)| (truncate(key, 120), compact_value(value)))
                .collect(),
        ),
        other => other,
    }
}

fn compact_string(text: String) -> String {
    if text.starts_with("data:image/") {
        let media_type = text
            .split_once(';')
            .map(|(prefix, _)| prefix)
            .unwrap_or("data:image");
        return format!("[redacted {media_type} data url, {} chars]", text.len());
    }
    truncate(text, MAX_DETAIL_STRING)
}

fn truncate(mut value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value;
    }
    value = value.chars().take(max_chars).collect();
    value.push_str("...");
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_logger_writes_jsonl() {
        let dir = std::env::temp_dir().join(format!(
            "mc-gui-crafter-session-log-test-{}",
            timestamp_millis()
        ));
        let mut logger = SessionLogger::new(&dir).unwrap();
        logger
            .append(SessionLogEntry {
                level: SessionLogLevel::Warning,
                source: "test".to_string(),
                category: "export".to_string(),
                message: "warning text".to_string(),
                details: Some(json!({ "warnings": ["a"] })),
            })
            .unwrap();

        let content = fs::read_to_string(logger.path()).unwrap();
        assert!(content.contains("\"level\":\"warning\""));
        assert!(content.contains("warning text"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn session_logger_redacts_image_data_urls() {
        let dir = std::env::temp_dir().join(format!(
            "mc-gui-crafter-session-log-redaction-test-{}",
            timestamp_millis()
        ));
        let mut logger = SessionLogger::new(&dir).unwrap();
        logger
            .append(SessionLogEntry {
                level: SessionLogLevel::Info,
                source: "test".to_string(),
                category: "feedback".to_string(),
                message: "image payload".to_string(),
                details: Some(json!({ "image": "data:image/png;base64,abcdef" })),
            })
            .unwrap();

        let content = fs::read_to_string(logger.path()).unwrap();
        assert!(content.contains("[redacted data:image/png data url"));
        assert!(!content.contains("abcdef"));

        let _ = fs::remove_dir_all(dir);
    }
}
