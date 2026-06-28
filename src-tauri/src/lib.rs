mod animation;
mod commands;
mod config;
mod export;
mod font;
mod format;
#[allow(dead_code)]
mod mcp;
mod project;
mod session_log;
mod startup;
mod templates;
mod texture;
mod texture_pack;

use std::sync::Mutex;
use tauri::{Manager, WindowEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorkArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionState {
    Available,
    Unavailable,
    Missing,
}

pub struct AppState {
    pub sessions: Mutex<project::ProjectSessionManager>,
    pub mcp_handle: Mutex<Option<mcp::McpServerHandle>>,
    pub app_handle: Mutex<Option<tauri::AppHandle>>,
    pub session_log: Mutex<session_log::SessionLogger>,
}

pub fn configure_platform_environment() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(
            startup::handle_second_instance,
        ));
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let config_dir = config::config_dir().map_err(Box::<dyn std::error::Error>::from)?;
            let session_log = session_log::SessionLogger::new(&config_dir)
                .map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(AppState {
                sessions: Mutex::new(project::ProjectSessionManager::default()),
                mcp_handle: Mutex::new(None),
                app_handle: Mutex::new(Some(app.handle().clone())),
                session_log: Mutex::new(session_log),
            });
            let state = app.state::<AppState>();
            let mut app_config = config::load().map_err(Box::<dyn std::error::Error>::from)?;
            if let Some(window_config) = app_config.window.clone() {
                if let Some(window) = app.get_webview_window("main") {
                    restore_main_window_geometry(&window, window_config);
                }
            }
            let handle = mcp::start_web_server(app.handle().clone(), app_config.mcp_port)
                .map_err(Box::<dyn std::error::Error>::from)?;
            app_config.mcp_port = Some(handle.address().port());
            config::save(&app_config).map_err(Box::<dyn std::error::Error>::from)?;
            *state.mcp_handle.lock().unwrap() = Some(handle);

            let args = std::env::args().collect::<Vec<_>>();
            let cwd = std::env::current_dir().unwrap_or_default();
            startup::open_project_from_args(app.handle(), &args, cwd);

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if matches!(
                event,
                WindowEvent::CloseRequested { .. }
                    | WindowEvent::Resized(_)
                    | WindowEvent::Moved(_)
            ) {
                save_main_window_geometry(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_config_get,
            commands::editor_layout_save,
            commands::app_window_save,
            commands::ui_layout_reset,
            commands::session_log_append,
            commands::session_log_paths,
            commands::project_new,
            commands::project_open,
            commands::project_save,
            commands::project_save_as,
            commands::project_close,
            commands::project_set_active,
            commands::project_list_sessions,
            commands::project_get_active,
            commands::project_undo,
            commands::project_redo,
            commands::project_summary,
            commands::project_resize,
            commands::project_export_settings_update,
            commands::project_semantic_groups_update,
            commands::template_list,
            commands::asset_import,
            commands::asset_update,
            commands::asset_list,
            commands::asset_metadata_update,
            commands::asset_remove,
            commands::asset_get_data_url,
            commands::texture_pack_load,
            commands::project_export_preview,
            commands::project_export,
            commands::element_add,
            commands::element_move,
            commands::element_move_many,
            commands::element_update,
            commands::element_update_many,
            commands::element_resize,
            commands::element_reorder,
            commands::element_remove,
            commands::element_list,
            commands::attached_region_create,
            commands::attached_region_update,
            commands::attached_region_remove,
            commands::attached_region_list,
            commands::attached_region_move_with_elements,
            commands::state_list,
            commands::state_add,
            commands::state_update,
            commands::state_remove,
            commands::state_set_active,
            commands::state_override_update,
            commands::state_override_clear,
            commands::group_create,
            commands::group_ungroup,
            commands::group_list,
            commands::animation_create,
            commands::animation_update,
            commands::animation_remove,
            commands::animation_bind,
            commands::animation_unbind,
            mcp::mcp_status,
            commands::list_minecraft_sources,
            commands::font_import,
            commands::font_list,
            commands::font_glyph_map,
            commands::font_render_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCGUI Crafter");
}

fn save_main_window_geometry(window: &tauri::Window) {
    let Ok(size) = window.inner_size() else {
        return;
    };
    let position = window.outer_position().ok();
    if let Ok(mut config) = crate::config::load() {
        let window_config = crate::config::WindowConfig {
            width: size.width,
            height: size.height,
            x: position.as_ref().map(|position| position.x),
            y: position.as_ref().map(|position| position.y),
        }
        .clamped();
        config.window = Some(sanitize_window_config_for_window(window, window_config).0);
        let _ = crate::config::save(&config);
    }
}

fn restore_main_window_geometry(
    window: &tauri::WebviewWindow,
    window_config: crate::config::WindowConfig,
) {
    let (window_config, position_state) =
        sanitize_window_config_for_webview_window(window, window_config);
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
        width: window_config.width,
        height: window_config.height,
    }));
    match (position_state, window_config.x.zip(window_config.y)) {
        (PositionState::Available, Some((x, y))) => {
            let _ =
                window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        }
        _ => {
            let _ = window.center();
        }
    }
}

