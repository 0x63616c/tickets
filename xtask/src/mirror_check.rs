//! Regression check: signed issue webhooks cannot change native imported tickets.
use super::import_github::Api;
use reqwest::{Method, blocking::Client};
use serde_json::{Value, json};
use std::{error::Error, path::Path};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn signature(body: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    // RFC 2104 HMAC, fixture key only. Production uses WebCrypto.
    let mut key = [0; 64];
    key[..14].copy_from_slice(b"mirror-fixture");
    let inner: Vec<_> = key.iter().map(|b| b ^ 0x36).collect();
    let outer: Vec<_> = key.iter().map(|b| b ^ 0x5c).collect();
    let mut hash = Sha256::new();
    hash.update(inner);
    hash.update(body);
    let digest = hash.finalize();
    let mut hash = Sha256::new();
    hash.update(outer);
    hash.update(digest);
    format!("sha256={}", tickets_core::auth::hex(&hash.finalize()))
}
fn send(event: &str, payload: Value, valid: bool, expected: u16) -> Result<()> {
    let body = payload.to_string();
    let response = Client::new()
        .post("http://127.0.0.1:8795/api/v1/github/webhook")
        .header("X-GitHub-Event", event)
        .header(
            "X-Hub-Signature-256",
            if valid {
                signature(body.as_bytes())
            } else {
                format!("sha256={}", "00".repeat(32))
            },
        )
        .body(body)
        .send()?;
    if response.status().as_u16() != expected {
        return Err(format!(
            "{event}: expected {expected}, got {}: {}",
            response.status(),
            response.text()?
        )
        .into());
    }
    if expected == 202 {
        assert_eq!(
            response.json::<Value>()?,
            json!({"ignored":true,"reason":"issue mirror retired"})
        );
    }
    Ok(())
}
pub fn check(root: &Path, password: &str, token: &str) -> Result<()> {
    let api = Api::new("http://127.0.0.1:8795", token.into(), false)?;
    let project = api.call(
        Method::POST,
        "/api/v1/projects",
        json!({"name":"Tickets","key_prefix":"TIK","color":"#789abc"}),
    )?;
    let ticket = api.call(Method::PUT, "/api/v1/tickets/by-external?id=github:0x63616c/tickets:39", json!({"project_id":project["id"],"title":"Imported tickets are native","body":"Edit this ticket in Tickets.","status":"todo","external_url":"https://github.com/0x63616c/tickets/issues/39"}))?;
    api.call(Method::POST, &format!("/api/v1/tickets/{}/comments",ticket["id"]), json!({"body":"<!-- tickets-import:github:0x63616c/tickets:comment:42 -->\n\n**@fixture-agent** on GitHub\n\nOriginal discussion."}))?;
    let before = api.list("/api/v1/tickets")?;
    for event in ["issues", "issue_comment", "label"] {
        for action in ["opened", "edited", "deleted", "transferred"] {
            let payload = json!({"action":action,"repository":{"id":1436584641},"issue":{"number":39,"title":"Overwrite"},"label":{"name":"bug"}});
            send(event, payload.clone(), false, 401)?;
            send(event, payload, true, 202)?;
        }
    }
    send("ping", json!({}), true, 200)?;
    assert_eq!(before, api.list("/api/v1/tickets")?);
    screenshots(root, password)?;
    api.call(
        Method::PATCH,
        &format!("/api/v1/tickets/{}", ticket["id"]),
        json!({"title":"Edited natively","status":"in_progress"}),
    )?;
    assert_eq!(
        api.call(
            Method::GET,
            &format!("/api/v1/tickets/{}", ticket["id"]),
            Value::Null
        )?["title"],
        "Edited natively"
    );
    println!(
        "PASS: signed issue events ignored, imported tickets editable, GitHub attribution preserved"
    );
    Ok(())
}
fn screenshots(root: &Path, password: &str) -> Result<()> {
    use super::design::Browser;
    let mut b = Browser::start_named(root, "imported-ticket-browser")?;
    b.go("http://127.0.0.1:8795/login?next=/ticket%3Fid%3D1")?;
    b.wait("!!document.querySelector('#username')")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("!!document.querySelector('.ticket-detail h1')")?;
    let output = root.join("docs/screenshots/tik-33");
    std::fs::create_dir_all(&output)?;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    if !before {
        b.wait("document.body.innerText.includes('Imported from GitHub 0x63616c/tickets#39') && !!document.querySelector('.comment-form') && !document.querySelector('[aria-label=Status]').disabled")?;
        b.wait("document.querySelector('.comment-heading strong').textContent==='@fixture-agent'")?;
    }
    let prefix = if before { "before" } else { "after" };
    b.screenshot(&output.join(format!("{prefix}-desktop.png")))?;
    b.viewport(390, 900)?;
    b.screenshot(&output.join(format!("{prefix}-mobile.png")))?;
    if !before {
        b.go("http://127.0.0.1:8795/")?;
        b.wait("document.querySelector('.api-ticket')?.getAttribute('draggable')==='true' && !document.querySelector('.card-status select')?.disabled")?;
        b.go("http://127.0.0.1:8795/list")?;
        b.wait("!!document.querySelector('.api-list-row select') && [...document.querySelectorAll('.api-list-row select')].every(s=>!s.disabled)")?;
    }
    Ok(())
}
