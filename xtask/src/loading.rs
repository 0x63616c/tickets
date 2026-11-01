//! Held fixture responses on the real routes; no delay or mocked animation in the app.
use crate::{component::FixtureServer, design::Browser};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

pub fn check(root: &std::path::Path) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS");
    let phase = if before.is_some() { "before" } else { "after" };
    let assets = before
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let output = env::var_os("TICKETS_LOADING_SHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("docs/screenshots/TIK-47"));
    fs::create_dir_all(&output)?;
    let seed: Value = serde_json::from_str(include_str!("../../docs/fixtures/home-api.json"))?;
    let home: Value = serde_json::from_str(include_str!("../../docs/fixtures/home.json"))?;
    let mut fixtures = HashMap::from([
        (
            "/api/v1/me".into(),
            json!({"authenticated":true,"username":"sample-user","display_name":"Sample User"}),
        ),
        (
            "/api/v1/projects".into(),
            json!({"data":seed["projects"],"next_cursor":null}),
        ),
        (
            "/api/v1/tickets".into(),
            json!({"data":seed["tickets"],"next_cursor":null}),
        ),
        (
            "/api/v1/search".into(),
            json!({"data":seed["tickets"],"next_cursor":null}),
        ),
        ("/api/v1/analytics/home".into(), home),
        (
            "/api/v1/mission".into(),
            json!({"generated_at":"2026-10-31T00:00:00Z","reports":[],"waiting_on_calum":[],"in_review":[],"recently_done":[]}),
        ),
        (
            "/api/v1/pickup-rules".into(),
            json!({"source":"workspace","rules":tickets_core::pickup::Rules::default()}),
        ),
        (
            "/api/v1/github/app".into(),
            json!({"registered":false,"app_slug":null,"app_name":null,"html_url":null,"installations":[]}),
        ),
    ]);
    for ticket in seed["tickets"].as_array().unwrap() {
        fixtures.insert(
            format!("/api/v1/tickets/{}", ticket["key"].as_str().unwrap()),
            ticket.clone(),
        );
        fixtures.insert(format!("/api/v1/tickets/{}", ticket["id"]), ticket.clone());
    }
    let server = FixtureServer::start_with_api(&assets, fixtures)?;
    let mut b = Browser::start_named(root, "loading")?;
    cold_overflow(root, &server.url, phase)?;
    // Hold responses (including sequential reads) until explicitly released. Watch
    // library-owned DOM markers so a static lookalike cannot satisfy the check.
    b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":r#"
        window.loadingHeld = true;
        window.loadingWaiters = [];
        const originalLoadingFetch = window.fetch;
        window.fetch = async (...args) => {
            const response = await originalLoadingFetch(...args);
            const url = args[0] instanceof Request ? args[0].url : String(args[0]);
            if (url.includes('/api/v1/') && window.loadingHeld)
                await new Promise(resolve => window.loadingWaiters.push(resolve));
            return response;
        };
        window.releaseLoading = () => { window.loadingHeld = false; window.loadingWaiters.forEach(resolve => resolve()); };
        window.decodeSeen = false;
        new MutationObserver(records => {
            if (document.querySelector('[data-ar-t]')) window.decodeSeen = true;
        }).observe(document, {subtree:true, childList:true, attributes:true, attributeFilter:['data-ar-t']});
    "#}))?;
    for (width, height) in [(1440, 900), (390, 844)] {
        b.viewport(width, height)?;
        for (name, route, ready) in [
            (
                "board",
                "/board",
                "document.querySelector('.api-ticket .card-title')",
            ),
            (
                "detail",
                "/t/TKT-1",
                "document.querySelector('.ticket-detail h1')",
            ),
            ("list", "/list", "document.querySelector('.row-title')"),
            (
                "projects",
                "/projects",
                "document.querySelector('.api-project-row')",
            ),
            (
                "pickup-settings",
                "/settings/pickup-rules",
                "document.querySelector('.pickup-settings input')",
            ),
            ("home", "/", "document.querySelector('.home-metric strong')"),
            (
                "analytics",
                "/analytics",
                "document.querySelector('.analytics-card svg')",
            ),
            (
                "mission",
                "/mission",
                "document.querySelector('.mission-section')",
            ),
            (
                "account",
                "/settings/account",
                "document.body.textContent.includes('sample-user') && !!document.querySelector('.settings-github a')",
            ),
            (
                "tokens",
                "/settings/api-tokens",
                "document.querySelector('.token-empty')",
            ),
            (
                "project-settings",
                "/settings/projects",
                "document.querySelector('.settings-project')",
            ),
        ] {
            b.go(&format!("{}{route}", server.url))?;
            b.wait("window.loadingWaiters.length > 0")?;
            if phase == "after" {
                b.wait("!!document.querySelector('.ars canvas')")?;
                // Canvas output must change while the response is held.
                b.eval("window.sweepFrame=document.querySelector('.ars canvas').toDataURL();true")?;
                b.wait("document.querySelector('.ars canvas').toDataURL()!==window.sweepFrame")?;
            }
            b.eval("document.fonts.ready.then(()=>true)")?;
            b.eval("new Promise(r=>setTimeout(()=>r(true),700))")?;
            if phase == "after" {
                assert_surfaces(&mut b)?;
            }
            b.screenshot(&output.join(format!("{phase}-{name}-{width}.png")))?;
            b.eval("window.releaseLoading();true")?;
            b.wait(&format!("!!({ready})"))?;
            if phase == "after" {
                if name != "pickup-settings" {
                    b.wait("window.decodeSeen")?;
                }
                b.wait(
                    "!document.querySelector('.ars canvas') && !document.querySelector('[data-ar-t]')",
                )?;
            }
        }
    }
    if phase == "before" {
        return Ok(());
    }
    b.viewport(1440, 900)?;
    // Search uses the same package for its result skeleton and newly arrived
    // ticket text, while the user's input keeps focus throughout the transition.
    b.go(&format!("{}/board", server.url))?;
    b.wait("!!document.querySelector('.ars canvas')")?;
    b.eval("window.releaseLoading();true")?;
    b.wait("!!document.querySelector('.api-ticket') && !document.querySelector('[data-ar-t]')")?;
    b.eval("window.loadingHeld=true;window.loadingWaiters=[];window.decodeSeen=false;true")?;
    b.click(".search-trigger")?;
    b.text(".command-search input", "repository")?;
    b.wait(
        "!!document.querySelector('.command-empty .ars canvas') && window.loadingWaiters.length>0",
    )?;
    assert_surfaces(&mut b)?;
    b.screenshot(&output.join("after-search.png"))?;
    b.eval("window.releaseLoading();true")?;
    b.wait("window.decodeSeen && !!document.querySelector('.command-option')")?;
    b.wait("!document.querySelector('[data-ar-t]') && !document.querySelector('.ars canvas') && document.activeElement.matches('.command-search input')")?;
    b.key("Escape", 27, 0)?;
    // A submit control reserves its original footprint, sweeps while pending,
    // then decodes its real label. Repeating proves cleanup/replay work.
    b.go(&format!(
        "{}/design/c/busy-button?variant=cycle",
        server.url
    ))?;
    b.eval("window.busyWidth=document.querySelector('.busy-button').getBoundingClientRect().width;true")?;
    for _ in 0..2 {
        b.eval("window.decodeSeen=false;true")?;
        b.click(".busy-button")?;
        b.wait("document.querySelector('.busy-button').disabled && !!document.querySelector('.ars canvas')")?;
        b.wait("Math.abs(document.querySelector('.busy-button').getBoundingClientRect().width-window.busyWidth)<1")?;
        assert_surfaces(&mut b)?;
        b.wait("window.decodeSeen && !document.querySelector('.busy-button').disabled")?;
        b.wait("!document.querySelector('[data-ar-t]') && !document.querySelector('.ars canvas')")?;
    }
    // Both OS preference and in-app override must stop active sweeps. Then mount
    // new content under reduced motion and verify no decode ever starts.
    for in_app in [false, true] {
        b.go(&format!("{}/board", server.url))?;
        b.wait("!!document.querySelector('.ars canvas')")?;
        if in_app {
            b.eval("document.documentElement.dataset.motion='reduce';true")?;
        } else {
            b.call(
                "Emulation.setEmulatedMedia",
                json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
            )?;
        }
        b.wait("!!document.querySelector('.ars') && !document.querySelector('.ars canvas')")?;
        assert_surfaces(&mut b)?;
        let still = b.eval("document.querySelector('.loading-skeleton').innerHTML")?;
        b.eval("new Promise(r=>setTimeout(()=>r(true),250))")?;
        if b.eval("document.querySelector('.loading-skeleton').innerHTML")? != still {
            return Err("Reduced-motion skeleton changed while held".into());
        }
        b.screenshot(&output.join(format!(
            "after-board-reduced-{}.png",
            if in_app { "app" } else { "os" }
        )))?;
        b.eval("window.releaseLoading();true")?;
        b.wait("!!document.querySelector('.api-ticket .card-title')")?;
        b.wait("!window.decodeSeen && !document.querySelector('[data-ar-t]')")?;
        b.call(
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":"prefers-reduced-motion","value":"no-preference"}]}),
        )?;
    }
    // Switching motion off during a decode restores the content immediately.
    b.go(&format!("{}/board", server.url))?;
    b.wait("!!document.querySelector('.ars canvas')")?;
    b.eval("window.releaseLoading();true")?;
    b.wait("!!document.querySelector('[data-ar-t]')")?;
    b.eval("document.documentElement.dataset.motion='reduce';true")?;
    b.wait("!!document.querySelector('.api-ticket .card-title') && !document.querySelector('[data-ar-t]') && !document.querySelector('.ars canvas')")?;
    // Dispose a loading page, release its fetches, and ensure old controllers do
    // not mutate the next route or leave animation canvases behind.
    b.go(&format!("{}/t/TKT-1", server.url))?;
    b.wait("!!document.querySelector('.ars canvas')")?;
    b.click("a[href='/projects']")?;
    b.eval("window.releaseLoading();true")?;
    b.wait("!!document.querySelector('.api-project-row') && !document.querySelector('[data-ar-t]') && !document.querySelector('.ars canvas')")?;
    b.viewport(390, 844)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    println!(
        "PASS loading: real routes and search, moving package sweeps, decode arrival, repeated busy buttons, OS/app reduced motion, interrupted decode, disposal and mobile overflow"
    );
    Ok(())
}

