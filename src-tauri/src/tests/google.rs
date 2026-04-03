use crate::integrations::google::{auth_url, iso_to_unix, parse_oauth_code, url_decode};

#[test]
fn auth_url_contains_client_id() {
    let url = auth_url("my-client-id", "http://127.0.0.1:12345");
    assert!(url.contains("my-client-id"));
}

#[test]
fn auth_url_contains_picker_scope() {
    let url = auth_url("id", "http://127.0.0.1:1");
    assert!(url.contains("photospicker.mediaitems.readonly"));
}

#[test]
fn auth_url_requests_offline_access() {
    let url = auth_url("id", "http://127.0.0.1:1");
    assert!(url.contains("access_type=offline"));
}

#[test]
fn parse_oauth_code_extracts_code() {
    let request = "GET /?code=4%2F0ABC123&scope=openid HTTP/1.1\r\nHost: 127.0.0.1\r\n";
    let code = parse_oauth_code(request).unwrap();
    assert_eq!(code, "4/0ABC123");
}

#[test]
fn parse_oauth_code_returns_none_without_code() {
    let request = "GET /?error=access_denied HTTP/1.1\r\n";
    assert!(parse_oauth_code(request).is_none());
}

#[test]
fn url_decode_handles_percent_encoding() {
    assert_eq!(url_decode("hello%20world"), "hello world");
    assert_eq!(url_decode("4%2F0AX"), "4/0AX");
    assert_eq!(url_decode("a+b"), "a b");
}

#[test]
fn iso_to_unix_parses_rfc3339() {
    let ts = iso_to_unix("2021-01-01T00:00:00Z").unwrap();
    assert_eq!(ts, "1609459200");
}

#[test]
fn iso_to_unix_returns_none_for_invalid() {
    assert!(iso_to_unix("not-a-date").is_none());
}
