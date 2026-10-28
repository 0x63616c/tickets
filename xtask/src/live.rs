//! Two real WebSocket clients and two isolated Chromium windows, local only.
use crate::design::Browser;
use serde_json::{Value, json};
use std::{fs, path::Path, time::Duration};
use tungstenite::{client::IntoClientRequest, http::HeaderValue, stream::MaybeTlsStream};

pub fn check(root: &Path, password: &str, token: &str) -> super::Result {
    let client = reqwest::blocking::Client::new();
    let api = |method, path: &str, body: Value| -> Result<Value, Box<dyn std::error::Error>> {
        let response = client
            .request(method, format!("http://127.0.0.1:8795/api/v1/{path}"))
            .bearer_auth(token)
            .json(&body)
            .send()?
            .error_for_status()?;
        Ok(response.json()?)
    };
    let project = api(
        reqwest::Method::POST,
        "projects",
        json!({"name":"Tickets","key_prefix":"TKT","color":"#5e6ad2"}),
    )?["id"]
        .as_i64()
        .ok_or("project id")?;
    let ticket = api(
        reqwest::Method::POST,
        "tickets",
        json!({"project_id":project,"title":"Move appears in both windows","status":"todo"}),
    )?["id"]
        .as_i64()
        .ok_or("ticket id")?;
    let url = format!("ws://127.0.0.1:8795/api/v1/live/{project}");
    match tungstenite::connect(&url) {
        Err(tungstenite::Error::Http(response)) if response.status() == 401 => {}
        _ => return Err("unauthenticated upgrade did not return 401".into()),
    }
    let connect = || {
        let mut request = url.clone().into_client_request()?;
        request.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {token}"))?,
        );
        let (socket, _) = tungstenite::connect(request)?;
        Ok::<_, Box<dyn std::error::Error>>(socket)
    };
    let mut first = connect()?;
    let mut second = connect()?;
    for socket in [&mut first, &mut second] {
        if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
            stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        }
    }
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"priority":2}),
    )?;
    for socket in [&mut first, &mut second] {
        let event: tickets_core::live::ChangeEvent =
            serde_json::from_str(socket.read()?.to_text()?)?;
        if event.project_id != project || event.id != ticket || event.action != "update" {
            return Err("wrong broadcast".into());
        }
    }
    let output = root.join("docs/screenshots/16");
    fs::create_dir_all(&output)?;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let phase = if before { "before" } else { "after" };
    let login = |browser: &mut Browser| -> super::Result {
        browser.viewport(1440, 900)?;
        browser.go("http://127.0.0.1:8795/login?next=%2Fsettings%2Ftokens")?;
        browser.text("#username", "fixture")?;
        browser.text("#password", password)?;
        browser.click("button[type=submit]")?;
        browser.wait("location.pathname==='/settings/tokens'")?;
        browser.go("http://127.0.0.1:8795/")?;
        browser.wait("document.querySelectorAll('.ticket-card').length===1")?;
        Ok(())
    };
    let mut a = Browser::start_named(root, "live-browser-a")?;
    login(&mut a)?;
    a.screenshot(&output.join(format!("board-{phase}-desktop.png")))?;
    a.viewport(390, 900)?;
    a.screenshot(&output.join(format!("board-{phase}-mobile.png")))?;
    a.viewport(1440, 900)?;
    if before {
        a.go("http://127.0.0.1:8795/design/board")?;
        a.wait("document.querySelectorAll('.ticket-card').length>1")?;
        a.screenshot(&output.join("board-target-desktop.png"))?;
        return Ok(());
    }
    let mut b = Browser::start_named(root, "live-browser-b")?;
    login(&mut b)?;
    for browser in [&mut a, &mut b] {
        browser.wait("document.querySelector('.connection.connected')!==null")?;
    }
    a.screenshot(&output.join("demo-01-window-a.png"))?;
    b.screenshot(&output.join("demo-01-window-b.png"))?;
    a.eval("var s=document.querySelector('.ticket-card select');s.value='In progress';s.dispatchEvent(new Event('change',{bubbles:true}));true")?;
    for browser in [&mut a, &mut b] {
        browser.wait("document.querySelector('.ticket-card select')?.value==='In progress'")?;
    }
    a.screenshot(&output.join("demo-02-window-a.png"))?;
    b.screenshot(&output.join("demo-02-window-b.png"))?;
    b.go("http://127.0.0.1:8795/list")?;
    b.wait("document.querySelector('.connection.connected')!==null")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Live list reconciliation verified"}),
    )?;
    b.wait("document.body.innerText.includes('Live list reconciliation verified')")?;
    b.go(&format!("http://127.0.0.1:8795/ticket?id={ticket}"))?;
    b.wait("document.querySelector('.connection.connected')!==null && document.body.innerText.includes('Live list reconciliation verified')")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Live detail reconciliation verified"}),
    )?;
    b.wait("document.body.innerText.includes('Live detail reconciliation verified')")?;
    // Force offline then restore: reconnection must refetch missed writes.
    b.call("Network.enable", json!({}))?;
    b.call(
        "Network.emulateNetworkConditions",
        json!({"offline":true,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
    )?;
    b.wait("document.querySelector('.connection.offline')!==null")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Reconnected and reconciled"}),
    )?;
    b.call(
        "Network.emulateNetworkConditions",
        json!({"offline":false,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
    )?;
    b.wait("document.body.innerText.includes('Reconnected and reconciled')")?;
    println!(
        "live-check: upgrade auth, two-client broadcast, two-window move, list/detail reconciliation and reconnect verified"
    );
    Ok(())
}
