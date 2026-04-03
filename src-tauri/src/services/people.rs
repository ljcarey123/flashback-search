use anyhow::Result;
use base64::Engine;
use rusqlite::Connection;

use crate::{db, integrations::face::{FaceBbox, FaceEngine}};

/// Business logic for the people / face-recognition feature.
///
/// Commands in `commands::people` are thin wrappers that handle Tauri
/// wiring (state locking, error mapping) and delegate here.
pub struct PeopleService;

impl PeopleService {
    /// Crop and embed a face, create the person record, and store the first
    /// face example. Returns the newly created `Person`.
    pub fn create_person(
        face: &FaceEngine,
        conn: &Connection,
        name: &str,
        photo_id: &str,
        bbox: &FaceBbox,
        thumb_bytes: &[u8],
    ) -> Result<db::Person> {
        let (crop_bytes, vector) = face.crop_and_embed(thumb_bytes, bbox)?;
        let face_crop_b64 = base64::engine::general_purpose::STANDARD.encode(&crop_bytes);
        let vector_json = serde_json::to_string(&vector)?;
        let person_id = uuid::Uuid::new_v4().to_string();

        db::insert_person(conn, &person_id, name, photo_id, Some(&face_crop_b64), &vector_json)?;
        db::insert_person_example(conn, &person_id, Some(&face_crop_b64), &vector_json)?;

        Ok(db::Person {
            id: person_id,
            name: name.to_string(),
            anchor_photo_id: photo_id.to_string(),
            face_crop_base64: Some(face_crop_b64),
        })
    }

    /// Add a face example to an existing person and recompute their centroid.
    pub fn add_example(
        face: &FaceEngine,
        conn: &Connection,
        person_id: &str,
        bbox: &FaceBbox,
        thumb_bytes: &[u8],
    ) -> Result<()> {
        let (crop_bytes, vector) = face.crop_and_embed(thumb_bytes, bbox)?;
        let face_crop_b64 = base64::engine::general_purpose::STANDARD.encode(&crop_bytes);
        let vector_json = serde_json::to_string(&vector)?;

        db::insert_person_example(conn, person_id, Some(&face_crop_b64), &vector_json)?;
        db::recompute_person_centroid(conn, person_id)?;

        Ok(())
    }

    /// Run face detection on a thumbnail, store results in the DB, and return
    /// the detected bounding boxes. Replaces any previously stored faces for
    /// this photo.
    pub fn detect_for_photo(
        face: &FaceEngine,
        conn: &Connection,
        photo_id: &str,
        thumb_bytes: &[u8],
    ) -> Result<Vec<FaceBbox>> {
        db::delete_faces_for_photo(conn, photo_id)?;
        let bboxes = face.detect(thumb_bytes)?;
        for bbox in &bboxes {
            let json = serde_json::to_string(bbox)?;
            db::insert_face(conn, photo_id, &json)?;
        }
        db::mark_faces_detected(conn, photo_id)?;
        Ok(bboxes)
    }
}
