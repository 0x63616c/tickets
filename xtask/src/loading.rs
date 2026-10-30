//! Held fixture responses on the real routes; no delay or mocked animation in the app.
use crate::{component::FixtureServer, design::Browser};
use serde_json::{Value, json};
use std::{collections::HashMap, env, fs, path::PathBuf};

pub fn check(root: &std::path::Path) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS");
    let phase = if before.is_some() { "before" } else { "after" };
    let assets = before
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let output = root.join("docs/screenshots/TIK-47");
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
        ("home", "/", "document.querySelector('.home-metric strong')"),
        (
            "account",
            "/settings/account",
            "document.body.textContent.includes('sample-user')",
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
        if matches!(name, "board" | "detail")
            || (phase == "after" && matches!(name, "list" | "projects"))
        {
            b.eval("new Promise(r=>setTimeout(()=>r(true),700))")?;
            b.screenshot(&output.join(format!("{phase}-{name}.png")))?;
        }
        b.eval("window.releaseLoading();true")?;
        b.wait(&format!("!!({ready})"))?;
        if phase == "after" {
            b.wait("window.decodeSeen")?;
            b.wait(
                "!document.querySelector('.ars canvas') && !document.querySelector('[data-ar-t]')",
            )?;
        }
    }
    if phase == "before" {
        return Ok(());
    }
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
