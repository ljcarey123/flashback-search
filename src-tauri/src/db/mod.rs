use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

mod faces;
mod people;
mod photos;
mod settings;

pub use faces::*;
pub use people::*;
pub use photos::*;
pub use settings::*;

// ── Types ─────────────────────────────────────────────────────────────────────

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
    pub local_path: Option<String>,
    pub fingerprint: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersonExample {
    pub id: String,
    pub person_id: String,
    pub face_crop_base64: Option<String>,
}

// ── Shared helpers (accessible to submodules) ─────────────────────────────────

pub(crate) const SELECT_COLS: &str =
    "id, filename, description, created_at, width, height, base_url, mime_type, \
     is_video, indexed, local_path, fingerprint";

pub(crate) fn row_to_photo(row: &rusqlite::Row<'_>) -> rusqlite::Result<Photo> {
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
        thumb_path: None,
    })
}

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

// ── Connection ────────────────────────────────────────────────────────────────

pub fn open(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
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
    let _ = conn.execute("ALTER TABLE photos ADD COLUMN local_path TEXT", []);
    let _ = conn.execute("ALTER TABLE photos ADD COLUMN fingerprint TEXT", []);
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
