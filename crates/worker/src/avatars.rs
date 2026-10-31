//! Authenticated avatar storage. Binary parameters are JSON byte arrays at the Store boundary.
use crate::repository::{Result, Store};
use serde::Deserialize;
use serde_json::{Value, json};

pub const MAX_BYTES: usize = 512 * 1024;
pub const ACCOUNT: &str = "SELECT username,display_name,(SELECT '/api/v1/users/'||user_id||'/avatar?v='||version FROM user_avatars WHERE user_id=users.id) avatar_url FROM users WHERE id=?";

pub fn content_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub async fn save<S: Store>(
    store: &S,
    user: i64,
    mime: &str,
    bytes: &[u8],
) -> std::result::Result<Value, u16> {
    if bytes.len() > MAX_BYTES {
        return Err(413);
    }
    if content_type(bytes) != Some(mime) {
        return Err(415);
    }
    // Content-addressed versions also distinguish rapid replacements within one millisecond.
    let version = tickets_core::auth::hash_token(&tickets_core::auth::hex(bytes));
    let rows: Vec<Value> = store.query("INSERT INTO user_avatars(user_id,content_type,bytes,version) VALUES(?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET content_type=excluded.content_type,bytes=excluded.bytes,version=excluded.version,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') RETURNING user_id",vec![user.into(),mime.into(),json!(bytes),version.into()]).await.map_err(|_|500u16)?;
    if rows.is_empty() {
        return Err(500);
    }
    account(store, user).await.map_err(|_| 500)
}
pub async fn account<S: Store>(store: &S, user: i64) -> Result<Value> {
    let rows: Vec<Value> = store.query(ACCOUNT, vec![user.into()]).await?;
    rows.into_iter()
        .next()
        .ok_or(crate::repository::Error::NotFound)
}
pub async fn remove<S: Store>(store: &S, user: i64) -> Result<Value> {
    let _: Vec<Value> = store
        .query(
            "DELETE FROM user_avatars WHERE user_id=?",
            vec![user.into()],
        )
        .await?;
    account(store, user).await
}
#[derive(Deserialize)]
pub struct Photo {
    pub content_type: String,
    pub bytes: Vec<u8>,
    pub version: String,
}
pub async fn get<S: Store>(store: &S, user: i64) -> Result<Option<Photo>> {
    Ok(store
        .query(
            "SELECT content_type,bytes,version FROM user_avatars WHERE user_id=?",
            vec![user.into()],
        )
        .await?
        .into_iter()
        .next())
}

#[cfg(target_arch = "wasm32")]
pub async fn upload(
    req: &mut worker::Request,
    db: &worker::D1Database,
    user: i64,
) -> worker::Result<worker::Response> {
    use futures_util::StreamExt;
    if req
        .headers()
        .get("Content-Length")?
        .and_then(|s| s.parse::<usize>().ok())
        .is_some_and(|n| n > MAX_BYTES)
    {
        return reject(413);
    }
    let mime = req.headers().get("Content-Type")?.unwrap_or_default();
    let mut bytes = Vec::new();
    let mut stream = req.stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if chunk.len() > MAX_BYTES.saturating_sub(bytes.len()) {
            return reject(413);
        }
        bytes.extend_from_slice(&chunk);
    }
    match save(db, user, &mime, &bytes).await {
        Ok(account) => crate::auth::reply(account, 200),
        Err(status) => reject(status),
    }
}
#[cfg(target_arch = "wasm32")]
fn reject(status: u16) -> worker::Result<worker::Response> {
    crate::auth::reply(
        json!({"error":{"code":if status==413 {"payload_too_large"} else if status==415 {"unsupported_media_type"} else {"internal"},"message":match status {413=>"Processed photo must be 512 KB or smaller.",415=>"Choose a JPEG, PNG or WebP image.",_=>"Could not save photo. Try again."}}}),
        status,
    )
}
#[cfg(target_arch = "wasm32")]
pub async fn serve(
    req: &worker::Request,
    db: &worker::D1Database,
    user: i64,
) -> worker::Result<worker::Response> {
    let Some(photo) = get(db, user)
        .await
        .map_err(|e| worker::Error::RustError(e.to_string()))?
    else {
        return crate::auth::reply(json!({"error":"Photo not found"}), 404);
    };
    let etag = format!("\"{}\"", photo.version);
    let unchanged = req.headers().get("If-None-Match")?.is_some_and(|value| {
        value
            .split(',')
            .any(|v| v.trim() == etag || v.trim() == format!("W/{etag}") || v.trim() == "*")
    });
    let mut response = if unchanged {
        worker::Response::empty()?.with_status(304)
    } else {
        worker::Response::from_bytes(photo.bytes)?
    };
    response
        .headers_mut()
        .set("Content-Type", &photo.content_type)?;
    response.headers_mut().set("ETag", &etag)?;
    response
        .headers_mut()
        .set("Cache-Control", "private, no-cache")?;
    response
        .headers_mut()
        .set("Vary", "Cookie, Authorization")?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    Ok(response)
}
