use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

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
}

/// Planned for Stage 3 – Face Anchoring.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub anchor_photo_id: String,
    pub face_crop_base64: Option<String>,
}

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
            indexed     INTEGER NOT NULL DEFAULT 0
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

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ")?;
    Ok(())
}

pub fn upsert_photos(conn: &Connection, photos: &[Photo]) -> Result<()> {
    let mut stmt = conn.prepare_cached("
        INSERT INTO photos (id, filename, description, created_at, width, height, base_url, mime_type, is_video, indexed)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(id) DO UPDATE SET
            base_url = excluded.base_url,
            description = excluded.description
    ")?;
    for p in photos {
        stmt.execute(params![
            p.id, p.filename, p.description, p.created_at,
            p.width, p.height, p.base_url, p.mime_type,
            p.is_video as i32, p.indexed as i32
        ])?;
    }
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

pub fn get_unindexed_photos(conn: &Connection, limit: usize) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, description, created_at, width, height, base_url, mime_type, is_video, indexed
         FROM photos WHERE indexed=0 AND is_video=0 AND base_url IS NOT NULL
         LIMIT ?1"
    )?;
    let rows: Vec<Result<Photo, rusqlite::Error>> = stmt.query_map([limit as i64], |row| {
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
        })
    })?.collect();
    Ok(rows.into_iter().filter_map(|r| r.ok()).collect())
}

