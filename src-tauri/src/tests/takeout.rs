use crate::takeout::scan_folder;
use std::fs;
use std::path::Path;
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
