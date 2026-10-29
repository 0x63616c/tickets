//! A fake GitHub server and real isolated local Worker/D1 integration.
use super::import_github::{Api, Options, import};
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
            handle.join().expect("fixture server panicked");
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
            let mut moved = std::collections::BTreeMap::<String, Vec<serde_json::Value>>::new();
            let mut closed = std::collections::BTreeMap::<String, String>::new();
            for stream in listener.incoming() {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(mut stream) = stream else {
                    break;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let mut bytes = Vec::new();
                loop {
                    let mut byte = [0];
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    bytes.push(byte[0]);
                    if bytes.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&bytes).into_owned();
                let length = request
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0; length];
                if stream.read_exact(&mut body).is_err() {
                    continue;
                }
                let method = request.split_whitespace().next().unwrap_or("");
                let raw_path = request.split_whitespace().nth(1).unwrap_or("");
                let mapped = raw_path.replace(
                    "/repos/0x63616c/voxelite/issues",
                    "/repos/example/fixture/issues",
                );
                let mapped = mapped.replace(
                    "/repos/0x63616c/quantum/issues",
                    "/repos/example/fixture/issues",
                );
                let path = mapped.as_str();
                let (status, headers, mut body) = if !limited {
                    limited = true;
                    (429, "Retry-After: 0\r\n", json!({}))
                } else if method == "POST" && path.ends_with("/comments") {
                    let mut comment: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    comment["id"] = json!(1000 + moved.values().map(Vec::len).sum::<usize>());
                    moved.entry(path.into()).or_default().push(comment.clone());
                    (201, "", comment)
                } else if method == "PATCH" && path.contains("/issues/") {
                    let update: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    closed.insert(path.into(), update["state_reason"].as_str().unwrap().into());
                    (200, "", update)
                } else if path == "/repos/example/fixture" {
                    (200, "", json!({"full_name":"example/fixture"}))
                } else if path == "/repos/0x63616c/voxelite" {
                    (307, "Location: /repos/0x63616c/quantum\r\n", json!({}))
                } else if path == "/repos/0x63616c/quantum" {
                    (200, "", json!({"full_name":"0x63616c/quantum"}))
                } else if path == "/repos/example/renamed" {
                    (301, "Location: /repos/example/fixture\r\n", json!({}))
                } else if path.starts_with("/repos/example/fixture/issues?")
                    && path.contains("page=2")
                {
                    (
                        200,
                        "",
                        json!([{ "number":3,"title":"Closed history","body":null,"state":"closed","state_reason":"not_planned","closed_at":"2026-10-01T00:00:00Z","html_url":"https://github.com/example/fixture/issues/3","labels":[] }]),
                    )
                } else if path
                    .starts_with("/repos/example/fixture/issues/2/dependencies/blocked_by?")
                    && !path.contains("page=2")
                {
                    (
                        200,
                        "",
                        json!([{"number":1,"html_url":"https://github.com/example/fixture/issues/1"},{"number":99,"html_url":"https://github.com/other/repo/issues/99"}]),
                    )
                } else if path.starts_with("/repos/example/fixture/issues/2/sub_issues?")
                    && !path.contains("page=2")
                {
                    (
                        200,
                        "",
                        json!([{"number":1,"html_url":"https://github.com/example/fixture/issues/1"}]),
                    )
                } else if path.contains("/dependencies/blocked_by")
                    || path.contains("/sub_issues")
                    || path.contains("page=2")
                    || path.contains("page=3")
                {
                    (200, "", json!([]))
                } else if path.starts_with("/repos/example/fixture/issues?") {
                    (
                        200,
                        "",
                        json!([
                            {"number":1,"title":"Blocker","body":"**Markdown**","state":"open","html_url":"https://github.com/example/fixture/issues/1","labels":[{"name":"model:sol","color":"abcdef"}]},
                            {"number":2,"title":"Blocked issue","body":"Depends on #1\nUnknown #99","state":"open","html_url":"https://github.com/example/fixture/issues/2","labels":[{"name":"status:blocked","color":"123456"}]},
                            {"number":4,"pull_request":{},"title":"Excluded PR"}
                        ]),
                    )
                } else if path.starts_with("/repos/example/fixture/issues/2/comments?") {
                    (
                        200,
                        "",
                        json!([{"id":42,"body":"A **comment**","user":{"login":"octocat"},"created_at":"2026-10-01T00:00:00Z"},{"id":43,"body":"Same content"},{"id":44,"body":"Same content"}]),
                    )
                } else if path.starts_with("/repos/example/fixture/issues/1/comments?")
                    || path.starts_with("/repos/example/fixture/issues/3/comments?")
                {
                    (200, "", json!([]))
                } else {
                    (404, "", json!({}))
                };
                if method == "GET"
                    && let Some(items) = body.as_array_mut()
                {
                    if path.contains("/comments?") && !path.contains("page=2") {
                        let base = path.split('?').next().unwrap();
                        items.extend(moved.get(base).into_iter().flatten().cloned());
                    } else if path.starts_with("/repos/example/fixture/issues?") {
                        for issue in items {
                            let base = format!("/repos/example/fixture/issues/{}", issue["number"]);
                            if let Some(reason) = closed.get(&base) {
                                issue["state"] = json!("closed");
                                issue["state_reason"] = json!(reason);
                            }
                        }
                    }
                }
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
// Forward to the isolated Worker, then lose successful PUT/PATCH responses.
// This exercises retries after commit, rather than only pre-write failures.
fn proxy() -> Result<Fixture> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?.to_string();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let handle = thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let mut failed = std::collections::BTreeSet::new();
        for stream in listener.incoming() {
            if flag.load(Ordering::Relaxed) {
                break;
            }
            let Ok(mut stream) = stream else {
                break;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let mut byte = [0];
                if stream.read_exact(&mut byte).is_err() {
                    break None;
                }
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break Some(bytes.len());
                }
            };
            let Some(header_end) = header_end else {
                continue;
            };
            let headers = String::from_utf8_lossy(&bytes).into_owned();
            let mut parts = headers.split_whitespace();
            let method = parts.next().unwrap();
            let path = parts.next().unwrap();
            let mut request = client.request(
                Method::from_bytes(method.as_bytes()).unwrap(),
                format!("http://127.0.0.1:8795{path}"),
            );
            let mut length = 0;
            for line in headers.lines().skip(1) {
                if let Some((key, value)) = line.split_once(':') {
                    if key.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap();
                    }
                    if key.eq_ignore_ascii_case("authorization")
                        || key.eq_ignore_ascii_case("content-type")
                    {
                        request = request.header(key, value.trim());
                    }
                }
            }
            bytes.resize(header_end + length, 0);
            stream.read_exact(&mut bytes[header_end..]).unwrap();
            let response = request.body(bytes[header_end..].to_vec()).send().unwrap();
            let original_status = response.status().as_u16();
            let mut body = response.bytes().unwrap().to_vec();
            let status = if original_status < 300
                && method == "POST"
                && path.ends_with("/comments")
                && failed.insert("comment".into())
            {
                body = b"{}".to_vec();
                503
            } else if original_status < 300
                && matches!(method, "GET" | "PUT" | "PATCH")
                && failed.insert(method.to_owned())
            {
                body = b"{}".to_vec();
                if method == "GET" { 429 } else { 503 }
            } else {
                original_status
            };
            write!(stream, "HTTP/1.1 {status} Fixture\r\nRetry-After: 0\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            let _ = stream.write_all(&body);
        }
        assert!(
            failed.contains("GET")
                && failed.contains("PUT")
                && failed.contains("PATCH")
                && failed.contains("comment")
        );
    });
    Ok(Fixture {
        address,
        stop,
        handle: Some(handle),
    })
}
/// Quantum's original and renamed GitHub repository names.
const ORIGINAL_REPO: &str = "0x63616c/voxelite";
const RENAMED_REPO: &str = "0x63616c/quantum";

