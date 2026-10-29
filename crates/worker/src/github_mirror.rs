//! Signed GitHub endpoint retained for pull-request integration. Issue ingestion retired.
use futures_util::StreamExt;
use js_sys::{Array, Object, Reflect, Uint8Array};
use tickets_core::github_mirror as mapping;
use wasm_bindgen_futures::JsFuture;
use worker::{wasm_bindgen::JsCast, *};
const MAX_BODY: usize = 1024 * 1024;
async fn digest(secret: &str, body: &[u8]) -> Result<Vec<u8>> {
    let crypto: web_sys::Crypto =
        Reflect::get(&js_sys::global(), &"crypto".into())?.unchecked_into();
    let subtle = crypto.subtle();
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"HMAC".into())?;
    Reflect::set(&algorithm, &"hash".into(), &"SHA-256".into())?;
    let key = JsFuture::from(subtle.import_key_with_object(
        "raw",
        &Uint8Array::from(secret.as_bytes()),
        &algorithm,
        false,
        &Array::of1(&"sign".into()),
    )?)
    .await?;
    let result =
        JsFuture::from(subtle.sign_with_str_and_u8_array("HMAC", &key.unchecked_into(), body)?)
            .await?;
    Ok(Uint8Array::new(&result).to_vec())
}
pub async fn webhook(mut req: Request, env: &Env) -> Result<Response> {
    if req.method() != Method::Post {
        return Response::error("Method not allowed", 405);
    }
    let secret = match env.secret("GITHUB_WEBHOOK_SECRET") {
        Ok(s) if !s.to_string().is_empty() => s.to_string(),
        _ => return Response::error("Webhook unavailable", 503),
    };
    if req
        .headers()
        .get("Content-Length")?
        .and_then(|s| s.parse::<usize>().ok())
        .is_some_and(|n| n > MAX_BODY)
    {
        return Response::error("Payload too large", 413);
    }
    let signature = req
        .headers()
        .get("X-Hub-Signature-256")?
        .unwrap_or_default();
    if !signature.starts_with("sha256=") || signature.len() != 71 {
        return Response::error("Invalid signature", 401);
    }
    let mut bytes = Vec::new();
    let mut stream = req.stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if chunk.len() > MAX_BODY.saturating_sub(bytes.len()) {
            return Response::error("Payload too large", 413);
        }
        bytes.extend_from_slice(&chunk);
    }
    if !mapping::signature_matches(&signature, &digest(&secret, &bytes).await?) {
        return Response::error("Invalid signature", 401);
    }
    let event = req.headers().get("X-GitHub-Event")?.unwrap_or_default();
    let _: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(payload) => payload,
        Err(_) => return Response::error("Malformed payload", 400),
    };
    if event == "ping" {
        return Response::empty();
    }
    let reason = if matches!(event.as_str(), "issues" | "issue_comment" | "label") {
        "issue mirror retired"
    } else {
        "unsupported event"
    };
    Response::from_json(&serde_json::json!({"ignored": true, "reason": reason}))
        .map(|response| response.with_status(202))
}
