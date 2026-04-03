use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::{db, integrations::gemini, secrets};

use super::{enrich, thumb_path, AppState, SearchResult};

#[tauri::command]
pub async fn search(
    query: String,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let gemini_api_key = secrets::get_gemini_key()
        .map_err(|e| e.to_string())?
        .ok_or("Gemini API key not set")?;

    let query_vec = gemini::embed_text(&state.http, &gemini_api_key, &query)
        .await
        .map_err(|e| e.to_string())?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let results = db::semantic_search(&db, &query_vec, limit).map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .map(|(photo, score)| {
            let mut p = photo;
            p.thumb_path = Some(
                thumb_path(&state.data_dir, &p.id)
                    .to_string_lossy()
                    .into_owned(),
            );
            SearchResult { photo: p, score }
        })
        .collect())
}

#[tauri::command]
pub async fn get_library(state: State<'_, AppState>) -> Result<Vec<db::Photo>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let photos = db::get_all_photos(&db).map_err(|e| e.to_string())?;
    Ok(enrich(photos, &state.data_dir))
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let total: i64 = db
        .query_row("SELECT COUNT(*) FROM photos", [], |r| r.get(0))
        .unwrap_or(0);
    let indexed: i64 = db
        .query_row("SELECT COUNT(*) FROM photos WHERE indexed=1", [], |r| r.get(0))
        .unwrap_or(0);
    let videos: i64 = db
        .query_row("SELECT COUNT(*) FROM photos WHERE is_video=1", [], |r| r.get(0))
        .unwrap_or(0);
    Ok(serde_json::json!({
        "total": total,
        "indexed": indexed,
        "videos": videos,
        "photos": total - videos,
    }))
}

#[tauri::command]
pub async fn get_db_path(app: AppHandle) -> Result<String, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("flashback.db");
    Ok(path.to_string_lossy().into_owned())
}

/// Copy a photo from its local path to `Pictures\Flashback`.
#[tauri::command]
pub async fn download_photo(
    photo_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (local_path, filename) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db
            .prepare("SELECT local_path, filename FROM photos WHERE id=?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row(rusqlite::params![photo_id], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
    };

    let local_path = local_path.ok_or("This photo has no local file (video or pending import)")?;

    let dest_dir = dirs_next::picture_dir()
        .unwrap_or_else(|| PathBuf::from("C:\\Users\\Public\\Pictures"))
        .join("Flashback");
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let dest = dest_dir.join(&filename);
    std::fs::copy(&local_path, &dest).map_err(|e| e.to_string())?;

    Ok(dest.to_string_lossy().into_owned())
}
