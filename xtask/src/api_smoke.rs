//! Isolated local Worker/D1 end-to-end check. Credentials never leave ignored target/.
use serde_json::{Value, json};
use sha2::Sha256;
use std::{
    error::Error,
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = Command::new("kill")
            .args(["-TERM", "--", &format!("-{}", self.0.id())])
            .output();
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
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
        let value: Value = serde_json::from_str(body)?;
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
pub fn run(root: &Path, skill: bool) -> Result<()> {
    run_inner(root, skill, false)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn board(root: &Path) -> Result<()> {
    run_inner(root, false, true)
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

#[cfg(not(target_arch = "wasm32"))]
pub fn settings(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "settings")
}

pub fn first_run(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "first-run")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn login(root: &Path) -> Result<()> {
    run_inner_mode(root, false, true, "login")
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
    super::build(root)?;
    let mut random = [0u8; 24];
    getrandom::getrandom(&mut random).map_err(|_| "randomness unavailable")?;
    let password = tickets_core::auth::hex(&random);
    let dir = root
        .join("target")
        .join(format!("api-smoke-{}", &password[..12]));
    fs::create_dir_all(&dir)?;
    let mut digest = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        &random[..16],
        tickets_core::auth::PASSWORD_ITERATIONS,
        &mut digest,
    );
    fs::write(
        dir.join(".dev.vars"),
        format!(
            "BOOTSTRAP_USERNAME=fixture\nBOOTSTRAP_PASSWORD_HASH=\"pbkdf2-sha256${}${}${}\"\n",
            tickets_core::auth::PASSWORD_ITERATIONS,
            tickets_core::auth::hex(&random[..16]),
            tickets_core::auth::hex(&digest)
        ),
    )?;
    let config = dir.join("wrangler.toml");
    fs::write(
        &config,
        format!(
            "name='tickets-api-smoke'\nmain='{}'\ncompatibility_date='2026-10-27'\n[assets]\ndirectory='{}'\nbinding='ASSETS'\nnot_found_handling='none'\nrun_worker_first=true\n[[d1_databases]]\nbinding='DB'\ndatabase_name='api-smoke'\ndatabase_id='00000000-0000-0000-0000-000000000000'\nmigrations_dir='{}'\n",
            root.join("crates/worker/build/worker/shim.mjs").display(),
            std::env::var_os("TICKETS_BEFORE_ASSETS")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| root.join("crates/web/dist"))
                .display(),
            root.join("migrations").display()
        ),
    )?;
    use std::io::Write;
    fs::OpenOptions::new().append(true).open(&config)?.write_all(b"\n[[durable_objects.bindings]]\nname='BOARD_ROOMS'\nclass_name='BoardRoom'\n[[migrations]]\ntag='v1-board-room'\nnew_sqlite_classes=['BoardRoom']\n")?;
    #[cfg(not(target_arch = "wasm32"))]
    if _view == "mirror" {
        fs::OpenOptions::new()
            .append(true)
            .open(&config)?
            .write_all(b"\n[vars]\nGITHUB_PR_OWNERS='0x63616c'\n")?;
        fs::OpenOptions::new()
            .append(true)
            .open(dir.join(".dev.vars"))?
            .write_all(b"GITHUB_WEBHOOK_SECRET=mirror-fixture\n")?;
    }
    let state = dir.join("state");
    let status = Command::new("wrangler")
        .args(["d1", "migrations", "apply", "DB", "--local", "--config"])
        .arg(&config)
        .arg("--persist-to")
        .arg(&state)
        .stdin(Stdio::null())
        .status()?;
    if !status.success() {
        return Err("local migrations failed".into());
    }
    let log = fs::File::create(dir.join("worker.log"))?;
    let mut server_command = Command::new("wrangler");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        server_command.process_group(0);
    }
    let port = if _view == "login" { 8787 } else { 8795 };
    let mut port_available = false;
    for _ in 0..60 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            port_available = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    if !port_available {
        return Err(format!(
            "Local check port {port} is busy; retry after the other check finishes"
        )
        .into());
    }

    let mut server = Server(
        server_command
            .args([
                "dev",
                "--local",
                "--ip",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--inspector-port",
                "0",
                "--config",
            ])
            .arg(&config)
            .arg("--persist-to")
            .arg(&state)
            .stdout(log.try_clone()?)
            .stderr(log)
            .stdin(Stdio::null())
            .spawn()?,
    );
    let c = Client { dir, port };
    let start = Instant::now();
    loop {
        if c.call("GET", "/api/health", Value::Null, "", 200).is_ok() {
            break;
        }
        if server.0.try_wait()?.is_some() || start.elapsed() > Duration::from_secs(45) {
            return Err(format!(
                "Worker did not start; see {}",
                c.dir.join("worker.log").display()
            )
            .into());
        }
        thread::sleep(Duration::from_millis(300));
    }
    // First-run must reach login with no prior API auth, token or domain writes.
    #[cfg(not(target_arch = "wasm32"))]
    if _view == "settings" {
        return super::settings::check(root, &password);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if _view == "first-run" {
        return super::first_run::check(root, &password);
    }
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
    c.call("GET", "/api/v1/projects", Value::Null, "", 401)?;
    c.call(
        "POST",
        "/api/v1/login",
        json!({"username":"fixture","password":password}),
        "login",
        200,
    )?;
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
            &password,
            token["token"].as_str().ok_or("missing token")?,
        );
    }
    if board {
        #[cfg(not(target_arch = "wasm32"))]
        return if _view == "login" {
            super::login::check(root, &password, &c.dir)
        } else if _view == "contrast" {
            super::contrast::check(root, &password)
        } else if _view == "polish" {
            super::polish::check(root, &password)
        } else if _view == "live" {
            super::live::check(
                root,
                &password,
                token["token"].as_str().ok_or("missing token")?,
            )
        } else if _view == "projects" {
            super::projects::check(root, &password)
        } else {
            super::design::board_check(root, &password, _view)
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
    c.call("GET", "/api/v1/labels/1", Value::Null, "bearer", 200)?;
    c.call(
        "PATCH",
        "/api/v1/labels/1",
        json!({"name":"updated"}),
        "bearer",
        200,
    )?;
    c.call(
        "POST",
        "/api/v1/tickets/1/labels/1",
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
        "label=1",
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
        "/api/v1/tickets/1/labels/1",
        Value::Null,
        "bearer",
        200,
    )?;
    c.call("DELETE", "/api/v1/labels/1", Value::Null, "bearer", 200)?;
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
