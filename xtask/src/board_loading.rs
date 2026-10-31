//! Sample the real local Worker routes while holding ticket responses, including
//! Home's project-only preload and live reconciliation. No app state is mocked.
use crate::design::Browser;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;
use std::{
    env, fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

pub fn check(root: &Path, b: &mut Browser) -> super::Result {
    let phase = if env::var_os("TICKETS_BEFORE_ASSETS").is_some() {
        "before"
    } else {
        "after"
    };
    let output = root.join("docs/screenshots/TIK-75");
    fs::create_dir_all(&output)?;
    b.call("Network.setCacheDisabled", json!({"cacheDisabled":true}))?;
    b.call(
        "Page.addScriptToEvaluateOnNewDocument",
        json!({"source":r#"
        window.held = true;
        window.waiters = [];
        window.heldRequests = 0;
        const originalFetch = window.fetch;
        window.fetch = async (...args) => {
            const response = await originalFetch(...args);
            const url = args[0] instanceof Request ? args[0].url : String(args[0]);
            if (url.includes('/api/v1/tickets') && window.held) {
                window.heldRequests++;
                await new Promise(resolve => window.waiters.push(resolve));
            }
            return response;
        };
        window.release = () => { window.held = false; window.waiters.splice(0).forEach(r => r()); };
        window.visible = selector => [...document.querySelectorAll(selector)].some(e => {
            const r=e.getBoundingClientRect();
            return r.width>0 && r.height>0 && getComputedStyle(e).visibility!=='hidden';
        });
        window.samples=[];
        window.sample = () => ({time:performance.now(),
            skeleton:visible('.loading-skeleton'),
            content:visible('.api-board, .ticket-list, .board-filters, .empty-project'),
            boardY:document.querySelector('.api-board')?.getBoundingClientRect().y ?? null,
            cards:document.querySelectorAll('.api-ticket,.api-list-row').length,
            busy:document.querySelector('.api-board,.ticket-list')?.getAttribute('aria-busy') ?? null,
            sameRoot:!window.contentRoot || document.querySelector('.api-board,.ticket-list')===window.contentRoot});
        setInterval(()=>window.samples.push(sample()),50);
    "#}),
    )?;
    let mut report = String::new();
    let mut failures = Vec::new();
    for (name, route, home, slow, refresh) in [
        ("board-cold", "/board", false, false, false),
        ("board-slow", "/board", false, true, false),
        ("home-board", "/board", true, false, false),
        ("board-empty", "/board?project=3", true, false, false),
        ("home-board-mobile", "/board", true, false, false),
        ("board-catchup", "/board", false, false, true),
        ("board-refresh", "/board", false, false, true),
        ("list-cold", "/list", false, false, false),
        ("home-list", "/list", true, false, false),
        ("home-list-mobile", "/list", true, false, false),
        ("list-catchup", "/list", false, false, true),
    ] {
        if name.ends_with("mobile") {
            b.viewport(390, 844)?;
        } else {
            b.viewport(1440, 900)?;
        }
        b.call("Network.emulateNetworkConditions", json!({"offline":false,"latency":if slow {150} else {0},"downloadThroughput":-1,"uploadThroughput":-1}))?;
        b.go(&format!(
            "http://127.0.0.1:8795{}",
            if home { "/" } else { route }
        ))?;
        if home {
            b.wait("!!document.querySelector('.home-metric strong') && !!document.querySelector('.sidebar a[href*=board]') && document.querySelector('.sidebar').textContent.includes('Quantum')")?;
            if name.ends_with("mobile") {
                b.click("button[aria-label='Toggle sidebar']")?;
            }
            b.click(&format!("a[href='{route}']"))?;
        }
        b.wait("window.heldRequests>0")?;
        if refresh {
            b.eval("release();true")?;
            b.wait("!document.querySelector('.loading-skeleton') && !!document.querySelector('.api-ticket,.api-list-row,.empty-project')")?;
            b.wait("!document.querySelector('[data-ar-t]')")?;
            b.eval("window.held=true;window.heldRequests=0;true")?;
            if name == "board-refresh" {
                b.click("a[href='/list']")?;
                // Navigation reuses the shared collection; exercise explicit
                // recovery while it is mounted instead of requiring a reload.
                b.eval("window.dispatchEvent(new Event('online'));true")?;
                b.wait("window.heldRequests>0")?;
                b.click("a[href='/board']")?;
                // SPA navigation commits asynchronously. Start continuity
                // sampling on Board, rather than capturing List as its root.
                b.wait("!!document.querySelector('.api-board') && !document.querySelector('.ticket-list')")?;
            } else {
                b.eval("window.dispatchEvent(new Event('online'));true")?;
            }
            b.wait("window.heldRequests>0")?;
        }
        b.eval("window.contentRoot=document.querySelector('.api-board,.ticket-list');window.samples=[];true")?;
        let frames = root.join(format!("target/TIK-75/{phase}-{name}"));
        fs::create_dir_all(&frames)?;
        let start = Instant::now();
        let mut index = 0;
        while start.elapsed() < Duration::from_secs(4) {
            if start.elapsed() >= Duration::from_millis(1800) {
                b.eval("release();true")?;
            }
            let shot = b.call(
                "Page.captureScreenshot",
                json!({"format":"png","captureBeyondViewport":false}),
            )?;
            fs::write(
                frames.join(format!("{index:03}.png")),
                STANDARD.decode(shot["data"].as_str().ok_or("missing frame")?)?,
            )?;
            if index == 0 {
                fs::copy(
                    frames.join("000.png"),
                    output.join(format!("{phase}-{name}-held.png")),
                )?;
            }
            index += 1;
            let deadline = Duration::from_millis(index * 50);
            if let Some(wait) = deadline.checked_sub(start.elapsed()) {
                thread::sleep(wait);
            }
        }
        b.eval("release();true")?;
        b.wait("!document.querySelector('.loading-skeleton') && !!document.querySelector('.api-ticket,.api-list-row,.empty-project')")?;
        b.screenshot(&output.join(format!("{phase}-{name}-settled.png")))?;
        let samples = b.eval("window.samples")?;
        let samples = samples.as_array().ok_or("missing samples")?;
        let overlaps = samples
            .iter()
            .filter(|s| s["skeleton"] == true && s["content"] == true)
            .count();
        let skeleton = samples.iter().filter(|s| s["skeleton"] == true).count();
        let line = format!(
            "{phase} {name}: {overlaps}/{} overlap samples; {skeleton} skeleton samples; {index} PNG frames over 4s\n",
            samples.len()
        );
        print!("{line}");
        report.push_str(&line);
        fs::write(
            output.join(format!("{phase}-{name}.json")),
            serde_json::to_string_pretty(&samples)?,
        )?;
        let stable = !refresh
            || samples.iter().all(|s| {
                s["skeleton"] == false
                    && s["content"] == true
                    && s["boardY"] == samples[0]["boardY"]
                    && s["cards"] == samples[0]["cards"]
                    && s["sameRoot"] == true
            });
        if overlaps > 0
            || !stable
            || (!refresh && skeleton == 0)
            || (phase == "after" && refresh && !samples.iter().any(|s| s["busy"] == "true"))
        {
            failures.push(name);
        }
    }
    use std::io::Write;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(output.join("transitions.txt"))?
        .write_all(report.as_bytes())?;
    if phase == "after" && !failures.is_empty() {
        return Err(format!("Loading exclusivity/refresh continuity failed: {failures:?}").into());
    }
    Ok(())
}
