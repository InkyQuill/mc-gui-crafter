mod animation;
mod commands;
mod config;
mod export;
mod font;
mod format;
#[allow(dead_code)]
mod mcp;
mod project;
mod startup;
mod templates;
mod texture;

use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub sessions: Mutex<project::ProjectSessionManager>,
    pub mcp_handle: Mutex<Option<mcp::McpServerHandle>>,
    pub app_handle: Mutex<Option<tauri::AppHandle>>,
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
            app.manage(AppState {
                sessions: Mutex::new(project::ProjectSessionManager::default()),
                mcp_handle: Mutex::new(None),
                app_handle: Mutex::new(Some(app.handle().clone())),
            });
            let state = app.state::<AppState>();
            let mut app_config = config::load().map_err(Box::<dyn std::error::Error>::from)?;
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
            commands::project_export_settings_update,
            commands::project_semantic_groups_update,
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
            commands::element_move_many,
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
