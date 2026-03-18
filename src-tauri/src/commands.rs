use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use image::imageops::FilterType;
use reqwest::Client;
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{db, gemini, google, secrets, takeout};

// ── App State ─────────────────────────────────────────────────────────────────

pub struct AppState {
    pub db: Mutex<Connection>,
    pub http: Client,
    /// App data directory — used to compute thumbnail and photo storage paths.
    pub data_dir: PathBuf,
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SearchResult {
    pub photo: db::Photo,
    pub score: f32,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn thumb_path(data_dir: &Path, photo_id: &str) -> PathBuf {
    data_dir
        .join("thumbnails")
        .join(photo_id)
        .with_extension("jpg")
}

fn photos_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("photos")
}

/// Attach the computed `thumb_path` to a list of photos (in-place).
fn enrich(mut photos: Vec<db::Photo>, data_dir: &Path) -> Vec<db::Photo> {
    for p in &mut photos {
        p.thumb_path = Some(
            thumb_path(data_dir, &p.id)
                .to_string_lossy()
                .into_owned(),
        );
    }
    photos
}

// ── Thumbnail generation ──────────────────────────────────────────────────────

/// Decode `bytes`, resize to at most 512×512, encode as JPEG.
/// Returns `None` if the bytes cannot be decoded (e.g. HEIC).
fn make_thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let thumb = img.resize(512, 512, FilterType::Triangle);
    let mut out = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .ok()?;
    Some(out)
}

// ── OAuth ─────────────────────────────────────────────────────────────────────

/// Combined auth flow: starts a localhost redirect server, opens the browser,
/// waits up to 2 minutes for the user to sign in, then exchanges the code and
/// stores the tokens.  Returns the user's display name.
#[tauri::command]
pub async fn start_auth_flow(
    client_id: String,
    client_secret: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // 1. Bind a random localhost port for the OAuth redirect
    let (port, code_rx) = google::start_oauth_server()
        .await
        .map_err(|e| e.to_string())?;
    let redirect_uri = format!("http://127.0.0.1:{port}");

    // 2. Persist client_id (non-sensitive) for display
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::set_setting(&db, "client_id", &client_id).map_err(|e| e.to_string())?;
    }

    // 3. Open the consent URL in the default browser
    let url = google::auth_url(&client_id, &redirect_uri);
    open::that(&url).map_err(|e| format!("Failed to open browser: {e}"))?;

    // 4. Wait for the redirect (2-minute timeout)
    let code = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        code_rx,
    )
    .await
    .map_err(|_| "Sign-in timed out after 2 minutes. Please try again.")?
    .map_err(|_| "OAuth server closed unexpectedly")?;

    // 5. Exchange code → tokens
    let tokens = google::exchange_code(
        &state.http,
        &client_id,
        &client_secret,
        &redirect_uri,
        &code,
    )
    .await
    .map_err(|e| e.to_string())?;

    secrets::set_access_token(&tokens.access_token).map_err(|e| e.to_string())?;
    if let Some(rt) = &tokens.refresh_token {
        secrets::set_refresh_token(rt).map_err(|e| e.to_string())?;
    }
    secrets::set_client_secret(&client_secret).map_err(|e| e.to_string())?;

    // 6. Fetch user profile
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

// ── Takeout import ─────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct ImportSummary {
    pub added: usize,
    pub skipped: usize,
    pub errors: usize,
}

