use tauri::{AppHandle, Emitter, State};

use crate::{db, integrations::gemini, secrets};

use super::{thumb_path, AppState};

/// Clear all embeddings and reset every photo to unindexed so they are picked
/// up again by the next `index_next_batch` call.
#[tauri::command]
pub fn reset_index(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::reset_index(&db).map_err(|e| e.to_string())
}

/// Embed the next batch of unindexed photos with Gemini.
///
/// No Google Photos access token required — reads thumbnails from disk.
#[tauri::command]
pub async fn index_next_batch(
    batch_size: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let gemini_api_key = secrets::get_gemini_key()
        .map_err(|e| e.to_string())?
        .ok_or("Gemini API key not set")?;

    let unindexed: Vec<db::Photo> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::get_unindexed_photos(&db, batch_size).map_err(|e| e.to_string())?
    };

    let count = unindexed.len();

    for photo in unindexed {
        let thumb = thumb_path(&state.data_dir, &photo.id);
        let bytes = match std::fs::read(&thumb) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Thumbnail missing for {}: {e}", photo.id);
                continue;
            }
        };

        // Generate description and embedding concurrently
        let (description_result, vector_result) = tokio::join!(
            gemini::describe_image(&state.http, &gemini_api_key, &bytes),
            gemini::embed_image(&state.http, &gemini_api_key, &bytes),
        );

        let vector = match vector_result {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Embed error for {}: {e}", photo.id);
                continue;
            }
        };

        {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db::save_embedding(&db, &photo.id, &vector).map_err(|e| e.to_string())?;
            if let Ok(desc) = description_result {
                if let Err(e) = db::update_description(&db, &photo.id, &desc) {
                    eprintln!("Description save error for {}: {e}", photo.id);
                }
            } else if let Err(e) = description_result {
                eprintln!("Description error for {}: {e}", photo.id);
            }
        }

        app.emit("index-progress", serde_json::json!({ "photo_id": photo.id }))
            .ok();
    }

    Ok(count)
}
