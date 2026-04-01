//! Secure credential storage backed by the Windows Credential Manager.
//!
//! Sensitive values (access tokens, refresh tokens, API keys) never touch
//! the SQLite database — they live in the OS keychain and are encrypted at
//! rest by Windows on a per-user basis.

use anyhow::{anyhow, Result};
use keyring::Entry;

const SERVICE: &str = "flashback";

fn entry(key: &str) -> Result<Entry> {
    Entry::new(SERVICE, key).map_err(|e| anyhow!("Keyring entry error: {e}"))
}

pub fn set(key: &str, value: &str) -> Result<()> {
    entry(key)?.set_password(value).map_err(|e| anyhow!("Keyring set '{key}' failed: {e}"))
}

pub fn get(key: &str) -> Result<Option<String>> {
    match entry(key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow!("Keyring get '{key}' failed: {e}")),
    }
}

pub fn delete(key: &str) -> Result<()> {
    match entry(key)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // already gone — fine
        Err(e) => Err(anyhow!("Keyring delete '{key}' failed: {e}")),
    }
}

// Typed helpers so callers never hand-code key strings.

pub fn set_access_token(token: &str) -> Result<()> {
    set("access_token", token)
}
pub fn get_access_token() -> Result<Option<String>> {
    get("access_token")
}

pub fn set_refresh_token(token: &str) -> Result<()> {
    set("refresh_token", token)
}

/// Planned for token auto-refresh (Stage 2+).
#[allow(dead_code)]
pub fn get_refresh_token() -> Result<Option<String>> {
    get("refresh_token")
}

pub fn set_client_secret(secret: &str) -> Result<()> {
    set("client_secret", secret)
}

/// Available for use when re-authenticating with a stored secret.
#[allow(dead_code)]
pub fn get_client_secret() -> Result<Option<String>> {
    get("client_secret")
}

pub fn set_gemini_key(key: &str) -> Result<()> {
    set("gemini_api_key", key)
}
pub fn get_gemini_key() -> Result<Option<String>> {
    get("gemini_api_key")
}

pub fn clear_auth() -> Result<()> {
    delete("access_token")?;
    delete("refresh_token")?;
    Ok(())
}

