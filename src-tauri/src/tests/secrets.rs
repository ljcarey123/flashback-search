use crate::secrets::{delete, get, set};

// These tests hit the real OS keychain — they're integration-level.
// Run with: cargo test -- --ignored

#[test]
#[ignore]
fn roundtrip_secret() {
    set("test_key", "test_value").unwrap();
    assert_eq!(get("test_key").unwrap(), Some("test_value".into()));
    delete("test_key").unwrap();
    assert_eq!(get("test_key").unwrap(), None);
}

#[test]
#[ignore]
fn delete_missing_is_ok() {
    delete("nonexistent_key_xyz").unwrap();
}
