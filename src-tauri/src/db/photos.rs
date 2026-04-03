use anyhow::Result;
use rusqlite::{params, Connection};

use super::{cosine_similarity, row_to_photo, Photo, SELECT_COLS};

/// Bulk upsert — used by tests and legacy code.  On conflict by id, refreshes
/// base_url and description only (preserves indexed flag and local data).
#[cfg_attr(not(test), allow(dead_code))]
pub fn upsert_photos(conn: &Connection, photos: &[Photo]) -> Result<()> {
    let mut stmt = conn.prepare_cached(&format!(
        "INSERT INTO photos ({SELECT_COLS})
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
             base_url    = excluded.base_url,
             description = excluded.description,
             local_path  = COALESCE(excluded.local_path, photos.local_path)"
    ))?;
    for p in photos {
        stmt.execute(params![
            p.id,
            p.filename,
            p.description,
            p.created_at,
            p.width,
            p.height,
            p.base_url,
            p.mime_type,
            p.is_video as i32,
            p.indexed as i32,
            p.local_path,
            p.fingerprint,
        ])?;
    }
    Ok(())
}

/// Insert a new photo only if its fingerprint is not already in the DB.
/// Returns `true` if the photo was inserted, `false` if it was a duplicate.
pub fn insert_photo_if_new(conn: &Connection, photo: &Photo) -> Result<bool> {
    if let Some(fp) = &photo.fingerprint {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM photos WHERE fingerprint = ?1",
                params![fp],
                |r| r.get::<_, i32>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);
        if exists {
            return Ok(false);
        }
    }

    conn.execute(
        &format!(
            "INSERT INTO photos ({SELECT_COLS}) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11)"
        ),
        params![
            photo.id,
            photo.filename,
            photo.description,
            photo.created_at,
            photo.width,
            photo.height,
            photo.base_url,
            photo.mime_type,
            photo.is_video as i32,
            photo.local_path,
            photo.fingerprint,
        ],
    )?;
    Ok(true)
}

pub fn reset_index(conn: &Connection) -> Result<()> {
    conn.execute_batch("DELETE FROM embeddings; UPDATE photos SET indexed = 0;")?;
    Ok(())
}

pub fn update_description(conn: &Connection, photo_id: &str, description: &str) -> Result<()> {
    conn.execute(
        "UPDATE photos SET description=?1 WHERE id=?2",
        params![description, photo_id],
    )?;
    Ok(())
}

pub fn save_embedding(conn: &Connection, photo_id: &str, vector: &[f32]) -> Result<()> {
    let json = serde_json::to_string(vector)?;
    conn.execute(
        "INSERT INTO embeddings(photo_id, vector_json) VALUES(?1,?2)
         ON CONFLICT(photo_id) DO UPDATE SET vector_json=excluded.vector_json",
        params![photo_id, json],
    )?;
    conn.execute("UPDATE photos SET indexed=1 WHERE id=?1", params![photo_id])?;
    Ok(())
}

/// Photos ready to embed: not yet indexed, not a video, and have a local file.
pub fn get_unindexed_photos(conn: &Connection, limit: usize) -> Result<Vec<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM photos \
         WHERE indexed=0 AND is_video=0 AND local_path IS NOT NULL \
         LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([limit as i64], row_to_photo)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn get_all_photos(conn: &Connection) -> Result<Vec<Photo>> {
    let sql = format!("SELECT {SELECT_COLS} FROM photos ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], row_to_photo)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn find_by_fingerprint(conn: &Connection, fingerprint: &str) -> Result<Option<Photo>> {
    let sql = format!("SELECT {SELECT_COLS} FROM photos WHERE fingerprint = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let result = stmt.query_row(params![fingerprint], row_to_photo);
    match result {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn semantic_search(
    conn: &Connection,
    query_vec: &[f32],
    limit: usize,
) -> Result<Vec<(Photo, f32)>> {
    let sql = format!(
        "SELECT p.{cols}, e.vector_json
         FROM photos p JOIN embeddings e ON p.id = e.photo_id
         WHERE p.is_video = 0",
        cols = SELECT_COLS
            .split(", ")
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", p.")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut results: Vec<(Photo, f32)> = stmt
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
            let score = cosine_similarity(query_vec, &vec);
            (photo, score)
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(limit);
    Ok(results)
}
