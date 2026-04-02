use crate::db::{
    cosine_similarity, delete_faces_for_photo, delete_person, face_search, find_by_fingerprint,
    get_all_photos, get_photos_needing_face_detection, get_setting, get_unembedded_faces,
    get_unindexed_photos, insert_face, insert_person, insert_person_example,
    insert_photo_if_new, list_people, list_person_examples, mark_faces_detected, migrate,
    recompute_person_centroid, reset_index, save_embedding, save_face_embedding,
    semantic_search, set_setting, upsert_photos, Photo,
};
use rusqlite::Connection;

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
        local_path: Some(format!("/photos/{id}.jpg")),
        fingerprint: Some(format!("1704067200_{id}.jpg")),
        thumb_path: None,
    }
}

// ── migrate ───────────────────────────────────────────────────────────────────

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
    assert!(tables.contains(&"faces".to_string()));
    assert!(tables.contains(&"people".to_string()));
    assert!(tables.contains(&"person_examples".to_string()));
    assert!(tables.contains(&"settings".to_string()));
}

#[test]
fn migrate_is_idempotent() {
    let conn = test_db();
    assert!(migrate(&conn).is_ok());
}

// ── upsert_photos ─────────────────────────────────────────────────────────────

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

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM photos", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn upsert_preserves_indexed_flag_across_update() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    conn.execute("UPDATE photos SET indexed=1 WHERE id='p1'", []).unwrap();

    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();

    let indexed: i32 = conn
        .query_row("SELECT indexed FROM photos WHERE id='p1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(indexed, 1, "indexed flag must survive an upsert");
}

// ── insert_photo_if_new ───────────────────────────────────────────────────────

#[test]
fn insert_photo_if_new_inserts_first_time() {
    let conn = test_db();
    let inserted = insert_photo_if_new(&conn, &sample_photo("p1")).unwrap();
    assert!(inserted);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM photos", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn insert_photo_if_new_skips_duplicate_fingerprint() {
    let conn = test_db();
    insert_photo_if_new(&conn, &sample_photo("p1")).unwrap();

    // Different id, same fingerprint
    let mut dup = sample_photo("p2");
    dup.fingerprint = Some("1704067200_p1.jpg".to_string()); // same as p1
    let inserted = insert_photo_if_new(&conn, &dup).unwrap();
    assert!(!inserted);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM photos", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

// ── find_by_fingerprint ───────────────────────────────────────────────────────

#[test]
fn find_by_fingerprint_returns_matching_photo() {
    let conn = test_db();
    insert_photo_if_new(&conn, &sample_photo("p1")).unwrap();
    let found = find_by_fingerprint(&conn, "1704067200_p1.jpg").unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "p1");
}

#[test]
fn find_by_fingerprint_returns_none_for_missing() {
    let conn = test_db();
    let found = find_by_fingerprint(&conn, "nope").unwrap();
    assert!(found.is_none());
}

// ── save_embedding ────────────────────────────────────────────────────────────

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
        .query_row(
            "SELECT vector_json FROM embeddings WHERE photo_id='p1'",
            [],
            |r| r.get(0),
        )
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
        .query_row(
            "SELECT vector_json FROM embeddings WHERE photo_id='p1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let recovered: Vec<f32> = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, vec![0.9_f32, 0.8_f32]);
}

// ── cosine_similarity ─────────────────────────────────────────────────────────

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

// ── semantic_search ───────────────────────────────────────────────────────────

#[test]
fn semantic_search_returns_ranked_results() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1"), sample_photo("p2")]).unwrap();
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

// ── get_unindexed_photos ──────────────────────────────────────────────────────

#[test]
fn get_unindexed_excludes_indexed_videos_and_no_local_path() {
    let conn = test_db();

    let mut video = sample_photo("v1");
    video.is_video = true;

    let mut no_path = sample_photo("p3");
    no_path.local_path = None;
    no_path.fingerprint = Some("1704067200_p3.jpg".to_string());

    upsert_photos(&conn, &[sample_photo("p1"), sample_photo("p2"), video, no_path]).unwrap();
    conn.execute("UPDATE photos SET indexed=1 WHERE id='p1'", []).unwrap();

    let unindexed = get_unindexed_photos(&conn, 100).unwrap();
    let ids: Vec<&str> = unindexed.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"p2"), "p2 should be unindexed");
    assert!(!ids.contains(&"p1"), "p1 is indexed");
    assert!(!ids.contains(&"v1"), "videos excluded");
    assert!(!ids.contains(&"p3"), "no-local-path excluded");
}

// ── get_all_photos ────────────────────────────────────────────────────────────

#[test]
fn get_all_photos_returns_newest_first() {
    let conn = test_db();
    let mut older = sample_photo("old");
    older.created_at = Some("2023-01-01T00:00:00Z".to_string());
    older.fingerprint = Some("1672531200_old.jpg".to_string());
    let mut newer = sample_photo("new");
    newer.created_at = Some("2024-06-01T00:00:00Z".to_string());
    newer.fingerprint = Some("1717200000_new.jpg".to_string());

    upsert_photos(&conn, &[older, newer]).unwrap();

    let photos = get_all_photos(&conn).unwrap();
    assert_eq!(photos[0].id, "new", "newer photo should come first");
    assert_eq!(photos[1].id, "old");
}

// ── settings ──────────────────────────────────────────────────────────────────

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

// ── faces ─────────────────────────────────────────────────────────────────────

#[test]
fn insert_face_returns_id_and_is_retrievable() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    let id = insert_face(&conn, "p1", r#"{"x":0.1,"y":0.1,"w":0.2,"h":0.3}"#).unwrap();
    assert!(!id.is_empty());

    let faces = crate::db::get_faces_for_photo(&conn, "p1").unwrap();
    assert_eq!(faces.len(), 1);
    assert_eq!(faces[0].id, id);
    assert!(faces[0].vector_json.is_none());
}