fn sanitize_window_config_for_webview_window(
    window: &tauri::WebviewWindow,
    window_config: crate::config::WindowConfig,
) -> (crate::config::WindowConfig, PositionState) {
    let work_areas = window
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|monitor| work_area_from_monitor(&monitor))
        .collect::<Vec<_>>();
    let fallback = window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| work_area_from_monitor(&monitor))
        .or_else(|| work_areas.first().copied());
    sanitize_window_config_for_work_areas(window_config, &work_areas, fallback)
}

fn sanitize_window_config_for_window(
    window: &tauri::Window,
    window_config: crate::config::WindowConfig,
) -> (crate::config::WindowConfig, PositionState) {
    let work_areas = window
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|monitor| work_area_from_monitor(&monitor))
        .collect::<Vec<_>>();
    let fallback = window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| work_area_from_monitor(&monitor))
        .or_else(|| work_areas.first().copied());
    sanitize_window_config_for_work_areas(window_config, &work_areas, fallback)
}

fn work_area_from_monitor(monitor: &tauri::Monitor) -> WorkArea {
    let work_area = monitor.work_area();
    WorkArea {
        x: work_area.position.x,
        y: work_area.position.y,
        width: work_area.size.width,
        height: work_area.size.height,
    }
}

fn sanitize_window_config_for_work_areas(
    mut window_config: crate::config::WindowConfig,
    work_areas: &[WorkArea],
    fallback: Option<WorkArea>,
) -> (crate::config::WindowConfig, PositionState) {
    let saved_position = window_config.x.zip(window_config.y);
    let saved_area = saved_position
        .and_then(|(x, y)| work_areas.iter().copied().find(|area| area.contains(x, y)));
    let area = saved_area.or(fallback);
    let position_state = match (saved_position, saved_area) {
        (None, _) => PositionState::Missing,
        (Some(_), Some(_)) => PositionState::Available,
        (Some(_), None) => PositionState::Unavailable,
    };

    if let Some(area) = area {
        window_config.width = clamp_window_dimension(
            window_config.width,
            crate::config::MIN_WINDOW_WIDTH,
            area.width,
        );
        window_config.height = clamp_window_dimension(
            window_config.height,
            crate::config::MIN_WINDOW_HEIGHT,
            area.height,
        );
        if position_state == PositionState::Available {
            window_config.x = window_config
                .x
                .map(|x| clamp_window_axis(x, window_config.width, area.x, area.width));
            window_config.y = window_config
                .y
                .map(|y| clamp_window_axis(y, window_config.height, area.y, area.height));
        } else {
            window_config.x = None;
            window_config.y = None;
        }
    }

    (window_config, position_state)
}

impl WorkArea {
    fn contains(self, x: i32, y: i32) -> bool {
        let right = i64::from(self.x) + i64::from(self.width);
        let bottom = i64::from(self.y) + i64::from(self.height);
        i64::from(x) >= i64::from(self.x)
            && i64::from(x) < right
            && i64::from(y) >= i64::from(self.y)
            && i64::from(y) < bottom
    }
}

fn clamp_window_dimension(value: u32, minimum: u32, available: u32) -> u32 {
    if available == 0 {
        return value.max(1);
    }
    if available < minimum {
        available.max(1)
    } else {
        value.clamp(minimum, available)
    }
}

fn clamp_window_axis(position: i32, size: u32, origin: i32, available: u32) -> i32 {
    let min = i64::from(origin);
    let max = min + i64::from(available).saturating_sub(i64::from(size));
    let clamped = if max < min {
        min
    } else {
        i64::from(position).clamp(min, max)
    };
    clamped.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_config(
        width: u32,
        height: u32,
        x: Option<i32>,
        y: Option<i32>,
    ) -> crate::config::WindowConfig {
        crate::config::WindowConfig {
            width,
            height,
            x,
            y,
        }
    }

    #[test]
    fn window_geometry_clamps_to_available_work_area() {
        let (config, position_state) = sanitize_window_config_for_work_areas(
            window_config(5000, 3000, Some(100), Some(100)),
            &[WorkArea {
                x: 0,
                y: 0,
                width: 1920,
                height: 1040,
            }],
            None,
        );

        assert_eq!(position_state, PositionState::Available);
        assert_eq!((config.width, config.height), (1920, 1040));
        assert_eq!((config.x, config.y), (Some(0), Some(0)));
    }

    #[test]
    fn window_geometry_drops_position_on_unavailable_display() {
        let (config, position_state) = sanitize_window_config_for_work_areas(
            window_config(1280, 800, Some(3000), Some(100)),
            &[WorkArea {
                x: 0,
                y: 0,
                width: 1920,
                height: 1040,
            }],
            Some(WorkArea {
                x: 0,
                y: 0,
                width: 1920,
                height: 1040,
            }),
        );

        assert_eq!(position_state, PositionState::Unavailable);
        assert_eq!((config.width, config.height), (1280, 800));
        assert_eq!((config.x, config.y), (None, None));
    }
}