/// Import photos from an unzipped Google Takeout folder.
///
/// Emits `"import-progress"` events: `{ done, total, added, skipped }`.
#[tauri::command]
pub async fn import_takeout(
    folder_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ImportSummary, String> {
    let folder = PathBuf::from(&folder_path);
    let thumbs_dir = state.data_dir.join("thumbnails");
    std::fs::create_dir_all(&thumbs_dir).map_err(|e| e.to_string())?;

    // Scan the folder (fast — no image decoding yet)
    let entries = takeout::scan_folder(&folder);
    let total = entries.len();
    let mut added = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    app.emit(
        "import-progress",
        serde_json::json!({ "done": 0, "total": total, "added": 0, "skipped": 0 }),
    )
    .ok();

    for (i, entry) in entries.iter().enumerate() {
        // Dedup by fingerprint
        if let Some(ref fp) = entry.fingerprint {
            let already_exists = {
                let db = state.db.lock().map_err(|e| e.to_string())?;
                db::find_by_fingerprint(&db, fp)
                    .map(|r| r.is_some())
                    .unwrap_or(false)
            };
            if already_exists {
                skipped += 1;
                app.emit(
                    "import-progress",
                    serde_json::json!({
                        "done": i + 1, "total": total,
                        "added": added, "skipped": skipped
                    }),
                )
                .ok();
                continue;
            }
        }

        let photo_id = Uuid::new_v4().to_string();
        let thumb_dest = thumb_path(&state.data_dir, &photo_id);
        let local_path = entry.path.to_string_lossy().into_owned();

        let (width, height) = if entry.is_video {
            // Videos: no thumbnail, just record metadata
            (None, None)
        } else {
            // Try to generate a thumbnail
            match std::fs::read(&entry.path) {
                Ok(bytes) => match make_thumbnail(&bytes) {
                    Some(thumb_bytes) => {
                        if let Err(e) = std::fs::write(&thumb_dest, &thumb_bytes) {
                            eprintln!("Thumbnail write error for {}: {e}", entry.path.display());
                            errors += 1;
                            // Don't insert to DB if thumbnail write failed
                            app.emit(
                                "import-progress",
                                serde_json::json!({
                                    "done": i + 1, "total": total,
                                    "added": added, "skipped": skipped
                                }),
                            )
                            .ok();
                            continue;
                        }
                        // Get dimensions from the thumbnail we just created
                        let thumb_img = image::load_from_memory(&thumb_bytes).ok();
                        let dims = thumb_img.map(|img| {
                            let orig =
                                image::load_from_memory(&bytes).ok().map(|o| (o.width(), o.height()));
                            orig.unwrap_or_else(|| (img.width(), img.height()))
                        });
                        dims.map(|(w, h)| (Some(w as i64), Some(h as i64)))
                            .unwrap_or((None, None))
                    }
                    None => {
                        // Undecodable (e.g. HEIC) — skip
                        eprintln!(
                            "Cannot decode image (unsupported format?): {}",
                            entry.path.display()
                        );
                        errors += 1;
                        app.emit(
                            "import-progress",
                            serde_json::json!({
                                "done": i + 1, "total": total,
                                "added": added, "skipped": skipped
                            }),
                        )
                        .ok();
                        continue;
                    }
                },
                Err(e) => {
                    eprintln!("File read error for {}: {e}", entry.path.display());
                    errors += 1;
                    app.emit(
                        "import-progress",
                        serde_json::json!({
                            "done": i + 1, "total": total,
                            "added": added, "skipped": skipped
                        }),
                    )
                    .ok();
                    continue;
                }
            }
        };

        let photo = db::Photo {
            id: photo_id,
            filename: entry.filename.clone(),
            description: entry.description.clone(),
            created_at: entry.created_at.clone(),
            width,
            height,
            base_url: None,
            mime_type: if entry.is_video {
                Some("video/mp4".to_string())
            } else {
                Some("image/jpeg".to_string())
            },
            is_video: entry.is_video,
            indexed: false,
            local_path: Some(local_path),
            fingerprint: entry.fingerprint.clone(),
            thumb_path: None,
        };

        let inserted = {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db::insert_photo_if_new(&db, &photo).map_err(|e| e.to_string())?
        };

        if inserted {
            added += 1;
        } else {
            // Fingerprint appeared between our check and insert — clean up thumbnail
            let _ = std::fs::remove_file(&thumb_dest);
            skipped += 1;
        }

        app.emit(
            "import-progress",
            serde_json::json!({
                "done": i + 1, "total": total,
                "added": added, "skipped": skipped
            }),
        )
        .ok();
    }

    Ok(ImportSummary {
        added,
        skipped,
        errors,
    })
}

// ── Picker import ──────────────────────────────────────────────────────────────

/// Open the Google Photos Picker, wait for the user to select photos, then
/// download and store them locally.
///
/// Emits `"picker-status"` events:
/// - `{ status: "waiting" }` — picker is open in the browser
/// - `{ status: "downloading", total: N }` — user selected, downloading
/// - `{ status: "done", added: N, skipped: M }` — complete
#[tauri::command]
pub async fn run_picker_import(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ImportSummary, String> {
    let access_token = secrets::get_access_token()
        .map_err(|e| e.to_string())?
        .ok_or("Not signed in to Google. Please sign in first.")?;

    let thumbs_dir = state.data_dir.join("thumbnails");
    let orig_dir = photos_dir(&state.data_dir);
    std::fs::create_dir_all(&thumbs_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&orig_dir).map_err(|e| e.to_string())?;

    // 1. Create Picker session
    let session = google::create_picker_session(&state.http, &access_token)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Open picker in browser
    open::that(&session.picker_uri)
        .map_err(|e| format!("Cannot open browser: {e}"))?;
    app.emit("picker-status", serde_json::json!({ "status": "waiting" })).ok();

    // 3. Poll until selection is complete (10 min timeout, 3 s interval)
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(600);
    loop {
        if tokio::time::Instant::now() >= deadline {
            let _ = google::delete_picker_session(&state.http, &access_token, &session.id).await;
            return Err("Picker timed out after 10 minutes".to_string());
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        match google::poll_picker_session(&state.http, &access_token, &session.id).await {
            Ok(true) => break,
            Ok(false) => {}
            Err(e) => {
                eprintln!("Picker poll error: {e}");
                // Keep polling — transient network error
            }
        }
    }

    // 4. List selected items
    let items = google::list_picker_items(&state.http, &access_token, &session.id)
        .await
        .map_err(|e| e.to_string())?;

    // Cleanup session (best-effort)
    let _ = google::delete_picker_session(&state.http, &access_token, &session.id).await;

    let total = items.len();
    app.emit(
        "picker-status",
        serde_json::json!({ "status": "downloading", "total": total }),
    )
    .ok();

    let mut added = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    for item in items {
        let create_unix = item
            .create_time
            .as_deref()
            .and_then(google::iso_to_unix);
        let fingerprint = create_unix
            .as_ref()
            .map(|ts| format!("{ts}_{}", item.filename));

        // Dedup check
        if let Some(ref fp) = fingerprint {
            let exists = {
                let db = state.db.lock().map_err(|e| e.to_string())?;
                db::find_by_fingerprint(&db, fp)
                    .map(|r| r.is_some())
                    .unwrap_or(false)
            };
            if exists {
                skipped += 1;
                continue;
            }
        }

        let photo_id = Uuid::new_v4().to_string();
        let thumb_dest = thumb_path(&state.data_dir, &photo_id);
        let orig_dest = orig_dir.join(&item.filename);

        if item.is_video {
            // Videos: insert metadata only, no download
            let photo = db::Photo {
                id: photo_id.clone(),
                filename: item.filename.clone(),
                description: None,
                created_at: item.create_time.clone(),
                width: item.width,
                height: item.height,
                base_url: Some(item.base_url.clone()),
                mime_type: item.mime_type.clone(),
                is_video: true,
                indexed: false,
                local_path: None,
                fingerprint: fingerprint.clone(),
                thumb_path: None,
            };
            let inserted = {
                let db = state.db.lock().map_err(|e| e.to_string())?;
                db::insert_photo_if_new(&db, &photo).map_err(|e| e.to_string())?
            };
            if inserted { added += 1; } else { skipped += 1; }
            app.emit(
                "picker-status",
                serde_json::json!({ "status": "downloading", "done": added + skipped, "total": total }),
            )
            .ok();
            continue;
        }

        // Download thumbnail for display + embedding
        let thumb_url = format!("{}=w512-h512", item.base_url);
        let thumb_bytes = match google::download_bytes(&state.http, &thumb_url, &access_token).await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Picker thumb download failed for {}: {e}", item.id);
                errors += 1;
                continue;
            }
        };

        if let Err(e) = std::fs::write(&thumb_dest, &thumb_bytes) {
            eprintln!("Picker thumb write failed: {e}");
            errors += 1;
            continue;
        }

        // Download full-resolution original
        let orig_url = format!("{}=d", item.base_url);
        let orig_bytes = match google::download_bytes(&state.http, &orig_url, &access_token).await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Picker original download failed for {}: {e}", item.id);
                errors += 1;
                let _ = std::fs::remove_file(&thumb_dest);
                continue;
            }
        };
        if let Err(e) = std::fs::write(&orig_dest, &orig_bytes) {
            eprintln!("Picker original write failed: {e}");
            errors += 1;
            let _ = std::fs::remove_file(&thumb_dest);
            continue;
        }

        let photo = db::Photo {
            id: photo_id,
            filename: item.filename.clone(),
            description: None,
            created_at: item.create_time,
            width: item.width,
            height: item.height,
            base_url: Some(item.base_url),
            mime_type: item.mime_type,
            is_video: false,
            indexed: false,
            local_path: Some(orig_dest.to_string_lossy().into_owned()),
            fingerprint,
            thumb_path: None,
        };

        let inserted = {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db::insert_photo_if_new(&db, &photo).map_err(|e| e.to_string())?
        };
        if inserted {
            added += 1;
        } else {
            let _ = std::fs::remove_file(&thumb_dest);
            let _ = std::fs::remove_file(&orig_dest);
            skipped += 1;
        }

        app.emit(
            "picker-status",
            serde_json::json!({
                "status": "downloading",
                "done": added + skipped + errors,
                "total": total
            }),
        )
        .ok();
    }

    app.emit(
        "picker-status",
        serde_json::json!({ "status": "done", "added": added, "skipped": skipped }),
    )
    .ok();

    Ok(ImportSummary { added, skipped, errors })
}

// ── Indexing ──────────────────────────────────────────────────────────────────

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

// ── Library ───────────────────────────────────────────────────────────────────

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
    use tauri::Manager;
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("flashback.db");
    Ok(path.to_string_lossy().into_owned())
}

// ── Download ──────────────────────────────────────────────────────────────────

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

// ── Debug ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn debug_token(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let token = secrets::get_access_token()
        .map_err(|e| e.to_string())?
        .ok_or("No access token in keychain")?;

    let token_preview = if token.len() > 12 {
        format!("{}…{}", &token[..6], &token[token.len() - 6..])
    } else {
        "too short".to_string()
    };

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