/// Cold navigation includes module/CSS/Wasm arrival, held skeletons and live decode.
/// Compare with settled extents, allowing the page's legitimate vertical scrolling
/// and the board's intentional horizontal scrolling without allowing loader growth.
fn cold_overflow(root: &std::path::Path, url: &str, phase: &str) -> super::Result {
    let output = env::var_os("TICKETS_LOADING_SHOTS")
        .map(|p| PathBuf::from(p).join("cold"))
        .unwrap_or_else(|| root.join("docs/screenshots/TIK-62"));
    fs::create_dir_all(&output)?;
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    for (width, height) in [(1280, 800), (390, 844)] {
        for (name, route, ready) in [
            ("board", "/board", ".api-ticket .card-title"),
            ("list", "/list", ".row-title"),
            ("detail", "/t/TKT-1", ".ticket-detail h1"),
            ("projects", "/projects", ".api-project-row"),
            ("home", "/", ".home-metric strong"),
        ] {
            let mut b = Browser::start_named(root, "loading-cold")?;
            b.viewport(width, height)?;
            b.call("Network.setCacheDisabled", json!({"cacheDisabled":true}))?;
            b.call("Network.emulateNetworkConditions", json!({"offline":false,"latency":150,"downloadThroughput":200_000,"uploadThroughput":75_000,"connectionType":"cellular3g"}))?;
            b.call("Emulation.setCPUThrottlingRate", json!({"rate":4}))?;
            b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":format!(r#"
                window.loadingHeld=true; window.loadingWaiters=[];
                const fetchBeforeLoading=window.fetch;
                window.fetch=async (...args)=>{{
                    const response=await fetchBeforeLoading(...args);
                    const url=args[0] instanceof Request ? args[0].url : String(args[0]);
                    if(url.includes('/api/v1/') && window.loadingHeld)
                        await new Promise(r=>window.loadingWaiters.push(r));
                    return response;
                }};
                window.releaseLoading=()=>{{window.loadingHeld=false;window.loadingWaiters.forEach(r=>r());}};
                {}
            "#, include_str!("loading_probe.js"))}))?;
            b.call("Page.navigate", json!({"url":format!("{url}{route}")}))?;
            let start = Instant::now();
            while b.eval(
                "!!document.querySelector('.ars canvas') && window.loadingWaiters?.length>0",
            )? != true
            {
                if start.elapsed() > Duration::from_secs(120) {
                    return Err(format!("Cold loading timed out: {name} {width}").into());
                }
                thread::sleep(Duration::from_millis(100));
            }
            b.wait(
                "[...document.querySelectorAll('link[rel=stylesheet]')].every(link=>!!link.sheet)",
            )?;
            b.eval("document.fonts.ready.then(()=>true)")?;
            // Capture the held Home skeleton: its unbounded sweep canvases are
            // the reproducible two-axis failure. The marker has no layout size.
            if name == "home" {
                b.eval(r#"(()=>{const d=document.documentElement,m=document.createElement('canvas');
                    m.width=innerWidth;m.height=innerHeight;m.style='position:fixed;inset:0;pointer-events:none;z-index:2147483647';m.id='overflow-evidence';
                    const c=m.getContext('2d'),x=d.scrollWidth>d.clientWidth,y=d.scrollHeight>d.clientHeight;
                    c.fillStyle='#eb5757';if(x)c.fillRect(0,innerHeight-6,innerWidth,6);if(y)c.fillRect(innerWidth-6,0,6,innerHeight);
                    c.font='12px monospace';c.fillText(`document ${d.scrollWidth}×${d.scrollHeight} / client ${d.clientWidth}×${d.clientHeight}`,12,innerHeight-16);
                    document.body.append(m);return true;})()"#)?;
                let png = b.call(
                    "Page.captureScreenshot",
                    json!({"format":"png","captureBeyondViewport":false}),
                )?;
                fs::write(
                    output.join(format!("{phase}-home-{width}.png")),
                    STANDARD.decode(png["data"].as_str().ok_or("missing PNG")?)?,
                )?;
                b.eval("document.querySelector('#overflow-evidence').remove();true")?;
            }
            b.eval("window.releaseLoading();true")?;
            b.wait(&format!("!!document.querySelector('{ready}') && !document.querySelector('.ars, .ar-overlay canvas, [data-ar-t]')"))?;
            b.eval("new Promise(r=>setTimeout(()=>r(true),500))")?;
            let report = b.eval(r#"(()=>{
                window.loadingProbeDone=true;
                const frames=window.loadingFrames, settled=frames.at(-1);
                const violations=frames.filter(f=>['document','main'].some(scope=>f[scope] && settled[scope] &&
                    (f[scope].width>Math.max(settled[scope].width,settled[scope].clientWidth)+1 ||
                     f[scope].height>Math.max(settled[scope].height,settled[scope].clientHeight)+1)));
                const peak=scope=>frames.reduce((p,f)=>({width:Math.max(p.width,f[scope]?.width||0),height:Math.max(p.height,f[scope]?.height||0)}),{width:0,height:0});
                return {settled, samples:frames.length, phases:Object.fromEntries(['skeleton','decode','content'].map(p=>[p,frames.filter(f=>f.phase===p).length])),
                    peakDocument:peak('document'),peakMain:peak('main'),violationCount:violations.length,
                    violations:violations.filter(f=>f.offenders).slice(0,8)};
            })()"#)?;
            let count = report["violationCount"]
                .as_u64()
                .ok_or("missing overflow count")?;
            println!(
                "{phase} cold {name} {width}×{height}: {count} overflow frames; settled {}",
                report["settled"]
            );
            if count > 0 {
                failures.push(format!("{name} {width}×{height}: {count} frames"));
            }
            for stage in ["skeleton", "decode"] {
                if report["phases"][stage].as_u64().unwrap_or(0) == 0 {
                    failures.push(format!(
                        "{name} {width}×{height}: no sampled {stage} frames"
                    ));
                }
            }
            reports.push(json!({"route":route,"width":width,"height":height,"report":report}));
        }
    }
    fs::write(
        output.join(format!("{phase}-overflow.json")),
        serde_json::to_vec_pretty(&reports)?,
    )?;
    if phase != "before" && !failures.is_empty() {
        return Err(format!("Transient loading overflow: {}", failures.join(", ")).into());
    }
    Ok(())
}

