use tauri::State;

use crate::secrets;

use super::AppState;

#[tauri::command]
pub async fn debug_token(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let token = secrets::get_access_token()
        .map_err(|e| e.to_string())?
        .ok_or("No access token in keychain")?;

    let token_preview = if token.len() > 12 {
        format!("{}…{}", &token[..6], &token[token.len() - 6..])
    } else {
        "too short".to_string()
    };

    let resp = state
        .http
        .get("https://www.googleapis.com/oauth2/v1/tokeninfo")
        .query(&[("access_token", &token)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "token_preview": token_preview,
        "tokeninfo_status": status,
        "tokeninfo": body,
    }))
}
