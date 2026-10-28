//! Signed fixtures exercise the deployed wasm adapter against local D1 only.
use super::import_github::{Api, import};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub struct Fixture {
    pub base: String,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.base.trim_start_matches("http://"));
        if let Some(h) = self.handle.take() {
            h.join().unwrap();
        }
    }
}
fn issue() -> Value {
    json!({"number":1,"title":"Keep GitHub issues in sync","body":"All issues, **full content**.\n\n- [x] GitHub stays primary\n- [ ] Keep the mirror fresh","state":"open","html_url":"https://github.com/0x63616c/tickets/issues/39","labels":[{"name":"type:feat","color":"789abc"}],"assignees":[{"login":"fixture-agent"}],"updated_at":"2026-10-01T00:00:00Z"})
}
fn comment() -> Value {
    json!({"id":42,"body":"A **GitHub comment**. <script>bad()</script>","user":{"login":"fixture-agent"},"issue_url":"https://api.github.com/repos/0x63616c/tickets/issues/1","updated_at":"2026-10-01T00:00:00Z"})
}
impl Fixture {
    pub fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let base = format!("http://{}", listener.local_addr()?);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(mut stream) = stream else { break };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let mut bytes = [0; 8192];
                let Ok(n) = stream.read(&mut bytes) else {
                    continue;
                };
                let request = String::from_utf8_lossy(&bytes[..n]);
                let path = request.split_whitespace().nth(1).unwrap_or("");
                assert!(
                    request.starts_with("GET "),
                    "Mirror must never write to GitHub"
                );
                let conditional = request
                    .to_ascii_lowercase()
                    .contains("if-none-match: \"fixture\"");
                let (status, body) = if conditional {
                    (304, String::new())
                } else if path == "/repos/0x63616c/tickets" {
                    (200, json!({"full_name":"0x63616c/tickets"}).to_string())
                } else if path.contains("page=2") {
                    (200, "[]".into())
                } else if path.contains("/comments") {
                    (200, json!([comment()]).to_string())
                } else {
                    (200, json!([issue()]).to_string())
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nETag: \"fixture\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Ok(Self {
            base,
            stop,
            handle: Some(handle),
        })
    }
}
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
                "sha256=bad".into()
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
    Ok(())
}
fn payload(action: &str) -> Value {
    json!({"repository":{"id":1436584641u64},"action":action,"issue":issue(),"comment":comment()})
}
fn scheduled() -> Result<()> {
    let r = Client::new()
        .get("http://127.0.0.1:8795/__scheduled?time=0")
        .send()?;
    if !r.status().is_success() {
        return Err("scheduled handler failed".into());
    }
    thread::sleep(Duration::from_millis(500));
    Ok(())
}
pub fn check(root: &Path, password: &str, token: &str, fixture: &Fixture) -> Result<()> {
    let api = Api::new("http://127.0.0.1:8795", token.into(), false)?;
    send("ping", payload(""), false, 401)?;
    send("ping", payload(""), true, 200)?;
    let mut unknown = payload("opened");
    unknown["repository"]["id"] = json!(1);
    send("issues", unknown, true, 202)?;
    let mut pr = payload("opened");
    pr["issue"]["pull_request"] = json!({});
    send("issues", pr, true, 202)?;
    assert!(api.list("/api/v1/tickets")?.is_empty());
    send("issues", payload("opened"), true, 204)?;
    send("issues", payload("opened"), true, 204)?;
    for state in ["closed", "open"] {
        let mut p = payload(if state == "closed" {
            "closed"
        } else {
            "reopened"
        });
        p["issue"]["state"] = json!(state);
        send("issues", p, true, 204)?;
        assert_eq!(
            api.list("/api/v1/tickets")?[0]["status"],
            if state == "closed" { "done" } else { "todo" }
        );
    }
    send("issue_comment", payload("created"), true, 204)?;
    send("issue_comment", payload("created"), true, 204)?;
    let mut edited = payload("edited");
    edited["comment"]["body"] = json!("Edited body");
    send("issue_comment", edited, true, 204)?;
    assert!(
        api.list("/api/v1/tickets/1/comments")?[0]["body"]
            .as_str()
            .unwrap()
            .contains("Edited body")
    );
    send("issue_comment", payload("deleted"), true, 204)?;
    assert!(api.list("/api/v1/tickets/1/comments")?.is_empty());
    send(
        "label",
        json!({"repository":{"id":1436584641u64},"action":"edited","label":{"name":"type:feat","color":"ff0000"}}),
        true,
        204,
    )?;
    assert_eq!(api.list("/api/v1/labels")?[0]["color"], "#ff0000");
    send(
        "label",
        json!({"repository":{"id":1436584641u64},"action":"deleted","label":{"name":"type:feat","color":"ff0000"}}),
        true,
        204,
    )?;
    assert!(api.list("/api/v1/labels")?.is_empty());
    scheduled()?;
    scheduled()?;
    scheduled()?;
    assert_eq!(api.list("/api/v1/tickets")?.len(), 1);
    assert_eq!(api.list("/api/v1/tickets/1/comments")?.len(), 1);
    let github = Api::new(&fixture.base, "fixture".into(), true)?;
    import(&github, &api, "0x63616c/tickets", "TICKETS", false)?;
    scheduled()?;
    assert_eq!(api.list("/api/v1/tickets")?.len(), 1);
    assert_eq!(api.list("/api/v1/tickets/1/comments")?.len(), 1);
    screenshots(root, password)?;
    send("issues", payload("deleted"), true, 204)?;
    assert!(api.list("/api/v1/tickets")?.is_empty());
    println!("Mirror check passed: signed webhooks, cron, importer identities and read-only UI");
    Ok(())
}
fn screenshots(root: &Path, password: &str) -> Result<()> {
    use super::design::Browser;
    let mut b = Browser::start_named(root, "mirror-browser")?;
    b.go("http://127.0.0.1:8795/login?next=/ticket%3Fid%3D1")?;
    b.wait("!!document.querySelector('#username')")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("!!document.querySelector('.ticket-detail h1')")?;
    let output = root.join("docs/screenshots/39");
    std::fs::create_dir_all(&output)?;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    if !before {
        b.wait("document.body.innerText.includes('Synced from GitHub') && !document.querySelector('.comment-form') && document.querySelector('[aria-label=Status]').disabled")?;
        b.eval("document.querySelector('.comment .markdown').innerHTML.includes('tickets-import') ? (()=>{throw Error('Visible marker')})() : true")?;
    }
    let prefix = if before { "before" } else { "after" };
    b.screenshot(&output.join(format!("{prefix}-desktop.png")))?;
    b.viewport(390, 900)?;
    b.screenshot(&output.join(format!("{prefix}-mobile.png")))?;
    b.viewport(1440, 900)?;
    b.go("http://127.0.0.1:8795/design/ticket")?;
    b.wait("!!document.querySelector('.ticket-detail h1')")?;
    b.screenshot(&output.join("target-desktop.png"))?;
    Ok(())
}
