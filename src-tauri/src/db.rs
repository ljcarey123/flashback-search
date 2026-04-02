use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Photo {
    pub id: String,
    pub filename: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub base_url: Option<String>,
    pub mime_type: Option<String>,
    pub is_video: bool,
    pub indexed: bool,
    // New fields for local-first storage
    pub local_path: Option<String>,  // absolute path to original file on disk
    pub fingerprint: Option<String>, // dedup key: "{unix_timestamp}_{filename}"
    // Computed by command layer, never stored in DB
    pub thumb_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub anchor_photo_id: String,
    pub face_crop_base64: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Face {
    pub id: String,
    pub photo_id: String,
    pub bbox_json: String,
    pub vector_json: Option<String>,
}

pub fn open(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    // Create tables (idempotent — IF NOT EXISTS)
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS photos (
            id          TEXT PRIMARY KEY,
            filename    TEXT NOT NULL,
            description TEXT,
            created_at  TEXT,
            width       INTEGER,
            height      INTEGER,
            base_url    TEXT,
            mime_type   TEXT,
            is_video    INTEGER NOT NULL DEFAULT 0,
            indexed     INTEGER NOT NULL DEFAULT 0,
            local_path  TEXT,
            fingerprint TEXT
        );

        CREATE TABLE IF NOT EXISTS embeddings (
            photo_id    TEXT PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE,
            vector_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS people (
            id               TEXT PRIMARY KEY,
            name             TEXT NOT NULL,
            anchor_photo_id  TEXT NOT NULL REFERENCES photos(id),
            face_crop_base64 TEXT,
            vector_json      TEXT
        );

        CREATE TABLE IF NOT EXISTS person_examples (
            id               TEXT PRIMARY KEY,
            person_id        TEXT NOT NULL REFERENCES people(id) ON DELETE CASCADE,
            face_crop_base64 TEXT,
            vector_json      TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_examples_person ON person_examples(person_id);

        CREATE TABLE IF NOT EXISTS faces (
            id          TEXT PRIMARY KEY,
            photo_id    TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
            bbox_json   TEXT NOT NULL,
            vector_json TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_faces_photo ON faces(photo_id);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

    ")?;

    // Idempotent column additions for existing DBs that predate these columns.
    // Errors (e.g. "duplicate column name") are intentionally ignored.
    let _ = conn.execute("ALTER TABLE photos ADD COLUMN local_path TEXT", []);
    let _ = conn.execute("ALTER TABLE photos ADD COLUMN fingerprint TEXT", []);
    // Index must be created AFTER the column exists (matters for pre-existing DBs).
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_fingerprint \
         ON photos(fingerprint) WHERE fingerprint IS NOT NULL",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_photos_created_at ON photos(created_at DESC)",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE photos ADD COLUMN faces_detected INTEGER NOT NULL DEFAULT 0",
        [],
    );

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn row_to_photo(row: &rusqlite::Row<'_>) -> rusqlite::Result<Photo> {
    Ok(Photo {
        id: row.get(0)?,
        filename: row.get(1)?,
        description: row.get(2)?,
        created_at: row.get(3)?,
        width: row.get(4)?,
        height: row.get(5)?,
        base_url: row.get(6)?,
        mime_type: row.get(7)?,
        is_video: row.get::<_, i32>(8)? != 0,
        indexed: row.get::<_, i32>(9)? != 0,
        local_path: row.get(10)?,
        fingerprint: row.get(11)?,
        thumb_path: None, // populated by the command layer, never from DB
    })
}

const SELECT_COLS: &str =
    "id, filename, description, created_at, width, height, base_url, mime_type, \
     is_video, indexed, local_path, fingerprint";

// ── writes ────────────────────────────────────────────────────────────────────

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
    // Fingerprint-based dedup check
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
    conn.execute_batch(
        "DELETE FROM embeddings; UPDATE photos SET indexed = 0;",
    )?;
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

// ── reads ─────────────────────────────────────────────────────────────────────

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
    let sql = format!(
        "SELECT {SELECT_COLS} FROM photos ORDER BY created_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], row_to_photo)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn find_by_fingerprint(conn: &Connection, fingerprint: &str) -> Result<Option<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM photos WHERE fingerprint = ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let result = stmt.query_row(params![fingerprint], row_to_photo);
    match result {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// ── vector search ─────────────────────────────────────────────────────────────

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
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

// ── faces ─────────────────────────────────────────────────────────────────────

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

// ── people ────────────────────────────────────────────────────────────────────

pub fn insert_person(
    conn: &Connection,
    id: &str,
    name: &str,
    anchor_photo_id: &str,
    face_crop_base64: Option<&str>,
    vector_json: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO people(id, name, anchor_photo_id, face_crop_base64, vector_json) \
         VALUES(?1, ?2, ?3, ?4, ?5)",
        params![id, name, anchor_photo_id, face_crop_base64, vector_json],
    )?;
    Ok(())
}

pub fn list_people(conn: &Connection) -> Result<Vec<Person>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, anchor_photo_id, face_crop_base64 FROM people ORDER BY name",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Person {
                id: row.get(0)?,
                name: row.get(1)?,
                anchor_photo_id: row.get(2)?,
                face_crop_base64: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn get_person_vector(conn: &Connection, person_id: &str) -> Result<Option<Vec<f32>>> {
    let mut stmt = conn.prepare("SELECT vector_json FROM people WHERE id=?1")?;
    let result = stmt.query_row(params![person_id], |row| row.get::<_, Option<String>>(0));
    match result {
        Ok(Some(json)) => {
            let vec: Vec<f32> = serde_json::from_str(&json)?;
            Ok(Some(vec))
        }
        Ok(None) => Ok(None),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_person(conn: &Connection, person_id: &str) -> Result<()> {
    conn.execute("DELETE FROM people WHERE id=?1", params![person_id])?;
    Ok(())
}

// ── person examples ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersonExample {
    pub id: String,
    pub person_id: String,
    pub face_crop_base64: Option<String>,
}

pub fn insert_person_example(
    conn: &Connection,
    person_id: &str,
    face_crop_base64: Option<&str>,
    vector_json: &str,
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO person_examples(id, person_id, face_crop_base64, vector_json) \
         VALUES(?1, ?2, ?3, ?4)",
        params![id, person_id, face_crop_base64, vector_json],
    )?;
    Ok(id)
}

pub fn list_person_examples(conn: &Connection, person_id: &str) -> Result<Vec<PersonExample>> {
    let mut stmt = conn.prepare(
        "SELECT id, person_id, face_crop_base64 FROM person_examples WHERE person_id=?1",
    )?;
    let rows = stmt
        .query_map(params![person_id], |row| {
            Ok(PersonExample {
                id: row.get(0)?,
                person_id: row.get(1)?,
                face_crop_base64: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn delete_person_example(conn: &Connection, example_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM person_examples WHERE id=?1",
        params![example_id],
    )?;
    Ok(())
}

/// Recompute the centroid of all examples for a person and store it in people.vector_json.
/// Returns the new centroid, or None if the person has no examples.
pub fn recompute_person_centroid(
    conn: &Connection,
    person_id: &str,
) -> Result<Option<Vec<f32>>> {
    let mut stmt = conn.prepare(
        "SELECT vector_json FROM person_examples WHERE person_id=?1",
    )?;
    let vecs: Vec<Vec<f32>> = stmt
        .query_map(params![person_id], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .filter_map(|json| serde_json::from_str::<Vec<f32>>(&json).ok())
        .collect();

    if vecs.is_empty() {
        return Ok(None);
    }

    let dim = vecs[0].len();
    let n = vecs.len() as f32;
    let mut centroid = vec![0.0_f32; dim];
    for v in &vecs {
        for (i, x) in v.iter().enumerate() {
            centroid[i] += x;
        }
    }
    for x in &mut centroid {
        *x /= n;
    }

    let json = serde_json::to_string(&centroid)?;
    conn.execute(
        "UPDATE people SET vector_json=?1 WHERE id=?2",
        params![json, person_id],
    )?;

    Ok(Some(centroid))
}

// ── settings ──────────────────────────────────────────────────────────────────

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key=?1")?;
    let result = stmt.query_row(params![key], |row| row.get(0));
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
}