#[test]
fn save_face_embedding_stores_vector() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    let id = insert_face(&conn, "p1", r#"{"x":0.1,"y":0.1,"w":0.2,"h":0.3}"#).unwrap();
    save_face_embedding(&conn, &id, &[0.5_f32, 0.5]).unwrap();

    let faces = crate::db::get_faces_for_photo(&conn, "p1").unwrap();
    assert!(faces[0].vector_json.is_some());
}

#[test]
fn get_unembedded_faces_excludes_embedded() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    let id1 = insert_face(&conn, "p1", r#"{"x":0.1,"y":0.1,"w":0.2,"h":0.3}"#).unwrap();
    let id2 = insert_face(&conn, "p1", r#"{"x":0.5,"y":0.1,"w":0.2,"h":0.3}"#).unwrap();
    save_face_embedding(&conn, &id1, &[1.0_f32]).unwrap();

    let unembedded = get_unembedded_faces(&conn, 100).unwrap();
    let ids: Vec<&str> = unembedded.iter().map(|f| f.id.as_str()).collect();
    assert!(!ids.contains(&id1.as_str()), "embedded face excluded");
    assert!(ids.contains(&id2.as_str()), "unembedded face included");
}

#[test]
fn delete_faces_for_photo_removes_all() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    insert_face(&conn, "p1", r#"{"x":0.1,"y":0.1,"w":0.2,"h":0.3}"#).unwrap();
    insert_face(&conn, "p1", r#"{"x":0.5,"y":0.1,"w":0.2,"h":0.3}"#).unwrap();

    delete_faces_for_photo(&conn, "p1").unwrap();
    let faces = crate::db::get_faces_for_photo(&conn, "p1").unwrap();
    assert!(faces.is_empty());
}

#[test]
fn mark_faces_detected_sets_flag() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    conn.execute("UPDATE photos SET indexed=1 WHERE id='p1'", []).unwrap();

    let pending = get_photos_needing_face_detection(&conn, 100).unwrap();
    assert!(pending.iter().any(|p| p.id == "p1"));

    mark_faces_detected(&conn, "p1").unwrap();
    let pending2 = get_photos_needing_face_detection(&conn, 100).unwrap();
    assert!(!pending2.iter().any(|p| p.id == "p1"));
}

#[test]
fn face_search_returns_photo_with_best_score() {
    let conn = test_db();
    let mut p1 = sample_photo("p1");
    p1.fingerprint = Some("111_p1.jpg".to_string());
    let mut p2 = sample_photo("p2");
    p2.fingerprint = Some("222_p2.jpg".to_string());
    upsert_photos(&conn, &[p1, p2]).unwrap();

    let id1 = insert_face(&conn, "p1", r#"{"x":0.1,"y":0.1,"w":0.2,"h":0.2}"#).unwrap();
    let id2 = insert_face(&conn, "p2", r#"{"x":0.1,"y":0.1,"w":0.2,"h":0.2}"#).unwrap();
    save_face_embedding(&conn, &id1, &[1.0_f32, 0.0]).unwrap();
    save_face_embedding(&conn, &id2, &[0.0_f32, 1.0]).unwrap();

    let results = face_search(&conn, &[1.0_f32, 0.0], 10).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0.id, "p1");
    assert!(results[0].1 > results[1].1);
}

// ── people ────────────────────────────────────────────────────────────────────

#[test]
fn insert_and_list_people() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    insert_person(&conn, "person1", "Alice", "p1", None, "[0.1, 0.2]").unwrap();
    insert_person(&conn, "person2", "Bob", "p1", None, "[0.3, 0.4]").unwrap();

    let people = list_people(&conn).unwrap();
    assert_eq!(people.len(), 2);
    // Ordered by name
    assert_eq!(people[0].name, "Alice");
    assert_eq!(people[1].name, "Bob");
}

#[test]
fn delete_person_removes_row() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    insert_person(&conn, "person1", "Alice", "p1", None, "[]").unwrap();
    delete_person(&conn, "person1").unwrap();
    assert!(list_people(&conn).unwrap().is_empty());
}

#[test]
fn recompute_centroid_averages_examples() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    insert_person(&conn, "person1", "Alice", "p1", None, "[]").unwrap();

    insert_person_example(&conn, "person1", None, "[1.0, 0.0]").unwrap();
    insert_person_example(&conn, "person1", None, "[0.0, 1.0]").unwrap();

    let centroid = recompute_person_centroid(&conn, "person1").unwrap().unwrap();
    assert_eq!(centroid.len(), 2);
    assert!((centroid[0] - 0.5).abs() < 1e-5);
    assert!((centroid[1] - 0.5).abs() < 1e-5);
}

#[test]
fn list_person_examples_returns_all() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    insert_person(&conn, "person1", "Alice", "p1", None, "[]").unwrap();
    insert_person_example(&conn, "person1", None, "[1.0]").unwrap();
    insert_person_example(&conn, "person1", None, "[2.0]").unwrap();

    let examples = list_person_examples(&conn, "person1").unwrap();
    assert_eq!(examples.len(), 2);
}

// ── reset_index ───────────────────────────────────────────────────────────────

#[test]
fn reset_index_clears_embeddings_and_unsets_indexed() {
    let conn = test_db();
    upsert_photos(&conn, &[sample_photo("p1")]).unwrap();
    save_embedding(&conn, "p1", &[1.0, 0.0]).unwrap();

    reset_index(&conn).unwrap();

    let indexed: i32 = conn
        .query_row("SELECT indexed FROM photos WHERE id='p1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(indexed, 0);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM embeddings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}
