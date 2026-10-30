//! Shared callback preflight and parameterized storage, exercised against SQLite.
use crate::repository::Store;
use serde::Deserialize;
use serde_json::{Value, json};
use tickets_core::github_app::{AppMetadata, CallbackError, validate_callback};

pub async fn preflight<S: Store>(
    store: &S,
    setup_state: Option<&str>,
    encoded_key: Option<&str>,
    state: &str,
    code: &str,
) -> Result<[u8; 32], CallbackError> {
    let key = validate_callback(setup_state, encoded_key, state, code)?;
    let rows: Vec<Value> = store
        .query("SELECT id FROM github_app WHERE id=1", vec![])
        .await
        .map_err(|_| CallbackError::Internal)?;
    if !rows.is_empty() {
        return Err(CallbackError::AlreadyRegistered);
    }
    Ok(key)
}

pub async fn register<S: Store>(
    store: &S,
    app: &AppMetadata,
    ciphertext: &str,
) -> Result<(), CallbackError> {
    // A simultaneous callback may win after preflight. Never replace its credentials.
    let rows: Vec<Value> = store
        .query(
            "INSERT INTO github_app(id,app_id,slug,name,owner_login,client_id,html_url,secrets_ciphertext)
             VALUES(1,?,?,?,?,?,?,?) ON CONFLICT(id) DO NOTHING RETURNING id",
            vec![
                json!(app.app_id),
                json!(app.slug),
                json!(app.name),
                json!(app.owner_login),
                json!(app.client_id),
                json!(app.html_url),
                json!(ciphertext),
            ],
        )
        .await
        .map_err(|_| CallbackError::Internal)?;
    if rows.is_empty() {
        return Err(CallbackError::AlreadyRegistered);
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct StoredApp {
    #[serde(flatten)]
    pub app: AppMetadata,
    pub secrets_ciphertext: String,
}

pub async fn load<S: Store>(store: &S) -> Result<Option<StoredApp>, CallbackError> {
    let rows: Vec<StoredApp> = store
        .query(
            "SELECT app_id,slug,name,owner_login,client_id,html_url,secrets_ciphertext
             FROM github_app WHERE id=1",
            vec![],
        )
        .await
        .map_err(|_| CallbackError::Internal)?;
    Ok(rows.into_iter().next())
}
