//! Real local Worker login regression and dark route captures.
use crate::design::Browser;
use serde_json::json;
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn go(browser: &mut Browser, route: &str, selector: &str) -> Result<()> {
    browser.call("Page.navigate", json!({"url":route}))?;
    browser.wait(&format!(
        "location.href === {} && document.readyState === 'complete' && !!document.querySelector({})",
        serde_json::to_string(route)?, serde_json::to_string(selector)?
    ))?;
    browser.eval("document.fonts.ready.then(() => true)")?;
    Ok(())
}
pub fn before(root: &Path) -> Result<()> {
    let out = root.join("docs/screenshots/34");
    fs::create_dir_all(&out)?;
    let mut b = Browser::start_named(root, "login-before-chromium")?;
    for (name, w, h) in [("desktop", 1440, 900), ("mobile", 375, 812)] {
        b.viewport(w, h)?;
        for (page, route) in [("root", "/"), ("login", "/login"), ("target", "/design")] {
            go(
                &mut b,
                &format!("http://127.0.0.1:8787{route}"),
                if page == "login" { ".login" } else { ".topbar" },
            )?;
            b.screenshot(&out.join(format!("before-{page}-{name}.png")))?;
        }
    }
    Ok(())
}
pub fn check(root: &Path, password: &str, state: &Path) -> Result<()> {
    let out = root.join("docs/screenshots/34");
    fs::create_dir_all(&out)?;
    let mut evidence = String::new();
    for (route, cookie, expected, location) in [
        ("/", false, 302, Some("/login?next=%2F")),
        (
            "/ticket?id=1",
            false,
            302,
            Some("/login?next=%2Fticket%3Fid%3D1"),
        ),
        (
            "/unknown/path?x=2",
            false,
            302,
            Some("/login?next=%2Funknown%2Fpath%3Fx%3D2"),
        ),
        ("/list", false, 302, Some("/login?next=%2Flist")),
        ("/projects", false, 302, Some("/login?next=%2Fprojects")),
        ("/api/v1/projects", false, 401, None),
        ("/missing-deadbeef.js", false, 404, None),
        ("/fonts/missing.woff2", false, 404, None),
        ("/index.html", false, 302, Some("/login?next=%2Findex.html")),
        ("/", true, 200, None),
        ("/login", true, 302, Some("/")),
        ("/login?next=%2Flist", true, 302, Some("/list")),
        ("/login?next=%2F%2Fevil.com", true, 302, Some("/")),
    ] {
        let mut cmd = Command::new("curl");
        cmd.args(["-sI"]);
        if cookie {
            cmd.arg("-b").arg(state.join("cookies.local"));
        }
        let response = cmd.arg(format!("http://127.0.0.1:8787{route}")).output()?;
        let headers = String::from_utf8(response.stdout)?;
        if !headers.starts_with(&format!("HTTP/1.1 {expected}")) {
            return Err(headers.into());
        }
        if let Some(location) = location
            && !headers
                .to_lowercase()
                .contains(&format!("location: {location}\r\n").to_lowercase())
        {
            return Err(headers.into());
        }
        evidence.push_str(&format!(
            "$ curl -sI {}http://127.0.0.1:8787{route}\n{}\n",
            if cookie {
                "-b <local-session-cookie> "
            } else {
                ""
            },
            headers
        ));
    }
    for asset in fs::read_dir(root.join("crates/web/dist"))? {
        let asset = asset?;
        let name = asset.file_name().to_string_lossy().into_owned();
        if !["js", "wasm", "css"].contains(
            &asset
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or(""),
        ) {
            continue;
        }
        let response = Command::new("curl")
            .args(["-sI", &format!("http://127.0.0.1:8787/{name}")])
            .output()?;
        let headers = String::from_utf8(response.stdout)?;
        if !headers.starts_with("HTTP/1.1 200") {
            return Err(format!("asset {name}: {headers}").into());
        }
        evidence.push_str(&format!(
            "$ curl -sI http://127.0.0.1:8787/{name}\n{headers}\n"
        ));
    }
    fs::write(
        out.join("curl.txt"),
        format!("{}\n", evidence.replace("\r\n", "\n").trim_end()),
    )?;
    let mut b = Browser::start_named(root, "login-check")?;
    for (name, w, h) in [("desktop", 1440, 900), ("mobile", 375, 812)] {
        b.viewport(w, h)?;
        go(
            &mut b,
            "http://127.0.0.1:8787/login?next=%2Flist",
            ".login-page",
        )?;
        b.wait("document.activeElement.id === 'username'")?;
        b.wait("!document.querySelector('.sidebar, .app-shell') && document.documentElement.scrollWidth <= innerWidth")?;
        b.screenshot(&out.join(format!("after-login-{name}.png")))?;
        b.text("#username", "fixture")?;
        b.text("#password", "incorrect-local-fixture")?;
        // Hold the rejected request to capture loading, then let the real API reject it.
        b.call(
            "Fetch.enable",
            json!({"patterns":[{"urlPattern":"*/api/v1/login","requestStage":"Request"}]}),
        )?;
        b.click("button[type=submit]")?;
        b.wait("document.querySelector('button[type=submit]').disabled && document.body.textContent.includes('Signing in…')")?;
        b.screenshot(&out.join(format!("after-loading-{name}.png")))?;
        b.call("Fetch.disable", json!({}))?;
        b.wait("document.querySelector('[role=alert]').textContent.includes('Incorrect') && !document.querySelector('button[type=submit]').disabled")?;
        b.screenshot(&out.join(format!("after-error-{name}.png")))?;
    }
    // Exercise rate limiting and a failed connection without spending real login attempts.
    b.eval("window.originalFetch=window.fetch;window.fetch=()=>Promise.resolve(new Response('{}',{status:429}));true")?;
    b.text("#password", "local-attempt")?;
    b.click("button[type=submit]")?;
    b.wait("document.querySelector('[role=alert]').textContent.includes('Too many') && !document.querySelector('button[type=submit]').disabled")?;
    b.eval("window.fetch=()=>Promise.reject(new Error('offline'));true")?;
    b.text("#password", "local-attempt")?;
    b.click("button[type=submit]")?;
    b.wait("document.querySelector('[role=alert]').textContent.includes('Connection failed') && !document.querySelector('button[type=submit]').disabled")?;
    b.eval("window.fetch=window.originalFetch;true")?;
    b.text("#password", password)?;
    b.call("Input.dispatchKeyEvent", json!({"type":"keyDown","key":"Enter","code":"Enter","windowsVirtualKeyCode":13,"text":"\r"}))?;
    b.call(
        "Input.dispatchKeyEvent",
        json!({"type":"keyUp","key":"Enter","windowsVirtualKeyCode":13}),
    )?;
    b.wait("location.pathname === '/list' && !!document.querySelector('.topbar')")?;
    b.eval("[...document.querySelectorAll('.sidebar-bottom button')].find(b=>b.textContent.includes('Sign out')).click();true")?;
    b.wait("location.pathname === '/login' && !!document.querySelector('.login-page')")?;
    go(
        &mut b,
        "http://127.0.0.1:8787/login?next=%2F%2Fevil.com",
        ".login-page",
    )?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname === '/' && !!document.querySelector('.topbar')")?;
    // Client-side route navigation triggers an API load after session expiry.
    b.call("Network.clearBrowserCookies", json!({}))?;
    b.eval("document.querySelector('.sidebar a[href=\"/projects\"]').click();true")?;
    b.wait("location.pathname === '/login' && new URLSearchParams(location.search).get('next') === '/projects' && !!document.querySelector('.login-page')")?;
    println!(
        "Login routes, assets, form states, mobile layout, Enter, logout, next validation and mid-session 401 passed."
    );
    Ok(())
}
