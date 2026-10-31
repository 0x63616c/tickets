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
    let output = root.join("docs/screenshots/36");
    fs::create_dir_all(&output)?;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let phase = if before { "before" } else { "after" };
    let login = |browser: &mut Browser| -> super::Result {
        browser.viewport(1440, 900)?;
        browser.go("http://127.0.0.1:8795/login?next=%2Fsettings%2Ftokens")?;
        browser.text("#username", "fixture")?;
        browser.text("#password", password)?;
        browser.click("button[type=submit]")?;
        browser.wait_for(
            "['/settings/tokens','/settings/api-tokens'].includes(location.pathname)",
            std::time::Duration::from_secs(40),
        )?;
        browser.go("http://127.0.0.1:8795/board")?;
        browser.wait_for(
            "document.querySelectorAll('.ticket-card').length===1",
            std::time::Duration::from_secs(40),
        )?;
        Ok(())
    };
    let mut a = Browser::start_named(root, "live-36-browser-a")?;
    login(&mut a)?;
    a.screenshot(&output.join(format!("board-{phase}-desktop.png")))?;
    a.go("http://127.0.0.1:8795/list")?;
    a.wait_for("document.querySelector('.list-row')!==null || document.querySelector('.view-footer')!==null", std::time::Duration::from_secs(40))?;
    a.screenshot(&output.join(format!("list-{phase}-desktop.png")))?;
    a.go("http://127.0.0.1:8795/board")?;
    a.wait_for(
        "document.querySelectorAll('.ticket-card').length===1",
        std::time::Duration::from_secs(40),
    )?;
    a.viewport(390, 900)?;
    a.screenshot(&output.join(format!("board-{phase}-mobile.png")))?;
    a.viewport(1440, 900)?;
    if before {
        a.go("http://127.0.0.1:8795/design/board")?;
        a.wait_for(
            "document.querySelectorAll('.ticket-card').length>1",
            std::time::Duration::from_secs(40),
        )?;
        a.screenshot(&output.join("board-target-desktop.png"))?;
        return Ok(());
    }
    let mut b = Browser::start_named(root, "live-36-browser-b")?;
    b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":
        "window.__liveSockets=[];const NativeSocket=WebSocket;window.WebSocket=class extends NativeSocket{constructor(...args){super(...args);window.__liveSockets.push(this);this.addEventListener('message',e=>{if(window.__dropLive)e.stopImmediatePropagation()})}};"
    }))?;
    login(&mut b)?;
    for browser in [&mut a, &mut b] {
        browser.wait_for(
            "document.querySelector('.connection.connected')!==null",
            std::time::Duration::from_secs(40),
        )?;
    }
    a.screenshot(&output.join("demo-01-window-a.png"))?;
    b.screenshot(&output.join("demo-01-window-b.png"))?;
    a.choose(".ticket-card [role=combobox]", "In progress")?;
    for browser in [&mut a, &mut b] {
        browser.wait_for(
            "document.querySelector('.ticket-card [role=combobox]')?.value==='In progress'",
            std::time::Duration::from_secs(40),
        )?;
    }
    a.screenshot(&output.join("demo-02-window-a.png"))?;
    b.screenshot(&output.join("demo-02-window-b.png"))?;
    b.eval("window.__reads=0;window.__readPaths=[];const nativeFetch=window.fetch;window.fetch=(...args)=>{window.__reads++;window.__readPaths.push(new URL(args[0] instanceof Request?args[0].url:String(args[0]),location.href).pathname);return nativeFetch(...args)};window.__dropLive=true;true")?;
    let hidden_tab = b.call("Target.createTarget", json!({"url":"about:blank"}))?;
    b.wait_for("document.hidden", std::time::Duration::from_secs(40))?;
    a.choose(".ticket-card [role=combobox]", "Done")?;
    a.wait_for(
        "document.querySelector('.ticket-card [role=combobox]')?.value==='Done'",
        std::time::Duration::from_secs(40),
    )?;
    println!("live-check: checking a full hidden interval makes zero REST reads");
    let hidden_deadline = std::time::Instant::now() + Duration::from_secs(61);
    while std::time::Instant::now() < hidden_deadline {
        b.eval("if(window.__reads!==0)throw Error('hidden tab fetched: '+window.__reads);true")?;
        std::thread::sleep(Duration::from_millis(200));
    }
    b.eval("if(document.querySelector('.ticket-card [role=combobox]')?.value==='Done')throw Error('hidden client unexpectedly reconciled');window.__socketCount=window.__liveSockets.length;true")?;
    b.call("Page.bringToFront", json!({}))?;
    b.eval(
        "window.dispatchEvent(new Event('focus'));window.dispatchEvent(new Event('online'));true",
    )?;
    b.wait_for(
        "document.querySelector('.ticket-card [role=combobox]')?.value==='Done'",
        std::time::Duration::from_secs(40),
    )?;
    b.eval("if(window.__liveSockets.length!==window.__socketCount)throw Error('catch-up reconnected');true")?;
    b.eval("new Promise(resolve=>setTimeout(resolve,800))")?;
    // Recovery downloads each collection once; sidebar shares projects.
    b.eval(
        "if(JSON.stringify(window.__readPaths.sort())!==JSON.stringify(['/api/v1/labels','/api/v1/projects','/api/v1/tickets']))throw Error('event burst did not coalesce: '+JSON.stringify(window.__readPaths));true",
    )?;
    b.screenshot(&output.join("visibility-catchup-desktop.png"))?;
    b.eval("window.__dropLive=false;true")?;
    b.eval("var card=document.querySelector('.ticket-card');card.focus();card.dispatchEvent(new KeyboardEvent('keydown',{key:' ',bubbles:true}));window.dispatchEvent(new Event('focus'));true")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Deferred drag catch-up verified"}),
    )?;
    b.eval("new Promise(resolve=>setTimeout(resolve,800))")?;
    b.eval("if(document.body.innerText.includes('Deferred drag catch-up verified'))throw Error('drag was reconciled');document.querySelector('.ticket-card').dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));true")?;
    b.wait_for(
        "document.body.innerText.includes('Deferred drag catch-up verified')",
        std::time::Duration::from_secs(40),
    )?;
    // Healthy focus must not reload. A missed notification is recovered by
    // reopening the socket, rather than a periodic full-collection poll.
    b.eval("window.__priorReads=window.__reads;window.dispatchEvent(new Event('focus'));true")?;
    b.eval("new Promise(resolve=>setTimeout(resolve,800))")?;
    b.eval("if(window.__reads!==window.__priorReads)throw Error('healthy focus refetched');true")?;
    b.eval("window.__dropLive=true;true")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Reconnect catch-up verified"}),
    )?;
    b.eval("window.__dropLive=false;for(const socket of window.__liveSockets)socket.close();true")?;
    b.wait_for(
        "document.body.innerText.includes('Reconnect catch-up verified')",
        std::time::Duration::from_secs(40),
    )?;
    b.call(
        "Target.closeTarget",
        json!({"targetId":hidden_tab["targetId"]}),
    )?;

    for (event, target) in [
        ("visibilitychange", "document"),
        ("focus", "window"),
        ("online", "window"),
    ] {
        let title = format!("Catch-up on {event}");
        api(
            reqwest::Method::PATCH,
            &format!("tickets/{ticket}"),
            json!({"title":title}),
        )?;
        b.eval(&format!(
            "{target}.dispatchEvent(new Event('{event}'));true"
        ))?;
        b.wait_for(
            &format!(
                "document.body.innerText.includes({})",
                serde_json::to_string(&title)?
            ),
            std::time::Duration::from_secs(40),
        )?;
    }
    b.go("http://127.0.0.1:8795/list")?;
    b.wait_for(
        "document.querySelector('.connection.connected')!==null",
        std::time::Duration::from_secs(40),
    )?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Live list reconciliation verified"}),
    )?;
    b.wait_for(
        "document.body.innerText.includes('Live list reconciliation verified')",
        std::time::Duration::from_secs(40),
    )?;
    let key = api(
        reqwest::Method::GET,
        &format!("tickets/{ticket}"),
        Value::Null,
    )?["key"]
        .as_str()
        .ok_or("missing ticket key")?
        .to_owned();
    b.go(&format!("http://127.0.0.1:8795/t/{key}"))?;
    b.wait_for("document.querySelector('.connection.connected')!==null && document.body.innerText.includes('Live list reconciliation verified')", std::time::Duration::from_secs(40))?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Live detail reconciliation verified"}),
    )?;
    b.wait_for(
        "document.body.innerText.includes('Live detail reconciliation verified')",
        std::time::Duration::from_secs(40),
    )?;
    b.click(".topbar .button-secondary")?;
    b.text("#detail-title", "Unsaved local draft")?;
    b.eval("document.activeElement.blur();true")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Deferred edit catch-up verified"}),
    )?;
    b.eval("new Promise(resolve=>setTimeout(resolve,800))")?;
    b.eval("if(document.querySelector('#detail-title')?.value!=='Unsaved local draft')throw Error('draft clobbered');true")?;
    b.click("[aria-label='Close dialog']")?;
    b.wait_for(
        "document.body.innerText.includes('Deferred edit catch-up verified')",
        std::time::Duration::from_secs(40),
    )?;
    // Force offline then restore: reconnection must refetch missed writes.
    b.call("Network.enable", json!({}))?;
    b.call(
        "Network.emulateNetworkConditions",
        json!({"offline":true,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
    )?;
    b.wait_for(
        "document.querySelector('.connection.offline')!==null",
        std::time::Duration::from_secs(40),
    )?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"title":"Reconnected and reconciled"}),
    )?;
    b.call(
        "Network.emulateNetworkConditions",
        json!({"offline":false,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
    )?;
    b.wait_for(
        "document.body.innerText.includes('Reconnected and reconciled')",
        std::time::Duration::from_secs(40),
    )?;
    // Home consumes invalidations through its aggregate endpoint, without a board download.
    b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":
        "window.homeReads=[];const homeFetch=window.fetch;window.fetch=(...args)=>{window.homeReads.push(new URL(args[0] instanceof Request?args[0].url:String(args[0]),location.href).pathname);return homeFetch(...args)};"
    }))?;
    b.go("http://127.0.0.1:8795/")?;
    b.wait_for("document.querySelectorAll('.home-metric strong').length===3 && window.__liveSockets.some(s=>s.readyState===1)", std::time::Duration::from_secs(40))?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{ticket}"),
        json!({"status":"in_review"}),
    )?;
    b.wait_for("[...document.querySelectorAll('.home-metric strong')].slice(0,2).map(e=>e.textContent).join(',')==='1,1'", std::time::Duration::from_secs(40))?;
    b.eval("if(window.homeReads.some(p=>p.startsWith('/api/v1/tickets')))throw Error('Home downloaded tickets');if(window.homeReads.filter(p=>p==='/api/v1/analytics/home').length<2)throw Error('Home did not refresh aggregate');true")?;
    println!(
        "live-check: upgrade auth, two-client broadcast, two-window move, list/detail reconciliation reconnect, hidden/visible recovery, event coalescing, healthy focus without polling, edit/drag deferral and aggregate-only Home refresh verified"
    );
    Ok(())
}
