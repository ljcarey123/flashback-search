use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use serde::Serialize;

pub mod auth;
pub mod debug;
pub mod import;
pub mod index;
pub mod library;
pub mod people;
pub mod settings;

pub use crate::state::AppState;

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SearchResult {
    pub photo: crate::db::Photo,
    pub score: f32,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

pub fn thumb_path(data_dir: &Path, photo_id: &str) -> PathBuf {
    data_dir
        .join("thumbnails")
        .join(photo_id)
        .with_extension("jpg")
}

pub fn photos_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("photos")
}

/// Attach the computed `thumb_path` to a list of photos (in-place).
pub fn enrich(mut photos: Vec<crate::db::Photo>, data_dir: &Path) -> Vec<crate::db::Photo> {
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
pub fn make_thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let thumb = img.resize(512, 512, FilterType::Triangle);
    let mut out = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .ok()?;
    Some(out)
}
