mod commands;
mod db;
mod gemini;
mod google;
pub(crate) mod secrets;
mod takeout;

use commands::AppState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir");
            std::fs::create_dir_all(&data_dir)?;

            let db_path = data_dir.join("flashback.db");
            let conn = db::open(db_path.to_str().unwrap()).expect("Failed to open database");
            db::migrate(&conn).expect("Database migration failed");

            let http = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client");

            app.manage(AppState {
                db: Mutex::new(conn),
                http,
                data_dir,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_auth_flow,
            commands::get_auth_status,
            commands::sign_out,
            commands::import_takeout,
            commands::run_picker_import,
            commands::reset_index,
            commands::index_next_batch,
            commands::search,
            commands::get_library,
            commands::get_stats,
            commands::download_photo,
            commands::save_settings,
            commands::load_settings,
            commands::get_db_path,
            commands::debug_token,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Flashback");
}
