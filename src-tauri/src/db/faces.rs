use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;

use super::{cosine_similarity, row_to_photo, Face, Photo, SELECT_COLS};

// ── Face stats ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct FaceStats {
    pub photos_pending_detection: i64,
    pub faces_detected: i64,
    pub faces_embedded: i64,
}

pub fn get_face_stats(conn: &Connection) -> Result<FaceStats> {
    let photos_pending: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM photos WHERE indexed=1 AND is_video=0 AND faces_detected=0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let faces_detected: i64 = conn
        .query_row("SELECT COUNT(*) FROM faces", [], |r| r.get(0))
        .unwrap_or(0);
    let faces_embedded: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM faces WHERE vector_json IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(FaceStats {
        photos_pending_detection: photos_pending,
        faces_detected,
        faces_embedded,
    })
}

// ── Face CRUD ─────────────────────────────────────────────────────────────────

pub fn insert_face(conn: &Connection, photo_id: &str, bbox_json: &str) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO faces(id, photo_id, bbox_json) VALUES(?1, ?2, ?3)",
        params![id, photo_id, bbox_json],
    )?;
    Ok(id)
}

pub fn save_face_embedding(conn: &Connection, face_id: &str, vector: &[f32]) -> Result<()> {
    let json = serde_json::to_string(vector)?;
    conn.execute(
        "UPDATE faces SET vector_json=?1 WHERE id=?2",
        params![json, face_id],
    )?;
    Ok(())
}

pub fn get_faces_for_photo(conn: &Connection, photo_id: &str) -> Result<Vec<Face>> {
    let mut stmt = conn.prepare(
        "SELECT id, photo_id, bbox_json, vector_json FROM faces WHERE photo_id=?1",
    )?;
    let rows = stmt
        .query_map(params![photo_id], |row| {
            Ok(Face {
                id: row.get(0)?,
                photo_id: row.get(1)?,
                bbox_json: row.get(2)?,
                vector_json: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn delete_faces_for_photo(conn: &Connection, photo_id: &str) -> Result<()> {
    conn.execute("DELETE FROM faces WHERE photo_id=?1", params![photo_id])?;
    Ok(())
}

/// Photos that are indexed, not a video, and haven't had face detection run yet.
pub fn get_photos_needing_face_detection(conn: &Connection, limit: usize) -> Result<Vec<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM photos \
         WHERE indexed=1 AND is_video=0 AND faces_detected=0 \
         LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([limit as i64], row_to_photo)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn mark_faces_detected(conn: &Connection, photo_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE photos SET faces_detected=1 WHERE id=?1",
        params![photo_id],
    )?;
    Ok(())
}

/// Faces that have been detected but not yet embedded.
pub fn get_unembedded_faces(conn: &Connection, limit: usize) -> Result<Vec<Face>> {
    let mut stmt = conn.prepare(
        "SELECT id, photo_id, bbox_json, vector_json FROM faces \
         WHERE vector_json IS NULL LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(Face {
                id: row.get(0)?,
                photo_id: row.get(1)?,
                bbox_json: row.get(2)?,
                vector_json: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Cosine similarity search over face embeddings. Returns one result per photo,
/// using the highest-scoring face match per photo.
pub fn face_search(
    conn: &Connection,
    anchor_vec: &[f32],
    limit: usize,
) -> Result<Vec<(Photo, f32)>> {
    let sql = format!(
        "SELECT p.{cols}, f.vector_json
         FROM faces f JOIN photos p ON p.id = f.photo_id
         WHERE f.vector_json IS NOT NULL AND p.is_video = 0",
        cols = SELECT_COLS
            .split(", ")
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", p.")
    );
    let mut stmt = conn.prepare(&sql)?;

    let all_pairs: Vec<(Photo, f32)> = stmt
        .query_map([], |row| {
            let photo = row_to_photo(row)?;
            let vec_json: String = row.get(12)?;
            Ok((photo, vec_json))
        })?
        .filter_map(|r| r.ok())
        .filter_map(|(photo, json)| {
            let vec: Vec<f32> = serde_json::from_str(&json).ok()?;
            Some((photo, vec))
        })
        .map(|(photo, vec)| {
            let score = cosine_similarity(anchor_vec, &vec);
            (photo, score)
        })
        .collect();

    // Keep best score per photo
    let mut best: HashMap<String, (Photo, f32)> = HashMap::new();
    for (photo, score) in all_pairs {
        let entry = best
            .entry(photo.id.clone())
            .or_insert((photo.clone(), f32::NEG_INFINITY));
        if score > entry.1 {
            *entry = (photo, score);
        }
    }

    let mut results: Vec<(Photo, f32)> = best.into_values().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(limit);
    Ok(results)
}
