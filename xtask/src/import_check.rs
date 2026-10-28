//! A fake GitHub server and real isolated local Worker/D1 integration.
use super::import_github::{Api, import};
use reqwest::Method;
use serde_json::json;
use std::{
    error::Error,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
struct Fixture {
    address: String,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(&self.address);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
impl Fixture {
    fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?.to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = thread::spawn(move || {
            let mut limited = false;
            for stream in listener.incoming() {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(mut stream) = stream else {
                    break;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let mut bytes = [0; 8192];
                let Ok(len) = stream.read(&mut bytes) else {
                    continue;
                };
                let request = String::from_utf8_lossy(&bytes[..len]);
                let path = request.split_whitespace().nth(1).unwrap_or("");
                let (status, headers, body) = if !limited {
                    limited = true;
                    (429, "Retry-After: 0\r\n", json!({}))
                } else if path.starts_with("/repos/example/fixture/issues?")
                    && path.contains("page=2")
                {
                    (
                        200,
                        "",
                        json!([{ "number":3,"title":"Closed history","body":null,"state":"closed","html_url":"https://github.com/example/fixture/issues/3","labels":[] }]),
                    )
                } else if path.contains("page=2") || path.contains("page=3") {
                    (200, "", json!([]))
                } else if path.starts_with("/repos/example/fixture/issues?") {
                    (
                        200,
                        "",
                        json!([
                            {"number":1,"title":"Blocker","body":"**Markdown**","state":"open","html_url":"https://github.com/example/fixture/issues/1","labels":[{"name":"model:sol","color":"abcdef"}]},
                            {"number":2,"title":"Blocked issue","body":"Blocked by #1","state":"open","html_url":"https://github.com/example/fixture/issues/2","labels":[{"name":"status:blocked","color":"123456"}]},
                            {"number":4,"pull_request":{},"title":"Excluded PR"}
                        ]),
                    )
                } else if path.starts_with("/repos/example/fixture/issues/2/comments?") {
                    (
                        200,
                        "",
                        json!([{"id":42,"body":"A **comment**"},{"id":43,"body":"Same content"},{"id":44,"body":"Same content"}]),
                    )
                } else if path.starts_with("/repos/example/fixture/issues/1/comments?")
                    || path.starts_with("/repos/example/fixture/issues/3/comments?")
                {
                    (200, "", json!([]))
                } else {
                    (404, "", json!({}))
                };
                let body = body.to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} Fixture\r\n{headers}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Ok(Self {
            address,
            stop,
            handle: Some(handle),
        })
    }
}
pub fn check(token: &str) -> Result<()> {
    let fixture = Fixture::start()?;
    let gh = Api::new(
        &format!("http://{}", fixture.address),
        "fixture".into(),
        true,
    )?;
    let api = Api::new("http://127.0.0.1:8795", token.into(), false)?;
    import(&gh, &api, "example/fixture", "FIX", true)?;
    assert!(
        api.list("/api/v1/projects")?.is_empty(),
        "dry run wrote a project"
    );
    import(&gh, &api, "example/fixture", "FIX", false)?;
    let first = api.list("/api/v1/tickets")?;
    assert_eq!(first.len(), 3);
    assert_eq!(api.list("/api/v1/tickets?blocked=true")?.len(), 1);
    let ids: Vec<_> = first.iter().map(|t| t["id"].clone()).collect();
    let tid = first[1]["id"].as_i64().ok_or("missing ID")?;
    let original_comments = api.list(&format!("/api/v1/tickets/{tid}/comments"))?;
    assert_eq!(original_comments.len(), 3);
    // Deliberately drift imported fields, labels, edges and a comment; rerun repairs.
    api.call(
        Method::PATCH,
        &format!("/api/v1/tickets/{tid}"),
        json!({"title":"Drift","body":"Drift","status":"done"}),
    )?;
    api.call(
        Method::PATCH,
        &format!("/api/v1/comments/{}", original_comments[0]["id"]),
        json!({"body":"<!-- tickets-import:github:example/fixture:comment:42 -->\n\nChanged"}),
    )?;
    let label = first[1]["labels"][0]["id"]
        .as_i64()
        .ok_or("missing label")?;
    api.call(
        Method::PATCH,
        &format!("/api/v1/labels/{label}"),
        json!({"color":"#ffffff"}),
    )?;
    let blocker = first[0]["id"].as_i64().ok_or("missing blocker")?;
    api.call(
        Method::DELETE,
        &format!("/api/v1/tickets/{blocker}/dependencies/{tid}"),
        serde_json::Value::Null,
    )?;
    api.call(
        Method::POST,
        &format!("/api/v1/tickets/{tid}/dependencies/{blocker}"),
        serde_json::Value::Null,
    )?;
    let extra_label = first[0]["labels"][0]["id"]
        .as_i64()
        .ok_or("missing label")?;
    api.call(
        Method::POST,
        &format!("/api/v1/tickets/{tid}/labels/{extra_label}"),
        serde_json::Value::Null,
    )?;
    import(&gh, &api, "example/fixture", "FIX", false)?;
    let second = api.list("/api/v1/tickets")?;
    for (a, b) in first.iter().zip(&second) {
        for field in [
            "id",
            "created_at",
            "key",
            "title",
            "body",
            "status",
            "labels",
            "blocks",
            "blocked_by",
            "external_id",
            "external_url",
        ] {
            assert_eq!(a[field], b[field], "rerun changed {field}");
        }
    }
    assert_eq!(
        second.iter().map(|t| t["id"].clone()).collect::<Vec<_>>(),
        ids
    );
    let final_comments = api.list(&format!("/api/v1/tickets/{tid}/comments"))?;
    assert_eq!(final_comments.len(), 3);
    for (a, b) in original_comments.iter().zip(&final_comments) {
        assert_eq!(a["id"], b["id"]);
        assert_eq!(a["body"], b["body"]);
    }
    assert_eq!(api.list("/api/v1/projects")?.len(), 1);
    assert_eq!(api.list("/api/v1/labels")?.len(), 2);
    // Reconciliation must fail rather than silently count an extra imported ticket.
    api.call(
        Method::PUT,
        "/api/v1/tickets/by-external?id=github:example/fixture:99",
        json!({"project_id":first[0]["project_id"], "title":"Unexpected imported ticket"}),
    )?;
    assert!(
        import(&gh, &api, "example/fixture", "FIX", false).is_err(),
        "reconciliation ignored an extra ticket"
    );
    println!(
        "PASS: local GitHub pagination/rate limit, PR exclusion, dry run, open/closed, labels/colors, dependency direction, comments, rerun repair and stable identities"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_nonlocal_tickets() {
        for url in [
            "https://tickets.worldwidewebb.co",
            "http://example.com",
            "http://127.0.0.1@evil.test",
            "http://localhost/path",
        ] {
            assert!(Api::new(url, "test".into(), false).is_err());
        }
    }
    #[test]
    fn fixture_paginates_and_retries() -> Result<()> {
        let fixture = Fixture::start()?;
        let api = Api::new(&format!("http://{}", fixture.address), "test".into(), true)?;
        let issues = api.list("/repos/example/fixture/issues?state=all")?;
        assert_eq!(issues.len(), 4);
        assert!(issues[2].get("pull_request").is_some());
        Ok(())
    }
}
