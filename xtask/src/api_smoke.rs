//! Isolated local Worker/D1 end-to-end check. Credentials never leave ignored target/.
use serde_json::{Value, json};
use std::{error::Error, fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
struct Client {
    port: u16,
    dir: std::path::PathBuf,
}
impl Client {
    fn call(&self, m: &str, path: &str, body: Value, auth: &str, expected: u16) -> Result<Value> {
        self.raw(
            m,
            path,
            if body.is_null() {
                None
            } else {
                Some(body.to_string())
            },
            auth,
            expected,
        )
    }
    fn raw(
        &self,
        m: &str,
        path: &str,
        body: Option<String>,
        auth: &str,
        expected: u16,
    ) -> Result<Value> {
        let mut cmd = Command::new("curl");
        cmd.args([
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "-X",
            m,
            "-w",
            "\n%{http_code}",
            "-H",
            "Content-Type: application/json",
        ]);
        match auth {
            "bearer" => {
                cmd.arg("-H")
                    .arg(format!("@{}", self.dir.join("bearer.local").display()));
            }
            "cookie" => {
                cmd.arg("-b").arg(self.dir.join("cookies.local"));
                cmd.args(["-H", &format!("Origin: http://127.0.0.1:{}", self.port)]);
            }
            "csrf" => {
                cmd.arg("-b").arg(self.dir.join("cookies.local"));
            }
            "invalid" => {
                cmd.arg("-b").arg(self.dir.join("cookies.local"));
                cmd.args(["-H", "Authorization: Bearer invalid"]);
            }
            "login" => {
                cmd.args([
                    "-H",
                    &format!("Origin: http://127.0.0.1:{}", self.port),
                    "-c",
                ])
                .arg(self.dir.join("cookies.local"));
            }
            _ => {}
        }
        if let Some(body) = body {
            let file = self.dir.join("request.local");
            fs::write(&file, body)?;
            cmd.arg("--data-binary").arg(format!("@{}", file.display()));
        }
        let out = cmd
            .arg(format!("http://127.0.0.1:{}{path}", self.port))
            .output()?;
        if !out.status.success() {
            return Err("local request failed".into());
        }
        let output = String::from_utf8(out.stdout)?;
        let (body, status) = output.rsplit_once('\n').ok_or("missing status")?;
        let status: u16 = status.parse()?;
        if status != expected {
            return Err(format!("{m} {path}: expected {expected}, got {status}: {body}").into());
        }
        let value: Value = if status == 204 && body.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(body)?
        };
        if expected >= 400
            && (!value["error"]["code"].is_string() || !value["error"]["message"].is_string())
        {
            return Err("invalid error envelope".into());
        }
        Ok(value)
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub fn import_check(root: &Path) -> Result<()> {
    run_inner_mode(root, false, false, "import")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn mirror_check(root: &Path) -> Result<()> {
    run_inner_mode(root, false, false, "mirror")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn pickup(root: &Path) -> Result<()> {
    run_inner_mode(root, false, false, "pickup")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn perf(root: &Path) -> Result<()> {
    run_inner_mode(root, false, false, "perf")
}
pub fn run(root: &Path, skill: bool) -> Result<()> {
    run_inner(root, skill, false)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn board(root: &Path) -> Result<()> {
    run_inner(root, false, true)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn board_loading(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "board-loading")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn list(root: &Path) -> Result<()> {
    // Child browser selects the list verification after the shared API seed.
    run_inner_mode(root, false, true, "list")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn detail(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "detail")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn live(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "live")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn projects(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "projects")
}

pub fn mission(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "mission")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn home_loading(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "home-loading")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn login(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "login")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn palette(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "palette")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn polish(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "polish")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn contrast(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "contrast")
}
fn run_inner(root: &Path, skill: bool, board: bool) -> Result<()> {
    run_inner_mode(root, skill, board, "board")
}
fn run_inner_mode(root: &Path, skill: bool, board: bool, _view: &str) -> Result<()> {
    let mut options = crate::local_worker::LaunchOptions::default();
    if _view == "home-loading"
        && options.baseline_assets.is_some()
        && !matches!(options.build, crate::local_worker::Build::Reuse)
    {
        options.build = crate::local_worker::Build::WorkerOnly;
    }
    options.port = if _view == "login" { 8787 } else { 8795 };
    options.mirror_fixture = _view == "mirror";
    let port = options.port;
    let worker = crate::local_worker::LocalWorker::start(root, options)?;
    let password = worker.password();
    let c = Client {
        dir: worker.directory().to_path_buf(),
        port,
    };
    #[cfg(not(target_arch = "wasm32"))]
    if _view != "mirror" {
        let response = reqwest::blocking::Client::new()
            .post(format!("http://127.0.0.1:{port}/api/v1/github/webhook"))
            .body("{}")
            .send()?;
        if response.status().as_u16() != 503 {
            return Err("Unset webhook secret must fail closed".into());
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Public callback must fail closed before auth or any GitHub exchange.
        let response = reqwest::blocking::Client::new()
            .get(format!(
                "http://127.0.0.1:{port}/api/v1/github/app/manifest-callback"
            ))
            .send()?;
        if response.status().as_u16() != 503
            || response
                .headers()
                .get("Cache-Control")
                .and_then(|v| v.to_str().ok())
                != Some("no-store")
            || response
                .headers()
                .get("Referrer-Policy")
                .and_then(|v| v.to_str().ok())
                != Some("no-referrer")
            || !response
                .headers()
                .get("Content-Security-Policy")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| {
                    v.contains("default-src 'none'") && v.contains("style-src 'sha256-")
                })
        {
            return Err("Manifest callback must fail closed with private HTML headers".into());
        }
    }
    c.call("GET", "/api/v1/projects", Value::Null, "", 401)?;
    c.call(
        "POST",
        "/api/v1/login",
        json!({"username":"fixture","password":password}),
        "login",
        200,
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    if _view == "home-loading" {
        for (prefix, name) in [("TKT", "Tickets"), ("QUA", "Quantum"), ("APP", "Apps")] {
            let project = c.call(
                "POST",
                "/api/v1/projects",
                json!({"name":name,"key_prefix":prefix,"color":"#5e6ad2"}),
                "cookie",
                201,
            )?;
            for (title, status) in [
                ("Prepare release", "in_review"),
                ("Review loading", "in_progress"),
            ] {
                c.call(
                    "POST",
                    "/api/v1/tickets",
                    json!({"project_id":project["id"],"title":title,"status":status}),
                    "cookie",
                    201,
                )?;
            }
        }
        return super::home_loading::check(root, password);
    }
    let token = c.call(
        "POST",
        "/api/v1/tokens",
        json!({"name":"API smoke"}),
        "cookie",
        201,
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    if _view == "import" {
        return super::import_check::check(token["token"].as_str().ok_or("missing token")?);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if _view == "mirror" {
        return super::mirror_check::check(
            root,
            password,
            token["token"].as_str().ok_or("missing token")?,
        );
    }
    if board {
        #[cfg(not(target_arch = "wasm32"))]
        return if _view == "mission" {
            super::mission::check(root, password)
        } else if _view == "login" {
            super::login::check(root, password, &c.dir)
        } else if _view == "contrast" {
            super::contrast::check(root, password)
        } else if _view == "palette" {
            super::palette::check(root, password)
        } else if _view == "polish" {
            super::polish::check(root, password)
        } else if _view == "live" {
            super::live::check(
                root,
                password,
                token["token"].as_str().ok_or("missing token")?,
            )
        } else if _view == "projects" {
            super::projects::check(root, password)
        } else {
            super::design::board_check(root, password, _view)
        };
        #[cfg(target_arch = "wasm32")]
        return Err("board-check requires a native host".into());
    }
    if skill {
        return super::skill_check::run(root, token["token"].as_str().ok_or("missing token")?);
    }
    fs::write(
        c.dir.join("bearer.local"),
        format!(
            "Authorization: Bearer {}\n",
            token["token"].as_str().ok_or("missing token")?
        ),
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    if _view == "perf" {
        return super::perf_browser::check(root, password, token["token"].as_str().ok_or("token")?);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if _view == "pickup" {
        return pickup_check(&c, worker.config(), worker.state());
    }
    c.call(
        "PUT",
        "/api/v1/mission/reports/box",
        json!({"jobs":[]}),
        "none",
        401,
    )?;
    c.call("PUT", "/api/v1/mission/reports/box", json!({}), "csrf", 403)?;
    c.call(
        "PUT",
        "/api/v1/mission/reports/box",
        json!({"jobs":[{"state":"running","ticket":"SMK-1"}]}),
        "bearer",
        200,
    )?;
    c.call(
        "PUT",
        "/api/v1/mission/reports/Bad",
        json!({}),
        "bearer",
        422,
    )?;
    c.call(
        "PUT",
        "/api/v1/mission/reports/box",
        json!({"jobs":{}}),
        "bearer",
        422,
    )?;
    c.raw(
        "PUT",
        "/api/v1/mission/reports/box",
        Some(format!("{{\"large\":\"{}\"}}", "x".repeat(512 * 1024))),
        "bearer",
        413,
    )?;
    let mission = c.call("GET", "/api/v1/mission", Value::Null, "cookie", 200)?;
    assert_eq!(mission["reports"][0]["source"], "box");
    assert_eq!(mission["reports"][0]["stale"], false);
    assert_eq!(
        mission["reports"][0]["report"]["jobs"][0]["state"],
        "running"
    );
    c.call("GET", "/api/v1/projects", Value::Null, "invalid", 401)?;
    c.call(
        "POST",
        "/api/v1/projects",
        json!({"name":"No origin"}),
        "csrf",
        403,
    )?;
    c.raw("POST", "/api/v1/projects", Some("{".into()), "bearer", 400)?;
    c.call(
        "POST",
        "/api/v1/projects",
        json!({"name":"Smoke","key_prefix":"SMK","color":"#123456"}),
        "bearer",
        201,
    )?;
    c.call(
        "GET",
        "/api/v1/projects?limit=1",
        Value::Null,
        "cookie",
        200,
    )?;
    c.call("GET", "/api/v1/projects/1", Value::Null, "bearer", 200)?;
    c.call(
        "PATCH",
        "/api/v1/projects/1",
        json!({"archived":true,"github_repo":"example"}),
        "bearer",
        200,
    )?;
    for title in ["First needle", "Second"] {
        c.call(
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":title,"body":"**markdown** <script>bad</script>"}),
            "bearer",
            201,
        )?;
    }
    let a = c.call("GET", "/api/v1/tickets/SMK-1", Value::Null, "bearer", 200)?;
    if a["body_html"]
        .as_str()
        .ok_or("missing html")?
        .contains("<script>")
    {
        return Err("unsafe HTML".into());
    }
    c.call(
        "PATCH",
        "/api/v1/tickets/1",
        json!({"status":"todo","priority":2,"assignee":"agent","sort_order":1}),
        "bearer",
        200,
    )?;
    c.call(
        "POST",
        "/api/v1/labels",
        json!({"name":"smoke","color":"#abcdef"}),
        "bearer",
        201,
    )?;
    c.call("GET", "/api/v1/labels", Value::Null, "bearer", 200)?;
    c.call("GET", "/api/v1/labels/2", Value::Null, "bearer", 200)?;
    c.call(
        "PATCH",
        "/api/v1/labels/2",
        json!({"name":"updated"}),
        "bearer",
        200,
    )?;
    c.call(
        "POST",
        "/api/v1/tickets/1/labels/2",
        Value::Null,
        "bearer",
        200,
    )?;
    c.call(
        "POST",
        "/api/v1/tickets/1/dependencies/2",
        Value::Null,
        "bearer",
        200,
    )?;
    c.call(
        "POST",
        "/api/v1/tickets/2/dependencies/1",
        Value::Null,
        "bearer",
        409,
    )?;
    c.call(
        "POST",
        "/api/v1/tickets/1/dependencies/1",
        Value::Null,
        "bearer",
        409,
    )?;
    c.call(
        "POST",
        "/api/v1/tickets/1/dependencies/2",
        Value::Null,
        "bearer",
        409,
    )?;
    for filter in [
        "project=1",
        "status=todo",
        "label=2",
        "assignee=agent",
        "blocked=true",
        "q=needle",
        "limit=1&cursor=1",
    ] {
        c.call(
            "GET",
            &format!("/api/v1/tickets?{filter}"),
            Value::Null,
            "bearer",
            200,
        )?;
    }
    c.call(
        "POST",
        "/api/v1/tickets/1/comments",
        json!({"body":"**comment**"}),
        "bearer",
        201,
    )?;
    c.call(
        "GET",
        "/api/v1/tickets/1/comments",
        Value::Null,
        "bearer",
        200,
    )?;
    c.call(
        "PATCH",
        "/api/v1/comments/1",
        json!({"body":"updated"}),
        "bearer",
        200,
    )?;
    c.call("GET", "/api/v1/comments/1", Value::Null, "bearer", 200)?;
    c.call("DELETE", "/api/v1/comments/1", Value::Null, "bearer", 200)?;
    let mut imported = Value::Null;
    for title in ["Import", "Updated"] {
        let r = c.call(
            "PUT",
            "/api/v1/tickets/by-external?id=fixture:1",
            json!({"project_id":1,"title":title,"external_url":"https://example.test/1"}),
            "bearer",
            200,
        )?;
        if !imported.is_null() && imported["id"] != r["id"] {
            return Err("upsert duplicated ticket".into());
        }
        imported = r;
    }
    c.call(
        "DELETE",
        "/api/v1/tickets/1/dependencies/2",
        Value::Null,
        "bearer",
        200,
    )?;
    c.call(
        "DELETE",
        "/api/v1/tickets/1/labels/2",
        Value::Null,
        "bearer",
        200,
    )?;
    c.call("DELETE", "/api/v1/labels/2", Value::Null, "bearer", 200)?;
    c.call("DELETE", "/api/v1/tickets/1", Value::Null, "bearer", 200)?;
    c.call("GET", "/api/v1/tickets/1", Value::Null, "bearer", 404)?;
    c.call(
        "POST",
        "/api/v1/labels",
        json!({"name":"","color":"bad"}),
        "bearer",
        422,
    )?;
    c.call(
        "GET",
        "/api/v1/tickets?limit=101",
        Value::Null,
        "bearer",
        400,
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    read_cache_check(&c)?;
    let token_path = format!("/api/v1/tokens/{}", token["metadata"]["id"]);
    c.call("DELETE", &token_path, Value::Null, "cookie", 200)?;
    c.call(
        "DELETE",
        "/api/v1/tokens/999999",
        Value::Null,
        "cookie",
        404,
    )?;
    c.call("GET", "/api/v1/projects", Value::Null, "bearer", 401)?;
    println!(
        "PASS: authenticated project/ticket/label/dependency/comment routes, filters, pagination, conflicts, sanitized Markdown, idempotent external upsert; local state: {}",
        c.dir.display()
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn pickup_check(c: &Client, config: &Path, state: &Path) -> Result<()> {
    c.call("GET", "/api/v1/next", Value::Null, "", 401)?;
    c.call("POST", "/api/v1/next/claim", json!({}), "csrf", 403)?;
    c.call("GET", "/api/v1/next", Value::Null, "bearer", 204)?;
    c.call(
        "POST",
        "/api/v1/projects",
        json!({"name":"Pickup","key_prefix":"PICK","color":"#5e6ad2"}),
        "bearer",
        201,
    )?;
    for title in ["First", "Second"] {
        c.call(
            "POST",
            "/api/v1/tickets",
            json!({"project_id":1,"title":title,"status":"todo"}),
            "bearer",
            201,
        )?;
    }
    let preview = c.call("GET", "/api/v1/next?project=1", Value::Null, "bearer", 200)?;
    if preview["ticket"]["id"] != 1 || !preview["ticket"]["claimed_by"].is_null() {
        return Err("preview mutated or chose wrong ticket".into());
    }
    let header = fs::read_to_string(c.dir.join("bearer.local"))?;
    let token = header
        .strip_prefix("Authorization: Bearer ")
        .ok_or("missing local bearer")?
        .trim()
        .to_string();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let token = token.clone();
            let barrier = barrier.clone();
            let port = c.port;
            std::thread::spawn(move || -> std::result::Result<Value, String> {
                barrier.wait();
                let response = reqwest::blocking::Client::new()
                    .post(format!("http://127.0.0.1:{port}/api/v1/next/claim"))
                    .bearer_auth(token)
                    .json(&json!({"project":1}))
                    .send()
                    .map_err(|_| "claim request failed")?;
                if response.status() != 200 {
                    return Err("concurrent local claim failed".into());
                }
                response.json().map_err(|_| "invalid claim JSON".into())
            })
        })
        .collect();
    let mut claims = vec![];
    for thread in threads {
        claims.push(thread.join().map_err(|_| "claim thread failed")??);
    }
    if claims[0]["ticket"]["id"] == claims[1]["ticket"]["id"] {
        return Err("two D1 claims returned same ticket".into());
    }
    c.call("POST", "/api/v1/next/claim", Value::Null, "bearer", 204)?;
    let first = &claims[0]["ticket"];
    let id = first["id"].as_i64().ok_or("missing claim ticket")?;
    let heartbeat = format!("/api/v1/tickets/{id}/claim/heartbeat");
    let release = format!("/api/v1/tickets/{id}/claim/release");
    let body = json!({"claim_id":first["claim_id"]});
    c.call("POST", &heartbeat, body.clone(), "cookie", 409)?;
    c.call("POST", &heartbeat, body.clone(), "bearer", 200)?;
    c.call("POST", &release, body.clone(), "bearer", 200)?;
    c.call("POST", &heartbeat, body, "bearer", 409)?;
    c.call("POST", "/api/v1/next/claim", Value::Null, "bearer", 200)?;
    // Move the isolated fixture clock only. Never touches remote D1.
    let output=Command::new("wrangler").args(["d1","execute","DB","--local","--config"]).arg(config).arg("--persist-to").arg(state).args(["--command","UPDATE tickets SET claim_expires_at='2000-01-01T00:00:00.000Z' WHERE claimed_by IS NOT NULL"]).output()?;
    if !output.status.success() {
        return Err("local fixture expiry failed".into());
    }
    let response = reqwest::blocking::get(format!(
        "http://127.0.0.1:{}/__scheduled?cron=*+*+*+*+*",
        c.port
    ))?;
    if !response.status().is_success() {
        return Err("local cron failed".into());
    }
    for id in [1, 2] {
        let ticket = c.call(
            "GET",
            &format!("/api/v1/tickets/{id}"),
            Value::Null,
            "bearer",
            200,
        )?;
        if ticket["status"] != "todo" || !ticket["claimed_by"].is_null() {
            return Err("cron failed to reclaim lease".into());
        }
        let history = c.call(
            "GET",
            &format!("/api/v1/tickets/{id}/history"),
            Value::Null,
            "bearer",
            200,
        )?;
        if !history["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["field"] == "expiry")
        {
            return Err("cron omitted history".into());
        }
    }
    println!(
        "PASS pickup D1: authentication, CSRF, empty 204, preview, concurrent HTTP claims, owner fencing, heartbeat, release, cron and history"
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn read_cache_check(c: &Client) -> Result<()> {
    let header = fs::read_to_string(c.dir.join("bearer.local"))?;
    let token = header
        .strip_prefix("Authorization: Bearer ")
        .ok_or("missing bearer")?
        .trim();
    let client = reqwest::blocking::Client::new();
    for path in [
        "/api/v1/projects?limit=100",
        "/api/v1/tickets?limit=100",
        "/api/v1/labels?limit=100",
        "/api/v1/analytics/home",
        "/api/v1/search?q=fixture",
    ] {
        let url = format!("http://127.0.0.1:{}{path}", c.port);
        let first = client
            .get(&url)
            .bearer_auth(token)
            .send()?
            .error_for_status()?;
        let tag = first.headers().get("ETag").ok_or("missing ETag")?.clone();
        let version = first
            .headers()
            .get("X-Workspace-Version")
            .ok_or("missing version")?
            .clone();
        if first
            .headers()
            .get("Cache-Control")
            .and_then(|h| h.to_str().ok())
            != Some("private, no-cache")
        {
            return Err("cache policy must be private".into());
        }
        let first_body = first.text()?;
        let warm = client
            .get(&url)
            .bearer_auth(token)
            .send()?
            .error_for_status()?;
        if warm.text()? != first_body {
            return Err("warm cache changed response".into());
        }
        let unchanged = client
            .get(&url)
            .bearer_auth(token)
            .header("If-None-Match", &tag)
            .send()?;
        if unchanged.status() != 304 || !unchanged.bytes()?.is_empty() {
            return Err(format!("conditional GET failed: {path}").into());
        }
        let denied = client.get(&url).header("If-None-Match", &tag).send()?;
        if denied.status() != 401 {
            return Err("cache bypassed auth".into());
        }
        c.call("POST","/api/v1/labels",json!({"name":format!("cache-{}",tickets_core::auth::hash_token(path)),"color":"#5e6ad2"}),"bearer",201)?;
        let changed = client
            .get(&url)
            .bearer_auth(token)
            .header("If-None-Match", &tag)
            .send()?;
        if changed.status() != 200
            || changed.headers().get("ETag") == Some(&tag)
            || changed.headers().get("X-Workspace-Version") == Some(&version)
        {
            return Err("mutation did not invalidate cache".into());
        }
    }
    let invalid = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/tickets?limit=101",
            c.port
        ))
        .bearer_auth(token)
        .header("If-None-Match", "*")
        .send()?;
    if invalid.status() != 400 {
        return Err("conditional GET bypassed validation".into());
    }
    println!(
        "PASS private read cache: warm bodies, ETag/304, auth, transactional invalidation and validation"
    );
    Ok(())
}
