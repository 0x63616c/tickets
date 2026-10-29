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
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub struct Fixture {
    pub base: String,
    stop: Arc<AtomicBool>,
    conditional_hits: Arc<AtomicUsize>,
    missing_lookups: Arc<AtomicUsize>,
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
fn closed_issue() -> Value {
    let mut i = issue();
    i["number"] = json!(2);
    i["title"] = json!("Closed history");
    i["state"] = json!("closed");
    i
}
fn pull_request() -> Value {
    let mut i = issue();
    i["number"] = json!(3);
    i["pull_request"] = json!({});
    i
}
fn pr_comment() -> Value {
    let mut c = comment();
    c["id"] = json!(43);
    c["issue_url"] = json!("https://api.github.com/repos/0x63616c/tickets/issues/3");
    c
}
impl Fixture {
    pub fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let base = format!("http://{}", listener.local_addr()?);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let conditional_hits = Arc::new(AtomicUsize::new(0));
        let hits = conditional_hits.clone();
        let missing_lookups = Arc::new(AtomicUsize::new(0));
        let lookups = missing_lookups.clone();
        let handle = thread::spawn(move || {
            let mut limited = false;
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
                let url = reqwest::Url::parse(&format!("http://fixture{path}")).unwrap();
                let page = url
                    .query_pairs()
                    .find(|(k, _)| k == "page")
                    .and_then(|(_, v)| v.parse::<u32>().ok())
                    .unwrap_or(1);
                let incremental = url.query_pairs().any(|(k, _)| k == "since");
                let fresh = url
                    .query_pairs()
                    .any(|(k, v)| k == "since" && v.as_ref() >= "2026-10-01T00:00:00Z");
                let (status, body) = if incremental && !limited {
                    limited = true;
                    (403, "{}".into())
                } else if conditional {
                    hits.fetch_add(1, Ordering::Relaxed);
                    (304, String::new())
                } else if url.path() == "/repos/0x63616c/tickets" {
                    (200, json!({"full_name":"0x63616c/tickets"}).to_string())
                } else if url.path().ends_with("/issues/3") {
                    lookups.fetch_add(1, Ordering::Relaxed);
                    (200, pull_request().to_string())
                } else if fresh || (!incremental && page > 1) {
                    (200, "[]".into())
                } else if url.path().contains("/issues/1/comments") {
                    (200, json!([comment()]).to_string())
                } else if url.path().contains("/issues/2/comments") {
                    (200, "[]".into())
                } else if url.path().ends_with("/issues/comments") {
                    (
                        200,
                        if page == 1 {
                            json!([comment(), pr_comment()]).to_string()
                        } else {
                            "[]".into()
                        },
                    )
                } else if incremental {
                    (
                        200,
                        if page == 1 {
                            json!([issue(), closed_issue()]).to_string()
                        } else if page == 2 {
                            json!([pull_request()]).to_string()
                        } else {
                            "[]".into()
                        },
                    )
                } else {
                    (
                        200,
                        json!([issue(), closed_issue(), pull_request()]).to_string(),
                    )
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
            conditional_hits,
            missing_lookups,
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
pub(crate) fn send(event: &str, payload: Value, valid: bool, expected: u16) -> Result<()> {
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
    let unsigned = Client::new()
        .post("http://127.0.0.1:8795/api/v1/github/webhook")
        .body("{}")
        .send()?;
    assert_eq!(unsigned.status().as_u16(), 401);
    let oversized = Client::new()
        .post("http://127.0.0.1:8795/api/v1/github/webhook")
        .body("x".repeat(1024 * 1024 + 1))
        .send()?;
    assert_eq!(oversized.status().as_u16(), 413);
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
    send("issues", payload("labeled"), true, 204)?;
    api.call(
        reqwest::Method::POST,
        "/api/v1/labels",
        json!({"name":"renamed","color":"#123456"}),
    )?;
    let renamed = json!({"repository":{"id":1436584641u64},"action":"edited","label":{"name":"renamed","color":"abcdef"},"changes":{"name":{"from":"type:feat"}}});
    send("label", renamed.clone(), true, 204)?;
    send("label", renamed, true, 204)?;
    assert_eq!(api.list("/api/v1/labels")?.len(), 1);
    assert_eq!(
        api.list("/api/v1/tickets")?[0]["labels"][0]["name"],
        "renamed"
    );
    send(
        "label",
        json!({"repository":{"id":1436584641u64},"action":"deleted","label":{"name":"renamed","color":"abcdef"}}),
        true,
        204,
    )?;
    scheduled()?;
    scheduled()?;
    scheduled()?;
    scheduled()?;
    assert_eq!(api.list("/api/v1/tickets")?.len(), 2);
    assert_eq!(api.list("/api/v1/tickets/1/comments")?.len(), 1);
    let github = Api::new(&fixture.base, "fixture".into(), true)?;
    import(&github, &api, "0x63616c/tickets", "TIK", false)?;
    scheduled()?;
    assert_eq!(api.list("/api/v1/tickets")?.len(), 2);
    assert_eq!(api.list("/api/v1/tickets/1/comments")?.len(), 1);
    assert!(
        fixture.conditional_hits.load(Ordering::Relaxed) > 0,
        "ETag must yield 304"
    );
    assert!(
        fixture.missing_lookups.load(Ordering::Relaxed) > 0,
        "PR comments must resolve and skip"
    );
    let original = api.list("/api/v1/tickets/1/comments")?[0]["body"].clone();
    assert!(
        api.call(
            reqwest::Method::POST,
            "/api/v1/tickets/1/comments",
            json!({"body":original})
        )
        .is_err(),
        "SQLite must reject concurrent importer marker duplicates"
    );
    // Keep screenshot seed identical to the main baseline: one imported issue.
    let mut remove_closed = payload("deleted");
    remove_closed["issue"] = closed_issue();
    send("issues", remove_closed, true, 204)?;
    screenshots(root, password)?;
    send("issues", payload("deleted"), true, 204)?;
    assert!(api.list("/api/v1/tickets")?.is_empty());
    super::github_pr_check::check(root, password, &api)?;
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
    if !before {
        b.go("http://127.0.0.1:8795/")?;
        b.wait("document.querySelector('.api-ticket')?.getAttribute('draggable')==='false' && document.querySelector('.card-status select')?.disabled")?;
        b.eval("window.mirrorWrites=0; const realFetch=window.fetch;window.fetch=(...args)=>{if(args[1]?.method==='PATCH')window.mirrorWrites++;return realFetch(...args)};document.querySelector('.api-ticket').focus();true")?;
        b.key(" ", 32, 0)?;
        b.key("ArrowRight", 39, 0)?;
        b.key(" ", 32, 0)?;
        b.wait("window.mirrorWrites===0 && document.querySelector('.api-ticket')?.getAttribute('draggable')==='false'")?;
        b.go("http://127.0.0.1:8795/list")?;
        b.wait("document.querySelector('.api-list-row select')?.disabled && [...document.querySelectorAll('.api-list-row select')].every(s=>s.disabled)")?;
    }
    b.viewport(1440, 900)?;
    b.go("http://127.0.0.1:8795/design/ticket")?;
    b.wait("!!document.querySelector('.ticket-detail h1')")?;
    b.screenshot(&output.join("target-desktop.png"))?;
    for (name, width) in [
        (format!("{prefix}-desktop.png"), 1440u32),
        (format!("{prefix}-mobile.png"), 390u32),
        ("target-desktop.png".into(), 1440u32),
    ] {
        let png = std::fs::read(output.join(name))?;
        assert_eq!(u32::from_be_bytes(png[16..20].try_into()?), width);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into()?), 900);
    }
    Ok(())
}
