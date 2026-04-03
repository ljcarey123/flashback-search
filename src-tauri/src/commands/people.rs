use tauri::{AppHandle, Emitter, State};

use crate::{db, integrations::face, services::people::PeopleService};

use super::{thumb_path, AppState, SearchResult};

// ── Face detection batch ──────────────────────────────────────────────────────

/// Run face detection on a batch of indexed photos that have not yet been processed.
/// Stores detected bounding boxes in the `faces` table and marks photos as `faces_detected`.
/// Emits `"face-detect-progress"` events: `{ done, total }`.
#[tauri::command]
pub async fn detect_faces_batch(
    batch_size: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let photos = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::get_photos_needing_face_detection(&db, batch_size).map_err(|e| e.to_string())?
    };

    let total = photos.len();
    let mut done = 0usize;

    for photo in photos {
        let thumb = thumb_path(&state.data_dir, &photo.id);
        let bytes = match std::fs::read(&thumb) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Thumbnail missing for {}: {e}", photo.id);
                let db = state.db.lock().map_err(|e| e.to_string())?;
                let _ = db::mark_faces_detected(&db, &photo.id);
                done += 1;
                continue;
            }
        };

        let bboxes = match state.face.detect(&bytes) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Face detection failed for {}: {e}", photo.id);
                vec![]
            }
        };

        {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db::delete_faces_for_photo(&db, &photo.id).map_err(|e| e.to_string())?;
            for bbox in &bboxes {
                let bbox_json = serde_json::to_string(bbox).map_err(|e| e.to_string())?;
                db::insert_face(&db, &photo.id, &bbox_json).map_err(|e| e.to_string())?;
            }
            db::mark_faces_detected(&db, &photo.id).map_err(|e| e.to_string())?;
        }

        done += 1;
        app.emit(
            "face-detect-progress",
            serde_json::json!({ "done": done, "total": total }),
        )
        .ok();
    }

    Ok(done)
}

// ── Face embedding batch ──────────────────────────────────────────────────────

/// Embed all detected-but-unembedded face crops using MobileFaceNet.
/// Emits `"face-embed-progress"` events: `{ done, total }`.
#[tauri::command]
pub async fn embed_faces_batch(
    batch_size: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let faces = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::get_unembedded_faces(&db, batch_size).map_err(|e| e.to_string())?
    };

    let total = faces.len();
    let mut done = 0usize;

    for face_row in faces {
        let thumb = thumb_path(&state.data_dir, &face_row.photo_id);
        let thumb_bytes = match std::fs::read(&thumb) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Thumbnail missing for face {}: {e}", face_row.id);
                done += 1;
                continue;
            }
        };

        let bbox: face::FaceBbox = match serde_json::from_str(&face_row.bbox_json) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Bad bbox JSON for face {}: {e}", face_row.id);
                done += 1;
                continue;
            }
        };

        let vector = match state.face.crop_and_embed(&thumb_bytes, &bbox) {
            Ok((_, v)) => v,
            Err(e) => {
                eprintln!("Crop/embed failed for face {}: {e}", face_row.id);
                done += 1;
                continue;
            }
        };

        {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db::save_face_embedding(&db, &face_row.id, &vector)
                .map_err(|e| e.to_string())?;
        }

        done += 1;
        app.emit(
            "face-embed-progress",
            serde_json::json!({ "done": done, "total": total }),
        )
        .ok();
    }

    Ok(done)
}

// ── Per-photo face detection (for the People UI) ──────────────────────────────

/// Detect faces in a single photo and return their bounding boxes.
/// Results are stored in the DB (idempotent — returns cached results if already run).
#[tauri::command]
pub async fn detect_faces_for_photo(
    photo_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<face::FaceBbox>, String> {
    // Return cached results if detection has already run for this photo
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let rows = db::get_faces_for_photo(&db, &photo_id).map_err(|e| e.to_string())?;
        if !rows.is_empty() {
            return Ok(rows
                .into_iter()
                .filter_map(|r| serde_json::from_str(&r.bbox_json).ok())
                .collect());
        }
    }

    let thumb = thumb_path(&state.data_dir, &photo_id);
    let bytes = std::fs::read(&thumb).map_err(|e| format!("Thumbnail not found: {e}"))?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    PeopleService::detect_for_photo(&state.face, &db, &photo_id, &bytes)
        .map_err(|e| e.to_string())
}

// ── People CRUD ───────────────────────────────────────────────────────────────

/// List all saved people, ordered by name.
#[tauri::command]
pub async fn list_people(state: State<'_, AppState>) -> Result<Vec<db::Person>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::list_people(&db).map_err(|e| e.to_string())
}

/// Create a new person from a face in a specific photo.
#[tauri::command]
pub async fn create_person(
    name: String,
    photo_id: String,
    bbox_json: String,
    state: State<'_, AppState>,
) -> Result<db::Person, String> {
    if name.trim().is_empty() || name.len() > 100 {
        return Err("Name must be between 1 and 100 characters".into());
    }
    let bbox: face::FaceBbox =
        serde_json::from_str(&bbox_json).map_err(|e| format!("Invalid bbox: {e}"))?;
    let thumb_bytes = std::fs::read(thumb_path(&state.data_dir, &photo_id))
        .map_err(|e| format!("Thumbnail not found: {e}"))?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    PeopleService::create_person(&state.face, &db, &name, &photo_id, &bbox, &thumb_bytes)
        .map_err(|e| e.to_string())
}

/// Add an additional face example to an existing person.
#[tauri::command]
pub async fn add_person_example(
    person_id: String,
    photo_id: String,
    bbox_json: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let bbox: face::FaceBbox =
        serde_json::from_str(&bbox_json).map_err(|e| format!("Invalid bbox: {e}"))?;
    let thumb_bytes = std::fs::read(thumb_path(&state.data_dir, &photo_id))
        .map_err(|e| format!("Thumbnail not found: {e}"))?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    PeopleService::add_example(&state.face, &db, &person_id, &bbox, &thumb_bytes)
        .map_err(|e| e.to_string())
}

/// Delete a person and all their examples.
#[tauri::command]
pub async fn delete_person(
    person_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_person(&db, &person_id).map_err(|e| e.to_string())
}

// ── Person search ─────────────────────────────────────────────────────────────

/// Find photos containing a specific person using face similarity search.
#[tauri::command]
pub async fn search_by_person(
    person_id: String,
    limit: usize,
    min_score: f32,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let anchor_vec = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::get_person_vector(&db, &person_id)
            .map_err(|e| e.to_string())?
            .ok_or("Person has no embedding — add at least one face example")?
    };

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let results = db::face_search(&db, &anchor_vec, limit).map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .filter(|(_, score)| *score >= min_score)
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

// ── Face stats ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_face_stats(state: State<'_, AppState>) -> Result<db::FaceStats, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::get_face_stats(&db).map_err(|e| e.to_string())
}

/// List all face examples for a person (for the multi-example UI).
#[tauri::command]
pub async fn list_person_examples(
    person_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<db::PersonExample>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::list_person_examples(&db, &person_id).map_err(|e| e.to_string())
}

/// Delete a single face example from a person, then recompute the centroid.
#[tauri::command]
pub async fn delete_person_example(
    example_id: String,
    person_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_person_example(&db, &example_id).map_err(|e| e.to_string())?;
    db::recompute_person_centroid(&db, &person_id).map_err(|e| e.to_string())?;
    Ok(())
}
