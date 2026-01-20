mod commands;
mod db;
mod deps;
mod error;
mod export;
mod indexer;
mod previews;
mod scanner;
mod settings;
mod state;

use state::AppState;
use tauri::Manager;
use tracing_subscriber;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_state = AppState::new(app.handle().clone())?;
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::set_project_root,
            commands::set_output_folder,
            commands::get_settings,
            commands::get_current_project,
            commands::start_scan,
            commands::get_assets,
            commands::get_asset,
            commands::get_dependencies,
            commands::get_dependents,
            commands::get_type_counts,
            commands::export_file,
            commands::export_bundle,
            commands::reveal_in_explorer,
            commands::get_material_info,
            commands::get_model_info,
            commands::get_bundle_preview,
            commands::get_thumbnail_base64,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
