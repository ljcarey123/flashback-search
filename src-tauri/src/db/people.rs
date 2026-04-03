use anyhow::Result;
use rusqlite::{params, Connection};

use super::{Person, PersonExample};

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

// ── Person examples ───────────────────────────────────────────────────────────

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