pub fn check(token: &str) -> Result<()> {
    let fixture = Fixture::start()?;
    let gh = Api::new(
        &format!("http://{}", fixture.address),
        "fixture".into(),
        true,
    )?;
    let proxy = proxy()?;
    let api = Api::new(&format!("http://{}", proxy.address), token.into(), false)?;
    let renamed = gh.call(
        Method::GET,
        "/repos/example/renamed",
        serde_json::Value::Null,
    )?;
    assert_eq!(renamed["full_name"], "example/fixture");
    assert!(
        import(
            &gh,
            &api,
            "example/renamed",
            "FIX",
            &Options {
                state: "all".into(),
                ..Options::default()
            }
        )
        .is_err()
    );
    assert!(
        import(&gh, &api, "example/fixture", "FIX", &Options::default()).is_err(),
        "missing destination project must not be created"
    );
    let direct = Api::new("http://127.0.0.1:8795", token.into(), false)?;
    for key in ["FIX", "QUA"] {
        direct.call(
            Method::POST,
            "/api/v1/projects",
            json!({"name":key,"key_prefix":key,"color":"#789abc"}),
        )?;
    }
    import(
        &gh,
        &api,
        "example/fixture",
        "FIX",
        &Options {
            dry: true,
            state: "all".into(),
            ..Options::default()
        },
    )?;
    assert!(
        api.list("/api/v1/projects")?.len() == 2,
        "dry run wrote a project"
    );
    assert!(
        import(
            &gh,
            &api,
            "example/fixture",
            "FIX",
            &Options {
                state: "all".into(),
                ..Options::default()
            }
        )
        .is_err(),
        "comment POST must stop after ambiguous failure"
    );
    import(
        &gh,
        &api,
        "example/fixture",
        "FIX",
        &Options {
            state: "all".into(),
            ..Options::default()
        },
    )?;
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
    import(
        &gh,
        &api,
        "example/fixture",
        "FIX",
        &Options {
            state: "all".into(),
            ..Options::default()
        },
    )?;
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
    assert_eq!(api.list("/api/v1/projects")?.len(), 2);
    assert_eq!(api.list("/api/v1/labels")?.len(), 2);
    // A partial import must tolerate previously imported issues outside its selection.
    api.call(
        Method::PUT,
        "/api/v1/tickets/by-external?id=github:example/fixture:99",
        json!({"project_id":first[0]["project_id"], "title":"Unexpected imported ticket"}),
    )?;
    import(
        &gh,
        &api,
        "example/fixture",
        "FIX",
        &Options {
            state: "all".into(),
            ..Options::default()
        },
    )?;
    import(
        &gh,
        &api,
        ORIGINAL_REPO,
        "QUA",
        &Options {
            state: "all".into(),
            ..Options::default()
        },
    )?;
    let before_rename = api.list("/api/v1/tickets")?;
    import(
        &gh,
        &api,
        RENAMED_REPO,
        "QUA",
        &Options {
            state: "all".into(),
            ..Options::default()
        },
    )?;
    let after_rename = api.list("/api/v1/tickets")?;
    assert_eq!(before_rename.len(), after_rename.len());
    for (before, after) in before_rename.iter().zip(&after_rename) {
        assert_eq!(before["id"], after["id"]);
        assert_eq!(before["external_id"], after["external_id"]);
        let tid = before["id"].as_i64().ok_or("missing ID")?;
        let comments = api.list(&format!("/api/v1/tickets/{tid}/comments"))?;
        assert!(comments.len() <= 3, "rename duplicated comments");
    }
    let close_options = Options {
        state: "all".into(),
        close_github: true,
        ..Options::default()
    };
    import(&gh, &api, "example/fixture", "FIX", &close_options)?;
    import(&gh, &api, "example/fixture", "FIX", &close_options)?;
    let after_close = api.list("/api/v1/tickets")?;
    for ticket in &first {
        let after = after_close
            .iter()
            .find(|after| after["id"] == ticket["id"])
            .unwrap();
        assert_eq!(
            ticket["status"], after["status"],
            "archival GitHub close changed native task status on rerun"
        );
    }
    for n in 1..=3 {
        let discussion = gh.list(&format!("/repos/example/fixture/issues/{n}/comments"))?;
        assert_eq!(
            discussion
                .iter()
                .filter(|comment| comment["body"]
                    .as_str()
                    .is_some_and(|body| body.starts_with("Moved to Tickets: ")))
                .count(),
            1,
            "duplicate close comment"
        );
    }
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
    fn connection_drop_retries_safe_write_only() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let handle = thread::spawn(move || {
            for attempt in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut bytes = [0; 4096];
                let _ = stream.read(&mut bytes).unwrap();
                if attempt == 1 {
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                        )
                        .unwrap();
                }
            }
        });
        let api = Api::new(&format!("http://{address}"), "test".into(), false)?;
        api.call(
            Method::PUT,
            "/api/v1/tickets/by-external?id=test:1",
            json!({"title":"Test"}),
        )?;
        handle.join().unwrap();
        Ok(())
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
