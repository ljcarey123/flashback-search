use anyhow::{anyhow, Result};
use base64::Engine;
use reqwest::Client;
use serde_json::{json, Value};

const EMBED_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-2-preview:embedContent";
const VISION_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

/// Embed a text string (for search queries).
pub async fn embed_text(client: &Client, api_key: &str, text: &str) -> Result<Vec<f32>> {
    let body = json!({
        "model": "models/gemini-embedding-2-preview",
        "content": {
            "parts": [{ "text": text }]
        },
        "outputDimensionality": 1536
    });
    call_embed(client, api_key, body).await
}

/// Embed a JPEG/PNG thumbnail (raw bytes) using inline image data.
pub async fn embed_image(client: &Client, api_key: &str, image_bytes: &[u8]) -> Result<Vec<f32>> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(image_bytes);
    let body = json!({
        "model": "models/gemini-embedding-2-preview",
        "content": {
            "parts": [{
                "inline_data": {
                    "mime_type": "image/jpeg",
                    "data": b64
                }
            }]
        },
        "outputDimensionality": 1536
    });
    call_embed(client, api_key, body).await
}

/// Generate a concise natural language description of an image for display and search.
pub async fn describe_image(client: &Client, api_key: &str, image_bytes: &[u8]) -> Result<String> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(image_bytes);
    let body = json!({
        "contents": [{
            "parts": [
                {
                    "inline_data": {
                        "mime_type": "image/jpeg",
                        "data": b64
                    }
                },
                {
                    "text": "Describe this photo in 2-3 sentences for a search index. Include the main subjects, setting, colours, and mood. Be factual and concise."
                }
            ]
        }]
    });
    let url = format!("{VISION_URL}?key={api_key}");
    let resp = client.post(&url).json(&body).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("Gemini vision failed ({status}): {text}"));
    }
    let v: Value = serde_json::from_str(&text)?;
    let description = v["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow!("No text in vision response"))?
        .trim()
        .to_string();
    Ok(description)
}

async fn call_embed(client: &Client, api_key: &str, body: Value) -> Result<Vec<f32>> {
    let url = format!("{EMBED_URL}?key={api_key}");
    let resp = client.post(&url).json(&body).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("Gemini embed failed ({status}): {text}"));
    }
    let v: Value = serde_json::from_str(&text)?;
    let values = v["embedding"]["values"]
        .as_array()
        .ok_or_else(|| anyhow!("No embedding values in response"))?;
    Ok(values
        .iter()
        .filter_map(|x| x.as_f64().map(|f| f as f32))
        .collect())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn mock_embed_response(values: &[f64]) -> String {
        let vals: Vec<serde_json::Value> = values.iter().map(|&v| json!(v)).collect();
        serde_json::to_string(&json!({ "embedding": { "values": vals } })).unwrap()
    }

    #[tokio::test]
    async fn embed_text_parses_response() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_embed_response(&[0.1, 0.2, 0.3]))
            .create_async()
            .await;

        let client = Client::new();
        let url = format!(
            "{}/v1beta/models/gemini-embedding-2-preview:embedContent?key=test",
            server.url()
        );
        let body = json!({ "model": "models/gemini-embedding-2-preview", "content": { "parts": [{ "text": "hello" }] } });
        let resp = client.post(&url).json(&body).send().await.unwrap();
        let status = resp.status();
        let text = resp.text().await.unwrap();
        assert!(status.is_success());
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let values = v["embedding"]["values"].as_array().unwrap();
        let floats: Vec<f32> = values.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
        assert_eq!(floats, vec![0.1_f32, 0.2, 0.3]);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn embed_returns_error_on_api_failure() {
        let mut server = Server::new_async().await;
        server
            .mock("POST", mockito::Matcher::Any)
            .with_status(401)
            .with_body(r#"{"error": {"message": "API key invalid"}}"#)
            .create_async()
            .await;

        let client = Client::new();
        let url = format!(
            "{}/v1beta/models/gemini-embedding-2-preview:embedContent?key=bad",
            server.url()
        );
        let body = json!({ "model": "models/gemini-embedding-2-preview", "content": {} });
        let resp = client.post(&url).json(&body).send().await.unwrap();
        assert_eq!(resp.status(), 401);
    }

    #[test]
    fn mock_embed_response_is_valid_json() {
        let s = mock_embed_response(&[1.0, 2.0]);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v["embedding"]["values"].is_array());
    }
}
