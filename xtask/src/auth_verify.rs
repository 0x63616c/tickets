//! Local-only integration check; fixture credentials and outputs stay under target/.
use serde_json::{Value, json};
use std::{error::Error, fs, process::Command, time::Instant};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn request(path: &str, method: &str, extra: &[&str]) -> Result<(u16, Value)> {
    let output = Command::new("curl")
        .args(["-sS", "-X", method, "-w", "\n%{http_code}"])
        .args(extra)
        .arg(format!("http://localhost:8787{path}"))
        .output()?;
    if !output.status.success() {
        return Err("curl failed".into());
    }
    let text = String::from_utf8(output.stdout)?;
    let (body, status) = text.rsplit_once('\n').ok_or("missing status")?;
    Ok((status.parse()?, serde_json::from_str(body)?))
}
fn expect(expected: u16, result: (u16, Value)) -> Result<Value> {
    if result.0 != expected {
        return Err(format!("expected {expected}, got {}", result.0).into());
    }
    Ok(result.1)
}
pub fn run() -> Result<()> {
    let origin = ["-H", "Origin: http://localhost:8787"];
    expect(200, request("/api/health", "GET", &[])?)?;
    expect(401, request("/api/v1/me", "GET", &[])?)?;
    expect(
        403,
        request(
            "/api/v1/login",
            "POST",
            &[
                "-H",
                "Content-Type: application/json",
                "--data-binary",
                "@target/login.local",
            ],
        )?,
    )?;
    let start = Instant::now();
    expect(
        200,
        request(
            "/api/v1/login",
            "POST",
            &[
                "-H",
                "Origin: http://localhost:8787",
                "-H",
                "Content-Type: application/json",
                "--data-binary",
                "@target/login.local",
                "-D",
                "target/auth-headers.local",
                "-c",
                "target/auth-cookies.local",
            ],
        )?,
    )?;
    println!("Local WebCrypto login wall time: {:?}", start.elapsed());
    let headers = fs::read_to_string("target/auth-headers.local")?;
    for flag in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"] {
        if !headers.contains(flag) {
            return Err(format!("missing {flag}").into());
        }
    }
    expect(
        200,
        request("/api/v1/me", "GET", &["-b", "target/auth-cookies.local"])?,
    )?;
    expect(
        403,
        request(
            "/api/v1/tokens",
            "POST",
            &[
                "-b",
                "target/auth-cookies.local",
                "-H",
                "Content-Type: application/json",
                "-d",
                "{\"name\":\"fixture\"}",
            ],
        )?,
    )?;
    let created = expect(
        201,
        request(
            "/api/v1/tokens",
            "POST",
            &[
                "-b",
                "target/auth-cookies.local",
                "-H",
                origin[1],
                "-H",
                "Content-Type: application/json",
                "-d",
                "{\"name\":\"Local verification\"}",
            ],
        )?,
    )?;
    let token = created["token"].as_str().ok_or("missing token")?;
    // Put bearer material in an ignored file rather than process arguments.
    fs::write(
        "target/bearer.local",
        format!("Authorization: Bearer {token}\n"),
    )?;
    expect(
        200,
        request("/api/v1/me", "GET", &["-H", "@target/bearer.local"])?,
    )?;
    expect(
        404,
        request("/api/future", "GET", &["-H", "@target/bearer.local"])?,
    )?;
    expect(401, request("/api/future", "GET", &[])?)?;
    let list = expect(
        200,
        request(
            "/api/v1/tokens",
            "GET",
            &["-b", "target/auth-cookies.local"],
        )?,
    )?;
    if list.to_string().contains(token) || list.to_string().contains("token_hash") {
        return Err("list leaked credential".into());
    }
    if list[0]["last_used_at"].is_null() {
        return Err("last_used_at missing".into());
    }
    expect(
        200,
        request(
            &format!("/api/v1/tokens/{}", created["metadata"]["id"]),
            "DELETE",
            &["-H", "@target/bearer.local"],
        )?,
    )?;
    expect(
        401,
        request("/api/v1/me", "GET", &["-H", "@target/bearer.local"])?,
    )?;
    expect(
        401,
        request(
            "/api/v1/me",
            "GET",
            &[
                "-H",
                "@target/bearer.local",
                "-b",
                "target/auth-cookies.local",
            ],
        )?,
    )?;
    expect(
        200,
        request(
            "/api/v1/logout",
            "POST",
            &["-b", "target/auth-cookies.local", "-H", origin[1]],
        )?,
    )?;
    expect(
        401,
        request("/api/v1/me", "GET", &["-b", "target/auth-cookies.local"])?,
    )?;
    // Fresh browser session for the metadata-only screenshot.
    expect(
        200,
        request(
            "/api/v1/login",
            "POST",
            &[
                "-H",
                origin[1],
                "-H",
                "Content-Type: application/json",
                "--data-binary",
                "@target/login.local",
                "-D",
                "target/auth-headers.local",
            ],
        )?,
    )?;
    let headers = fs::read_to_string("target/auth-headers.local")?;
    let cookie = headers
        .lines()
        .find_map(|l| {
            l.strip_prefix("set-cookie: ")
                .or_else(|| l.strip_prefix("Set-Cookie: "))
        })
        .ok_or("cookie missing")?
        .split(';')
        .next()
        .ok_or("cookie missing")?
        .trim_start_matches("tickets_session=");
    fs::write("target/browser-storage.local",json!({"cookies":[{"name":"tickets_session","value":cookie,"domain":"localhost","path":"/","expires":-1,"httpOnly":true,"secure":true,"sameSite":"Lax"}],"origins":[]}).to_string())?;
    // Rate limit includes successful attempts and is enforced before hashing.
    for _ in 0..3 {
        expect(
            401,
            request(
                "/api/v1/login",
                "POST",
                &[
                    "-H",
                    origin[1],
                    "-H",
                    "Content-Type: application/json",
                    "-d",
                    "{\"username\":\"invalid\",\"password\":\"invalid\"}",
                ],
            )?,
        )?;
    }
    expect(
        429,
        request("/api/v1/login", "POST", &["-H", origin[1], "-d", "{}"])?,
    )?;
    println!(
        "PASS: health, login, secure cookie, session, CSRF, bearer, metadata, last use, immediate revocation, logout, rate limit"
    );
    Ok(())
}
