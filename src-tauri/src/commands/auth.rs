use tauri::State;

use crate::{db, integrations::google, secrets};

use super::AppState;

/// Combined auth flow: starts a localhost redirect server, opens the browser,
/// waits up to 2 minutes for the user to sign in, then exchanges the code and
/// stores the tokens.  Returns the user's display name.
#[tauri::command]
pub async fn start_auth_flow(
    client_id: String,
    client_secret: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // 1. Bind a random localhost port for the OAuth redirect
    let (port, code_rx) = google::start_oauth_server()
        .await
        .map_err(|e| e.to_string())?;
    let redirect_uri = format!("http://127.0.0.1:{port}");

    // 2. Persist client_id (non-sensitive) for display
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::set_setting(&db, "client_id", &client_id).map_err(|e| e.to_string())?;
    }

    // 3. Open the consent URL in the default browser
    let url = google::auth_url(&client_id, &redirect_uri);
    open::that(&url).map_err(|e| format!("Failed to open browser: {e}"))?;

    // 4. Wait for the redirect (2-minute timeout)
    let code = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        code_rx,
    )
    .await
    .map_err(|_| "Sign-in timed out after 2 minutes. Please try again.")?
    .map_err(|_| "OAuth server closed unexpectedly")?;

    // 5. Exchange code → tokens
    let tokens = google::exchange_code(
        &state.http,
        &client_id,
        &client_secret,
        &redirect_uri,
        &code,
    )
    .await
    .map_err(|e| e.to_string())?;

    secrets::set_access_token(&tokens.access_token).map_err(|e| e.to_string())?;
    if let Some(rt) = &tokens.refresh_token {
        secrets::set_refresh_token(rt).map_err(|e| e.to_string())?;
    }
    secrets::set_client_secret(&client_secret).map_err(|e| e.to_string())?;

    // 6. Fetch user profile
    let profile = google::get_user_profile(&state.http, &tokens.access_token)
        .await
        .map_err(|e| e.to_string())?;
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::set_setting(&db, "user_name", &profile.name).map_err(|e| e.to_string())?;
    }

    Ok(profile.name)
}

#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let has_token = secrets::get_access_token()
        .map(|t| t.is_some())
        .unwrap_or(false);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let user_name = db::get_setting(&db, "user_name").map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "authenticated": has_token,
        "user_name": user_name,
    }))
}

#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>) -> Result<(), String> {
    secrets::clear_auth().map_err(|e| e.to_string())?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM settings WHERE key='user_name'", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