/// Check the package's rendered palette, including its injected stylesheet.
/// Transparent glyph hosts must expose the enclosing page/card/button surface.
fn assert_surfaces(b: &mut Browser) -> super::Result {
    let report = b.eval(r#"(() => {
        const failures=[];
        const clear=el=>getComputedStyle(el).backgroundColor==='rgba(0, 0, 0, 0)';
        for(const el of document.querySelectorAll('.ascii-skeleton, .ars, .ars-line, .ars-block, .ars-base, .loading-skeleton, .home-loading, .analytics-loading, .loading-indicator')) {
            if(!clear(el)) failures.push({class:el.className,background:getComputedStyle(el).backgroundColor});
        }
        const ink=document.createElement('canvas').getContext('2d');
        const color=value=>{ink.fillStyle=value;return ink.fillStyle;};
        for(const el of document.querySelectorAll('.skeleton-card')) {
            const s=getComputedStyle(el);
            if(color(s.backgroundColor)!==color(s.getPropertyValue('--surface').trim()))
                failures.push({class:el.className,reason:'card surface differs from --surface'});
        }
        for(const el of document.querySelectorAll('.ars')) {
            const s=getComputedStyle(el);
            if(parseFloat(s.getPropertyValue('--ars-wave'))>.25 || parseFloat(s.getPropertyValue('--ars-base'))>.15)
                failures.push({class:el.className,reason:'high-contrast glyphs'});
        }
        return failures;
    })()"#)?;
    if report.as_array().is_none_or(|a| !a.is_empty()) {
        return Err(format!("Loading surface mismatch: {report}").into());
    }
    Ok(())
}
