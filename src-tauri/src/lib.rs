mod animation;
mod commands;
mod export;
mod font;
mod format;
#[allow(dead_code)]
mod mcp;
mod project;
mod templates;
mod texture;

use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub sessions: Mutex<project::ProjectSessionManager>,
    pub mcp_handle: Mutex<Option<mcp::McpServerHandle>>,
    pub app_handle: Mutex<Option<tauri::AppHandle>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            app.manage(AppState {
                sessions: Mutex::new(project::ProjectSessionManager::default()),
                mcp_handle: Mutex::new(None),
                app_handle: Mutex::new(Some(app.handle().clone())),
            });
            let state = app.state::<AppState>();
            let handle = mcp::start_web_server(app.handle().clone())
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            *state.mcp_handle.lock().unwrap() = Some(handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::template_list,
            commands::asset_import,
            commands::asset_update,
            commands::asset_list,
            commands::asset_remove,
            commands::asset_get_data_url,
            commands::project_export_preview,
            commands::project_export,
            commands::element_add,
            commands::element_move,
            commands::element_update,
            commands::element_resize,
            commands::element_reorder,
            commands::element_remove,
            commands::element_list,
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
