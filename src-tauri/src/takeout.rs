/// Google Takeout folder scanner.
///
/// Google Takeout exports photos as a folder tree under
/// `Takeout/Google Photos/`, with each image accompanied by a JSON sidecar
/// that contains the original metadata (timestamp, title, description, etc.).
///
/// This module discovers every image/video file in such a folder, pairs it
/// with its sidecar, and returns a flat list of [`TakeoutEntry`] structs ready
/// for import.
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Supported image extensions that the `image` crate can decode.
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "tiff", "tif", "bmp"];
/// Video extensions — imported as metadata-only (no thumbnail generated).
const VIDEO_EXTS: &[&str] = &["mp4", "mov", "avi", "mkv", "m4v", "3gp", "wmv", "flv"];

// ── Sidecar JSON structures ───────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct Sidecar {
    title: Option<String>,
    description: Option<String>,
    #[serde(rename = "photoTakenTime")]
    photo_taken_time: Option<TimestampField>,
    #[serde(rename = "creationTime")]
    creation_time: Option<TimestampField>,
}

#[derive(Deserialize)]
struct TimestampField {
    /// Unix epoch as a decimal string (e.g. "1609459200").
    timestamp: Option<String>,
}

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TakeoutEntry {
    /// Absolute path to the image/video file.
    pub path: PathBuf,
    /// Original filename from the sidecar `title` (falls back to filesystem name).
    pub filename: String,
    pub description: Option<String>,
    /// ISO 8601 creation time, e.g. "2021-01-01T00:00:00Z".
    pub created_at: Option<String>,
    /// `{unix_timestamp}_{filename}` — used for cross-source deduplication.
    pub fingerprint: Option<String>,
    pub is_video: bool,
}

/// Walk `folder` recursively and return every image/video entry found.
///
/// Files that cannot be read or are not recognised image/video types are
/// silently skipped.  Sidecar JSON files are never returned as entries.
pub fn scan_folder(folder: &Path) -> Vec<TakeoutEntry> {
    WalkDir::new(folder)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| entry_from_path(e.path()))
        .collect()
}

// ── internals ─────────────────────────────────────────────────────────────────

fn entry_from_path(path: &Path) -> Option<TakeoutEntry> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())?;

    let is_image = IMAGE_EXTS.contains(&ext.as_str());
    let is_video = VIDEO_EXTS.contains(&ext.as_str());

    if !is_image && !is_video {
        return None;
    }

    let sidecar = read_sidecar(path).unwrap_or_default();

    let filename = sidecar
        .title
        .clone()
        .unwrap_or_else(|| path.file_name().unwrap_or_default().to_string_lossy().into_owned());

    let unix_timestamp: Option<String> = sidecar
        .photo_taken_time
        .as_ref()
        .and_then(|t| t.timestamp.clone())
        .or_else(|| {
            sidecar
                .creation_time
                .as_ref()
                .and_then(|t| t.timestamp.clone())
        })
        .or_else(|| {
            // Fall back to file modification time
            std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        .to_string()
                })
        });

    let created_at = unix_timestamp.as_ref().and_then(|ts| {
        ts.parse::<i64>().ok().map(|secs| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default()
        })
    });

    let fingerprint = unix_timestamp.as_ref().map(|ts| format!("{ts}_{filename}"));

    Some(TakeoutEntry {
        path: path.to_path_buf(),
        filename,
        description: sidecar.description,
        created_at,
        fingerprint,
        is_video,
    })
}

/// Try to load and parse the JSON sidecar for a given media file.
///
/// Google Takeout names the sidecar `{filename}.json`.  For very long
/// filenames the base may be truncated, so we also try `{stem}.json`.
fn read_sidecar(media_path: &Path) -> Result<Sidecar> {
    // Primary: "photo.jpg.json"
    let primary = media_path.with_file_name(format!(
        "{}.json",
        media_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    if primary.exists() {
        let text = std::fs::read_to_string(&primary)
            .with_context(|| format!("reading {}", primary.display()))?;
        return Ok(serde_json::from_str(&text).unwrap_or_default());
    }

    // Fallback: "photo.json" (no image extension in the sidecar name)
    let fallback = media_path.with_extension("json");
    if fallback.exists() {
        let text = std::fs::read_to_string(&fallback)
            .with_context(|| format!("reading {}", fallback.display()))?;
        return Ok(serde_json::from_str(&text).unwrap_or_default());
    }

    Ok(Sidecar::default())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn scan_finds_images_and_skips_json() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "photo.jpg", "fake-jpeg");
        write(tmp.path(), "photo.jpg.json", r#"{"title":"photo.jpg"}"#);
        write(tmp.path(), "clip.mp4", "fake-video");
        write(tmp.path(), "readme.txt", "text");

        let entries = scan_folder(tmp.path());
        let names: Vec<&str> = entries.iter().map(|e| e.filename.as_str()).collect();
        assert!(names.contains(&"photo.jpg"), "image should be found");
        assert!(names.contains(&"clip.mp4"), "video should be found");
        assert!(!names.iter().any(|n| n.ends_with(".json")), "JSON skipped");
        assert!(!names.iter().any(|n| n.ends_with(".txt")), "txt skipped");
    }

    #[test]
    fn entry_reads_title_and_timestamp_from_sidecar() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "IMG_001.jpg", "data");
        write(
            tmp.path(),
            "IMG_001.jpg.json",
            r#"{"title":"IMG_001.jpg","photoTakenTime":{"timestamp":"1609459200"}}"#,
        );

        let entries = scan_folder(tmp.path());
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.filename, "IMG_001.jpg");
        assert_eq!(e.fingerprint.as_deref(), Some("1609459200_IMG_001.jpg"));
    }

    #[test]
    fn entry_falls_back_to_creation_time() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "IMG_002.jpg", "data");
        write(
            tmp.path(),
            "IMG_002.jpg.json",
            r#"{"title":"IMG_002.jpg","creationTime":{"timestamp":"1620000000"}}"#,
        );

        let entries = scan_folder(tmp.path());
        let e = &entries[0];
        assert_eq!(e.fingerprint.as_deref(), Some("1620000000_IMG_002.jpg"));
    }

    #[test]
    fn video_entries_are_marked_as_video() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "video.mp4", "data");

        let entries = scan_folder(tmp.path());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_video);
    }

    #[test]
    fn entry_without_sidecar_uses_filename() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "mystery.jpg", "data");

        let entries = scan_folder(tmp.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].filename, "mystery.jpg");
    }

    #[test]
    fn scan_walks_subdirectories() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("2024");
        fs::create_dir(&sub).unwrap();
        write(&sub, "nested.jpg", "data");

        let entries = scan_folder(tmp.path());
        assert!(!entries.is_empty());
    }
}
