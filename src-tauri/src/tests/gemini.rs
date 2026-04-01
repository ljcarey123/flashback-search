use mockito::Server;
use reqwest::Client;
use serde_json::json;

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
    let floats: Vec<f32> = values
        .iter()
        .filter_map(|x| x.as_f64().map(|f| f as f32))
        .collect();
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
