use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

use crate::{format, AppState};

pub fn project_path_from_args(args: &[String], cwd: impl AsRef<Path>) -> Option<PathBuf> {
    args.iter().skip(1).find_map(|arg| {
        let path = PathBuf::from(arg);
        let is_project = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mcgui"));
        if !is_project {
            return None;
        }
        if path.is_absolute() {
            Some(path)
        } else {
            Some(cwd.as_ref().join(path))
        }
    })
}

pub fn open_project_path(app: &AppHandle, path: impl AsRef<Path>) -> Result<(), String> {
    let path = path.as_ref();
    let project = format::load_from_mcgui(&path.to_string_lossy())?;
    let state = app.state::<AppState>();
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.create_session(project);
    }
    emit_project_opened(app, path);
    Ok(())
}

pub fn open_project_from_args(app: &AppHandle, args: &[String], cwd: impl AsRef<Path>) {
    if let Some(path) = project_path_from_args(args, cwd) {
        if let Err(error) = open_project_path(app, &path) {
            let _ = app.emit(
                "project-open-failed",
                serde_json::json!({
                    "source": "startup",
                    "path": path,
                    "error": error,
                }),
            );
        }
    }
}

pub fn handle_second_instance(app: &AppHandle, args: Vec<String>, cwd: String) {
    open_project_from_args(app, &args, cwd);
    focus_main_window(app);
}

pub fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn emit_project_opened(app: &AppHandle, path: &Path) {
    let _ = app.emit(
        "project-changed",
        serde_json::json!({
            "source": "startup",
            "tool": "project_open",
            "path": path,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn project_path_from_args_ignores_executable_and_picks_mcgui_file() {
        let args = vec![
            "/usr/bin/mc-gui-crafter".to_string(),
            "/tmp/example.mcgui".to_string(),
        ];

        assert_eq!(
            project_path_from_args(&args, "/home/user"),
            Some(PathBuf::from("/tmp/example.mcgui"))
        );
    }

    #[test]
    fn project_path_from_args_resolves_relative_path_against_cwd() {
        let args = vec![
            "mc-gui-crafter".to_string(),
            "projects/machine.mcgui".to_string(),
        ];

        assert_eq!(
            project_path_from_args(&args, "/home/user"),
            Some(PathBuf::from("/home/user/projects/machine.mcgui"))
        );
    }

    #[test]
    fn project_path_from_args_returns_none_without_mcgui_file() {
        let args = vec!["mc-gui-crafter".to_string(), "--help".to_string()];

        assert_eq!(project_path_from_args(&args, "/home/user"), None);
    }
}
