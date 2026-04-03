use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

// ── OAuth helpers ─────────────────────────────────────────────────────────────

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Scopes required for the Google Photos Picker API.
const PICKER_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/photospicker.mediaitems.readonly",
    "https://www.googleapis.com/auth/userinfo.profile",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
}

/// Build the consent URL the user opens in their browser.
pub fn auth_url(client_id: &str, redirect_uri: &str) -> String {
    let scopes = PICKER_SCOPES.join(" ");
    format!(
        "{AUTH_URL}?client_id={client_id}&redirect_uri={redirect}&\
         response_type=code&scope={scope}&access_type=offline&prompt=consent",
        redirect = url_encode(redirect_uri),
        scope = url_encode(&scopes),
    )
}

fn url_encode(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            ' ' => vec!['%', '2', '0'],
            ':' => vec!['%', '3', 'A'],
            '/' => vec!['%', '2', 'F'],
            _ => vec![c],
        })
        .collect()
}

pub(crate) fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte as char);
                    i += 3;
                    continue;
                }
            }
        } else if bytes[i] == b'+' {
            out.push(' ');
            i += 1;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Exchange an auth code for access + refresh tokens.
pub async fn exchange_code(
    client: &Client,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<OAuthTokens> {
    let params = [
        ("code", code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
    ];
    let resp = client.post(TOKEN_URL).form(&params).send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("Token exchange failed ({status}): {body}"));
    }
    Ok(serde_json::from_str(&body)?)
}

/// Refresh an expired access token.
#[allow(dead_code)]
pub async fn refresh_token(
    client: &Client,
    client_id: &str,
    client_secret: &str,
    refresh_tok: &str,
) -> Result<OAuthTokens> {
    let params = [
        ("refresh_token", refresh_tok),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "refresh_token"),
    ];
    let resp = client.post(TOKEN_URL).form(&params).send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("Token refresh failed ({status}): {body}"));
    }
    Ok(serde_json::from_str(&body)?)
}

// ── Localhost OAuth redirect server ──────────────────────────────────────────

/// Start a one-shot TCP server on a random port that catches the OAuth redirect.
///
/// Returns `(port, receiver)`.  The receiver resolves with the auth `code`
/// once Google redirects the browser to `http://127.0.0.1:{port}/?code=...`.
pub async fn start_oauth_server() -> Result<(u16, tokio::sync::oneshot::Receiver<String>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);

            let html = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: text/html; charset=utf-8\r\n\r\n",
                "<!doctype html><html><body style='font-family:sans-serif;padding:2em'>",
                "<h2>✓ Flashback sign-in complete</h2>",
                "<p>You can close this tab and return to the app.</p>",
                "</body></html>",
            );
            let _ = stream.write_all(html.as_bytes()).await;

            if let Some(code) = parse_oauth_code(&request) {
                let _ = tx.send(code);
            }
        }
    });

    Ok((port, rx))
}

/// Extract `code` from the first line of an HTTP/1.1 GET request.
pub(crate) fn parse_oauth_code(request: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some(val) = pair.strip_prefix("code=") {
            return Some(url_decode(val));
        }
    }
    None
}

// ── User profile ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug)]
pub struct UserProfile {
    pub name: String,
    pub picture: Option<String>,
}

pub async fn get_user_profile(client: &Client, access_token: &str) -> Result<UserProfile> {
    let resp = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("Profile fetch failed ({status}): {body}"));
    }
    Ok(serde_json::from_str(&body)?)
}

// ── Google Photos Picker API ──────────────────────────────────────────────────

const PICKER_API: &str = "https://photospicker.googleapis.com/v1";

#[derive(Debug, Clone, Serialize)]
pub struct PickerSession {
    pub id: String,
    pub picker_uri: String,
}

#[derive(Debug, Clone)]
pub struct PickerMediaItem {
    pub id: String,
    pub create_time: Option<String>,
    pub filename: String,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Short-lived (~60 min) CDN base URL.  Append `=w512-h512-c` for a 512px thumbnail.
    pub base_url: String,
    pub is_video: bool,
}

// ── Picker API deserialization types ──────────────────────────────────────────

#[derive(Deserialize)]
struct SessionResponse {
    id: String,
    #[serde(rename = "pickerUri")]
    picker_uri: String,
}

#[derive(Deserialize)]
struct SessionPollResponse {
    #[serde(rename = "mediaItemsSet", default)]
    media_items_set: bool,
}

