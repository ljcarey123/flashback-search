use tauri::State;

use crate::{db, secrets};

use super::AppState;

#[tauri::command]
pub async fn save_settings(gemini_api_key: Option<String>) -> Result<(), String> {
    if let Some(key) = gemini_api_key {
        secrets::set_gemini_key(&key).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let has_gemini_key = secrets::get_gemini_key()
        .map(|k| k.is_some())
        .unwrap_or(false);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let client_id = db::get_setting(&db, "client_id").map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "has_gemini_key": has_gemini_key,
        "client_id": client_id,
    }))
}
