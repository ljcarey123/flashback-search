mod commands;
mod db;
mod face;
mod gemini;
mod google;
pub(crate) mod secrets;
mod takeout;

#[cfg(test)]
mod tests;

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

            // Face models are bundled as Tauri resources under src-tauri/models/.
            let resource_dir = app.path().resource_dir()
                .expect("Failed to resolve resource dir");
            let face_detect_model = resource_dir.join("models").join("face_detect.onnx");
            let face_embed_model = resource_dir.join("models").join("face_embed.onnx");

            app.manage(AppState {
                db: Mutex::new(conn),
                http,
                data_dir,
                face_detect_model,
                face_embed_model,
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
            commands::detect_faces_batch,
            commands::embed_faces_batch,
            commands::detect_faces_for_photo,
            commands::list_people,
            commands::create_person,
            commands::add_person_example,
            commands::delete_person,
            commands::search_by_person,
            commands::get_face_stats,
            commands::list_person_examples,
            commands::delete_person_example,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Flashback");
}