pub fn get_all_photos(conn: &Connection) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, description, created_at, width, height, base_url, mime_type, is_video, indexed
         FROM photos ORDER BY created_at DESC"
    )?;
    let rows = stmt.query_map([], |row| {
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
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

pub fn semantic_search(conn: &Connection, query_vec: &[f32], limit: usize) -> Result<Vec<(Photo, f32)>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.filename, p.description, p.created_at, p.width, p.height,
                p.base_url, p.mime_type, p.is_video, p.indexed, e.vector_json
         FROM photos p JOIN embeddings e ON p.id = e.photo_id
         WHERE p.is_video = 0"
    )?;
    let mut results: Vec<(Photo, f32)> = stmt.query_map([], |row| {
        let vec_json: String = row.get(10)?;
        Ok((Photo {
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
        }, vec_json))
    })?
    .filter_map(|r| r.ok())
    .map(|(photo, json)| {
        let vec: Vec<f32> = serde_json::from_str(&json).unwrap_or_default();
        let score = cosine_similarity(query_vec, &vec);
        (photo, score)
    })
    .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(limit);
    Ok(results)
}

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
        "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn
    }

    fn sample_photo(id: &str) -> Photo {
        Photo {
            id: id.to_string(),
            filename: format!("{id}.jpg"),
            description: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            width: Some(1920),
            height: Some(1080),
            base_url: Some(format!("https://example.com/{id}")),
            mime_type: Some("image/jpeg".to_string()),
            is_video: false,
            indexed: false,
        }
    }

    // ── migrate ───────────────────────────────────────────────────────────────

    #[test]
    fn migrate_creates_tables() {
        let conn = test_db();
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert!(tables.contains(&"photos".to_string()));
        assert!(tables.contains(&"embeddings".to_string()));
        assert!(tables.contains(&"people".to_string()));
        assert!(tables.contains(&"settings".to_string()));
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = test_db();
        // Running migrate twice should not error
        assert!(migrate(&conn).is_ok());
    }

    // ── upsert_photos ─────────────────────────────────────────────────────────

    #[test]
    fn upsert_inserts_new_photos() {
        let conn = test_db();
        let photos = vec![sample_photo("p1"), sample_photo("p2")];
        upsert_photos(&conn, &photos).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM photos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn upsert_updates_existing_photo_base_url() {
        let conn = test_db();
        upsert_photos(&conn, &[sample_photo("p1")]).unwrap();

        let mut updated = sample_photo("p1");
        updated.base_url = Some("https://example.com/updated".to_string());
        upsert_photos(&conn, &[updated]).unwrap();

        let url: String = conn
            .query_row("SELECT base_url FROM photos WHERE id='p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(url, "https://example.com/updated");

        // Still only one row
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM photos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn upsert_preserves_indexed_flag_across_update() {
        let conn = test_db();
        upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
        // Mark as indexed manually
        conn.execute("UPDATE photos SET indexed=1 WHERE id='p1'", []).unwrap();

        // Upsert again (e.g. base_url refreshed by sync)
        upsert_photos(&conn, &[sample_photo("p1")]).unwrap();

        let indexed: i32 = conn
            .query_row("SELECT indexed FROM photos WHERE id='p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(indexed, 1, "indexed flag must survive an upsert");
    }

    // ── save_embedding ────────────────────────────────────────────────────────

    #[test]
    fn save_embedding_stores_vector_and_marks_indexed() {
        let conn = test_db();
        upsert_photos(&conn, &[sample_photo("p1")]).unwrap();

        let vec: Vec<f32> = vec![0.1, 0.2, 0.3];
        save_embedding(&conn, "p1", &vec).unwrap();

        let indexed: i32 = conn
            .query_row("SELECT indexed FROM photos WHERE id='p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(indexed, 1);

        let json: String = conn
            .query_row("SELECT vector_json FROM embeddings WHERE photo_id='p1'", [], |r| r.get(0))
            .unwrap();
        let recovered: Vec<f32> = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, vec);
    }

    #[test]
    fn save_embedding_replaces_existing_vector() {
        let conn = test_db();
        upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
        save_embedding(&conn, "p1", &[0.1, 0.2]).unwrap();
        save_embedding(&conn, "p1", &[0.9, 0.8]).unwrap();

        let json: String = conn
            .query_row("SELECT vector_json FROM embeddings WHERE photo_id='p1'", [], |r| r.get(0))
            .unwrap();
        let recovered: Vec<f32> = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, vec![0.9_f32, 0.8_f32]);
    }

    // ── cosine_similarity ─────────────────────────────────────────────────────

    #[test]
    fn cosine_similarity_identical_vectors() {
        let v = vec![1.0_f32, 0.0, 0.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6, "identical vectors → 1.0, got {sim}");
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6, "orthogonal → 0.0, got {sim}");
    }

    #[test]
    fn cosine_similarity_opposite_vectors() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![-1.0_f32, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6, "opposite → -1.0, got {sim}");
    }

    #[test]
    fn cosine_similarity_zero_vector_returns_zero() {
        let a = vec![0.0_f32, 0.0];
        let b = vec![1.0_f32, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    // ── semantic_search ───────────────────────────────────────────────────────

    #[test]
    fn semantic_search_returns_ranked_results() {
        let conn = test_db();
        upsert_photos(&conn, &[sample_photo("p1"), sample_photo("p2")]).unwrap();
        // p1 is aligned with query; p2 is orthogonal
        save_embedding(&conn, "p1", &[1.0, 0.0]).unwrap();
        save_embedding(&conn, "p2", &[0.0, 1.0]).unwrap();

        let query = vec![1.0_f32, 0.0];
        let results = semantic_search(&conn, &query, 10).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.id, "p1");
        assert!(results[0].1 > results[1].1);
    }

    #[test]
    fn semantic_search_excludes_videos() {
        let conn = test_db();
        let mut video = sample_photo("v1");
        video.is_video = true;
        upsert_photos(&conn, &[sample_photo("p1"), video]).unwrap();
        save_embedding(&conn, "p1", &[1.0, 0.0]).unwrap();
        // Give video an embedding too — it should still be excluded
        conn.execute(
            "INSERT INTO embeddings(photo_id, vector_json) VALUES('v1', '[1.0, 0.0]')",
            [],
        )
        .unwrap();

        let results = semantic_search(&conn, &[1.0_f32, 0.0], 10).unwrap();
        assert!(results.iter().all(|(p, _)| !p.is_video));
    }

    #[test]
    fn semantic_search_respects_limit() {
        let conn = test_db();
        for i in 0..5 {
            let p = sample_photo(&format!("p{i}"));
            upsert_photos(&conn, &[p]).unwrap();
            save_embedding(&conn, &format!("p{i}"), &[1.0, 0.0]).unwrap();
        }
        let results = semantic_search(&conn, &[1.0_f32, 0.0], 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    // ── get_unindexed_photos ──────────────────────────────────────────────────

    #[test]
    fn get_unindexed_excludes_indexed_and_videos() {
        let conn = test_db();
        let mut video = sample_photo("v1");
        video.is_video = true;
        upsert_photos(&conn, &[sample_photo("p1"), sample_photo("p2"), video]).unwrap();
        // Mark p1 as indexed
        conn.execute("UPDATE photos SET indexed=1 WHERE id='p1'", []).unwrap();

        let unindexed = get_unindexed_photos(&conn, 100).unwrap();
        let ids: Vec<&str> = unindexed.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"p2"), "p2 should be unindexed");
        assert!(!ids.contains(&"p1"), "p1 is indexed — should be excluded");
        assert!(!ids.contains(&"v1"), "videos should be excluded");
    }

    // ── settings ──────────────────────────────────────────────────────────────

    #[test]
    fn settings_roundtrip() {
        let conn = test_db();
        set_setting(&conn, "theme", "dark").unwrap();
        assert_eq!(get_setting(&conn, "theme").unwrap(), Some("dark".to_string()));
    }

    #[test]
    fn settings_missing_key_returns_none() {
        let conn = test_db();
        assert_eq!(get_setting(&conn, "nonexistent").unwrap(), None);
    }

    #[test]
    fn settings_upsert_overwrites() {
        let conn = test_db();
        set_setting(&conn, "key", "v1").unwrap();
        set_setting(&conn, "key", "v2").unwrap();
        assert_eq!(get_setting(&conn, "key").unwrap(), Some("v2".to_string()));
    }
}
