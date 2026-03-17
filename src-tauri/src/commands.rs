use std::sync::Mutex;

use reqwest::Client;
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{db, gemini, google, secrets};

// ── App State ─────────────────────────────────────────────────────────────────

pub struct AppState {
    pub db: Mutex<Connection>,
    pub http: Client,
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SearchResult {
    pub photo: db::Photo,
    pub score: f32,
}

// ── OAuth ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_auth_url(
    client_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let redirect_uri = "urn:ietf:wg:oauth:2.0:oob";
    let url = google::auth_url(&client_id, redirect_uri);
    // client_id is not sensitive — store in SQLite for UI convenience
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::set_setting(&db, "client_id", &client_id).map_err(|e| e.to_string())?;
    Ok(url)
}

#[tauri::command]
pub async fn exchange_auth_code(
    client_id: String,
    client_secret: String,
    code: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let redirect_uri = "urn:ietf:wg:oauth:2.0:oob";
    let tokens =
        google::exchange_code(&state.http, &client_id, &client_secret, redirect_uri, &code)
            .await
            .map_err(|e| e.to_string())?;

    // Secrets go to the OS keychain — never SQLite
    secrets::set_access_token(&tokens.access_token).map_err(|e| e.to_string())?;
    if let Some(rt) = &tokens.refresh_token {
        secrets::set_refresh_token(rt).map_err(|e| e.to_string())?;
    }
    secrets::set_client_secret(&client_secret).map_err(|e| e.to_string())?;

    // Non-sensitive: keep in SQLite so the UI can read them without keychain access
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::set_setting(&db, "client_id", &client_id).map_err(|e| e.to_string())?;
    }

    let profile = google::get_user_profile(&state.http, &tokens.access_token)
        .await
        .map_err(|e| e.to_string())?;

    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::set_setting(&db, "user_name", &profile.name).map_err(|e| e.to_string())?;
    }

    Ok(profile.name)
}

#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let has_token = secrets::get_access_token()
        .map(|t| t.is_some())
        .unwrap_or(false);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let user_name = db::get_setting(&db, "user_name").map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "authenticated": has_token,
        "user_name": user_name,
    }))
}

#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>) -> Result<(), String> {
    secrets::clear_auth().map_err(|e| e.to_string())?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM settings WHERE key='user_name'", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Sync ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn sync_library(
    app: AppHandle,
    state: State<'_, AppState>,
    // Maximum number of pages to fetch (each page = up to 100 photos). 0 = unlimited.
    max_pages: Option<usize>,
) -> Result<usize, String> {
    let access_token = secrets::get_access_token()
        .map_err(|e| e.to_string())?
        .ok_or("Not authenticated")?;

    let limit = max_pages.unwrap_or(0);
    let mut page_token: Option<String> = None;
    let mut total_fetched = 0usize;
    let mut pages_fetched = 0usize;

    loop {
        let (photos, next) =
            google::list_media_page(&state.http, &access_token, page_token.as_deref())
                .await
                .map_err(|e| e.to_string())?;

        let count = photos.len();
        {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db::upsert_photos(&db, &photos).map_err(|e| e.to_string())?;
        }
        total_fetched += count;
        pages_fetched += 1;

        app.emit("sync-progress", serde_json::json!({ "fetched": total_fetched })).ok();

        let page_limit_reached = limit > 0 && pages_fetched >= limit;
        if next.is_none() || count == 0 || page_limit_reached {
            break;
        }
        page_token = next;
    }

    Ok(total_fetched)
}

// ── Indexing ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn index_next_batch(
    batch_size: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    secrets::get_access_token()
        .map_err(|e| e.to_string())?
        .ok_or("Not authenticated")?;

    let gemini_api_key = secrets::get_gemini_key()
        .map_err(|e| e.to_string())?
        .ok_or("Gemini API key not set")?;

    let unindexed: Vec<db::Photo> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::get_unindexed_photos(&db, batch_size).map_err(|e| e.to_string())?
    };

    let count = unindexed.len();

    for photo in unindexed {
        let base_url = match &photo.base_url {
            Some(u) => u.clone(),
            None => continue,
        };

        let thumb = match google::download_thumbnail(&state.http, &base_url).await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Thumbnail error for {}: {e}", photo.id);
                continue;
            }
        };

        let vector = match gemini::embed_image(&state.http, &gemini_api_key, &thumb).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Embed error for {}: {e}", photo.id);
                continue;
            }
        };

        {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db::save_embedding(&db, &photo.id, &vector).map_err(|e| e.to_string())?;
        }

        app.emit("index-progress", serde_json::json!({ "photo_id": photo.id })).ok();
    }

    Ok(count)
}

// ── Search ────────────────────────────────────────────────────────────────────

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
        .map(|(photo, score)| SearchResult { photo, score })
        .collect())
}

// ── Library ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_library(state: State<'_, AppState>) -> Result<Vec<db::Photo>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::get_all_photos(&db).map_err(|e| e.to_string())
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
    use tauri::Manager;
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("flashback.db");
    Ok(path.to_string_lossy().into_owned())
}

// ── Download ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn download_photo(
    photo_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (base_url, filename) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db
            .prepare("SELECT base_url, filename FROM photos WHERE id=?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row(rusqlite::params![photo_id], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
    };

    let base_url = base_url.ok_or("No base URL for this photo")?;
    let bytes = google::download_original(&state.http, &base_url)
        .await
        .map_err(|e| e.to_string())?;

    let pictures_dir = dirs_next::picture_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Users\\Public\\Pictures"));
    let dest_dir = pictures_dir.join("Flashback");
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let dest = dest_dir.join(&filename);
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;

    Ok(dest.to_string_lossy().into_owned())
}

// ── Debug ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn debug_token(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let token = secrets::get_access_token()
        .map_err(|e| e.to_string())?
        .ok_or("No access token in keychain")?;

    // Show first/last 6 chars so the user can confirm which token is active
    let token_preview = if token.len() > 12 {
        format!("{}…{}", &token[..6], &token[token.len() - 6..])
    } else {
        "too short".to_string()
    };

    // Call Google's tokeninfo endpoint — returns scopes, expiry, email
    let resp = state
        .http
        .get("https://www.googleapis.com/oauth2/v1/tokeninfo")
        .query(&[("access_token", &token)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "token_preview": token_preview,
        "tokeninfo_status": status,
        "tokeninfo": body,
    }))
}

#[tauri::command]
pub async fn debug_photos_api(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let token = secrets::get_access_token()
        .map_err(|e| e.to_string())?
        .ok_or("No access token in keychain")?;

    let resp = state
        .http
        .get("https://photoslibrary.googleapis.com/v1/mediaItems?pageSize=1")
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    let body_text = resp.text().await.map_err(|e| e.to_string())?;
    let body: serde_json::Value =
        serde_json::from_str(&body_text).unwrap_or(serde_json::Value::String(body_text));

    Ok(serde_json::json!({
        "status": status,
        "body": body,
    }))
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn save_settings(gemini_api_key: Option<String>) -> Result<(), String> {
    if let Some(key) = gemini_api_key {
        secrets::set_gemini_key(&key).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    // Indicate key presence without exposing the value to the frontend
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