#[derive(Deserialize)]
struct MediaItemsResponse {
    #[serde(rename = "mediaItems", default)]
    media_items: Vec<RawMediaItem>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct RawMediaItem {
    id: String,
    #[serde(rename = "createTime")]
    create_time: Option<String>,
    #[serde(rename = "type")]
    item_type: Option<String>,
    #[serde(rename = "mediaFile")]
    media_file: Option<RawMediaFile>,
}

#[derive(Deserialize)]
struct RawMediaFile {
    #[serde(rename = "baseUrl")]
    base_url: Option<String>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    filename: Option<String>,
    #[serde(rename = "mediaFileMetadata")]
    metadata: Option<RawMediaFileMetadata>,
}

#[derive(Deserialize)]
struct RawMediaFileMetadata {
    width: Option<serde_json::Value>,
    height: Option<serde_json::Value>,
}

fn parse_dim(v: Option<&serde_json::Value>) -> Option<i64> {
    v.and_then(|x| x.as_i64().or_else(|| x.as_str()?.parse().ok()))
}

// ── Picker API functions ──────────────────────────────────────────────────────

pub async fn create_picker_session(client: &Client, access_token: &str) -> Result<PickerSession> {
    let resp = client
        .post(format!("{PICKER_API}/sessions"))
        .bearer_auth(access_token)
        .json(&serde_json::json!({}))
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("create_picker_session failed ({status}): {body}"));
    }
    let parsed: SessionResponse = serde_json::from_str(&body)?;
    Ok(PickerSession {
        id: parsed.id,
        picker_uri: parsed.picker_uri,
    })
}

pub async fn poll_picker_session(
    client: &Client,
    access_token: &str,
    session_id: &str,
) -> Result<bool> {
    let resp = client
        .get(format!("{PICKER_API}/sessions/{session_id}"))
        .bearer_auth(access_token)
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("poll_picker_session failed ({status}): {body}"));
    }
    let parsed: SessionPollResponse = serde_json::from_str(&body)?;
    Ok(parsed.media_items_set)
}

pub async fn list_picker_items(
    client: &Client,
    access_token: &str,
    session_id: &str,
) -> Result<Vec<PickerMediaItem>> {
    let mut all = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut url = format!(
            "{PICKER_API}/mediaItems?sessionId={session_id}&pageSize=100"
        );
        if let Some(ref pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }

        let resp = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("list_picker_items failed ({status}): {body}"));
        }

        let parsed: MediaItemsResponse = serde_json::from_str(&body)?;
        let next = parsed.next_page_token.clone();

        for item in parsed.media_items {
            if let Some(mf) = item.media_file {
                let base_url = match mf.base_url {
                    Some(u) => u,
                    None => continue,
                };
                let (w, h) = mf
                    .metadata
                    .as_ref()
                    .map(|m| (parse_dim(m.width.as_ref()), parse_dim(m.height.as_ref())))
                    .unwrap_or((None, None));

                let is_video = item
                    .item_type
                    .as_deref()
                    .map(|t| t == "VIDEO")
                    .unwrap_or_else(|| {
                        mf.mime_type
                            .as_deref()
                            .map(|m| m.starts_with("video/"))
                            .unwrap_or(false)
                    });

                all.push(PickerMediaItem {
                    id: item.id,
                    create_time: item.create_time,
                    filename: mf.filename.unwrap_or_else(|| "photo.jpg".to_string()),
                    mime_type: mf.mime_type,
                    width: w,
                    height: h,
                    base_url,
                    is_video,
                });
            }
        }

        if next.is_none() {
            break;
        }
        page_token = next;
    }

    Ok(all)
}

pub async fn delete_picker_session(
    client: &Client,
    access_token: &str,
    session_id: &str,
) -> Result<()> {
    client
        .delete(format!("{PICKER_API}/sessions/{session_id}"))
        .bearer_auth(access_token)
        .send()
        .await?;
    Ok(())
}

pub async fn download_bytes(client: &Client, url: &str, token: &str) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("Download failed: {}", resp.status()));
    }
    Ok(resp.bytes().await?.to_vec())
}

// ── Convert Picker createTime to unix timestamp ────────────────────────────────

/// Parse an ISO 8601 string (e.g. "2024-01-01T00:00:00Z") to a unix timestamp string.
pub fn iso_to_unix(iso: &str) -> Option<String> {
    iso.parse::<chrono::DateTime<chrono::Utc>>()
        .ok()
        .map(|dt| dt.timestamp().to_string())
}
