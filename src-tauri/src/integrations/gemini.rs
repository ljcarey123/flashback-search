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
