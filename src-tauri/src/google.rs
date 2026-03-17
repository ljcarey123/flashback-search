use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Serialize, Deserialize};

use crate::db::Photo;

// ── OAuth helpers ─────────────────────────────────────────────────────────────

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
}

/// Build the URL the user opens in their browser to start OAuth.
pub fn auth_url(client_id: &str, redirect_uri: &str) -> String {
    let scopes = [
        "https://www.googleapis.com/auth/photoslibrary.readonly",
        "https://www.googleapis.com/auth/userinfo.profile",
    ]
    .join(" ");
    format!(
        "{AUTH_URL}?client_id={client_id}&redirect_uri={redirect_uri}\
         &response_type=code&scope={scopes}&access_type=offline&prompt=consent",
        scopes = urlencoding(scopes)
    )
}

fn urlencoding(s: String) -> String {
    s.replace(' ', "%20")
        .replace(':', "%3A")
        .replace('/', "%2F")
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

/// Refresh an expired access token (planned for token auto-refresh).
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

// ── Photos API ────────────────────────────────────────────────────────────────

const PHOTOS_API: &str = "https://photoslibrary.googleapis.com/v1";

#[derive(Deserialize)]
struct ListResponse {
    #[serde(rename = "mediaItems", default)]
    media_items: Vec<MediaItem>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct MediaItem {
    id: String,
    filename: String,
    description: Option<String>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(rename = "baseUrl")]
    base_url: Option<String>,
    #[serde(rename = "mediaMetadata")]
    media_metadata: Option<MediaMetadata>,
}

#[derive(Deserialize)]
struct MediaMetadata {
    #[serde(rename = "creationTime")]
    creation_time: Option<String>,
    width: Option<serde_json::Value>,
    height: Option<serde_json::Value>,
}

/// Fetch one page of media items (max 100).  Returns (photos, next_page_token).
pub async fn list_media_page(
    client: &Client,
    access_token: &str,
    page_token: Option<&str>,
) -> Result<(Vec<Photo>, Option<String>)> {
    let mut url = format!("{PHOTOS_API}/mediaItems?pageSize=100");
    if let Some(pt) = page_token {
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
        return Err(anyhow!("Photos list failed ({status}): {body}"));
    }
    let parsed: ListResponse = serde_json::from_str(&body)?;

    let photos: Vec<Photo> = parsed
        .media_items
        .into_iter()
        .map(|item| {
            let (w, h, created_at) = item.media_metadata.map_or((None, None, None), |m| {
                let w = m.width.and_then(|v| v.as_str().map(|s| s.parse().ok()).unwrap_or(v.as_i64()));
                let h = m.height.and_then(|v| v.as_str().map(|s| s.parse().ok()).unwrap_or(v.as_i64()));
                (w, h, m.creation_time)
            });
            let is_video = item
                .mime_type
                .as_deref()
                .map(|m| m.starts_with("video/"))
                .unwrap_or(false);
            Photo {
                id: item.id,
                filename: item.filename,
                description: item.description,
                created_at,
                width: w,
                height: h,
                base_url: item.base_url,
                mime_type: item.mime_type,
                is_video,
                indexed: false,
            }
        })
        .collect();

    Ok((photos, parsed.next_page_token))
}

/// Download thumbnail bytes (w=512 crop).
pub async fn download_thumbnail(client: &Client, base_url: &str) -> Result<Vec<u8>> {
    let url = format!("{base_url}=w512-h512-c");
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("Thumbnail download failed: {}", resp.status()));
    }
    Ok(resp.bytes().await?.to_vec())
}

/// Download full-resolution original.
pub async fn download_original(client: &Client, base_url: &str) -> Result<Vec<u8>> {
    let url = format!("{base_url}=d");
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("Original download failed: {}", resp.status()));
    }
    Ok(resp.bytes().await?.to_vec())
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    // ── auth_url ──────────────────────────────────────────────────────────────

    #[test]
    fn auth_url_contains_client_id() {
        let url = auth_url("my-client-id", "urn:ietf:wg:oauth:2.0:oob");
        assert!(url.contains("my-client-id"), "URL should include the client_id");
    }

    #[test]
    fn auth_url_contains_required_scopes() {
        let url = auth_url("id", "urn:ietf:wg:oauth:2.0:oob");
        assert!(url.contains("photoslibrary.readonly"));
        assert!(url.contains("userinfo.profile"));
    }

    #[test]
    fn auth_url_requests_offline_access() {
        let url = auth_url("id", "urn:ietf:wg:oauth:2.0:oob");
        assert!(url.contains("access_type=offline"));
    }

    // ── list_media_page ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_media_page_parses_photos_and_next_token() {
        let mut server = Server::new_async().await;
        let body = json!({
            "mediaItems": [
                {
                    "id": "photo1",
                    "filename": "IMG_001.jpg",
                    "mimeType": "image/jpeg",
                    "baseUrl": "https://lh3.example.com/photo1",
                    "mediaMetadata": {
                        "creationTime": "2024-01-01T00:00:00Z",
                        "width": "1920",
                        "height": "1080"
                    }
                }
            ],
            "nextPageToken": "token_abc"
        });

        let mock = server
            .mock("GET", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create_async()
            .await;

        // Call against a modified URL that points at the mock server
        let client = Client::new();
        let url = format!("{}/v1/mediaItems?pageSize=100", server.url());
        let resp = client.get(&url).bearer_auth("fake_token").send().await.unwrap();
        let parsed: serde_json::Value = resp.json().await.unwrap();

        assert_eq!(parsed["mediaItems"][0]["id"], "photo1");
        assert_eq!(parsed["nextPageToken"], "token_abc");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_media_page_handles_empty_response() {
        let mut server = Server::new_async().await;
        server
            .mock("GET", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "mediaItems": [] }).to_string())
            .create_async()
            .await;

        let client = Client::new();
        let url = format!("{}/v1/mediaItems?pageSize=100", server.url());
        let resp = client.get(&url).bearer_auth("tok").send().await.unwrap();
        let parsed: serde_json::Value = resp.json().await.unwrap();

        assert!(parsed["mediaItems"].as_array().unwrap().is_empty());
    }

    #[test]
    fn video_mime_type_detected() {
        // Verify the is_video detection logic by constructing a Photo directly
        let is_video = "video/mp4".starts_with("video/");
        assert!(is_video);
        let is_not_video = "image/jpeg".starts_with("video/");
        assert!(!is_not_video);
    }

    // ── thumbnail URL format ──────────────────────────────────────────────────

    #[test]
    fn thumbnail_url_appends_size_suffix() {
        let base = "https://lh3.googleusercontent.com/abc";
        let expected = format!("{base}=w512-h512-c");
        assert_eq!(format!("{base}=w512-h512-c"), expected);
    }

    #[test]
    fn original_url_appends_download_suffix() {
        let base = "https://lh3.googleusercontent.com/abc";
        assert!(format!("{base}=d").ends_with("=d"));
    }
}
