mod commands;
mod db;
mod integrations;
pub(crate) mod secrets;
mod services;
mod state;

#[cfg(test)]
mod tests;

use state::AppState;
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

            let resource_dir = app.path().resource_dir()
                .expect("Failed to resolve resource dir");
            let face = integrations::face::FaceEngine::load(
                &resource_dir.join("models").join("face_detect.onnx"),
                &resource_dir.join("models").join("face_embed.onnx"),
            )
            .expect("Failed to load face models");

            app.manage(AppState {
                db: Mutex::new(conn),
                http,
                data_dir,
                face,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::start_auth_flow,
            commands::auth::get_auth_status,
            commands::auth::sign_out,
            commands::import::import_takeout,
            commands::import::run_picker_import,
            commands::index::reset_index,
            commands::index::index_next_batch,
            commands::library::search,
            commands::library::get_library,
            commands::library::get_stats,
            commands::library::download_photo,
            commands::settings::save_settings,
            commands::settings::load_settings,
            commands::library::get_db_path,
            commands::debug::debug_token,
            commands::people::detect_faces_batch,
            commands::people::embed_faces_batch,
            commands::people::detect_faces_for_photo,
            commands::people::list_people,
            commands::people::create_person,
            commands::people::add_person_example,
            commands::people::delete_person,
            commands::people::search_by_person,
            commands::people::get_face_stats,
            commands::people::list_person_examples,
            commands::people::delete_person_example,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Flashback");
}
