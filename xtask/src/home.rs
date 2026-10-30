//! Deterministic Home evidence, served on an OS-assigned loopback port.
use crate::{
    component::{FixtureServer, crop},
    design::Browser,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

pub fn shots(root: &Path) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS");
    let phase = if before.is_some() { "before" } else { "after" };
    let assets = before
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let output = root.join("docs/screenshots/TIK-43");
    fs::create_dir_all(&output)?;
    let seed: Value = serde_json::from_str(include_str!("../../docs/fixtures/home-api.json"))?;
    let home: Value = serde_json::from_str(include_str!("../../docs/fixtures/home.json"))?;
    let mut review = home.clone();
    review["cohort"] = review["review_queue"].clone();
    review["cohort_total"] = review["cohort"].as_array().unwrap().len().into();
    let mut scoped = home.clone();
    scoped["in_flight"] = json!(7);
    scoped["in_review"] = json!(3);
    scoped["blocked"] = json!(1);
    let fixtures = HashMap::from([
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
        ("/api/v1/analytics/home".into(), home),
        ("/api/v1/analytics/home?metric=review".into(), review),
        ("/api/v1/analytics/home?project=1".into(), scoped),
    ]);
    let server = FixtureServer::start_with_api(&assets, fixtures)?;
    let mut b = Browser::start_named(root, "home")?;
    b.viewport(1440, 900)?;
    b.go(&format!("{}/", server.url))?;
    b.wait(if phase=="after"{"document.querySelectorAll('.home-metric strong').length===4 && !document.querySelector('.home-page[aria-busy=true]')"}else{"document.querySelectorAll('.api-ticket').length===20"})?;
    for (size, width) in [("desktop", 1440), ("mobile", 390)] {
        b.viewport(width, 900)?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        b.screenshot(&output.join(format!("{phase}-home-{size}.png")))?;
    }
    if phase == "before" {
        return Ok(());
    }
    b.viewport(1440, 900)?;
    crop(&mut b, ".home-charts", &output.join("after-charts.png"))?;
    b.click(".chart-data summary")?;
    b.wait("document.querySelectorAll('.chart-data tbody tr').length===8 && document.querySelector('.chart-data tbody').textContent.includes('Unavailable')")?;
    b.key("Tab", 9, 0)?;
    b.wait("document.activeElement.matches('.chart-data a') && getComputedStyle(document.activeElement).outlineStyle!=='none'")?;
    crop(
        &mut b,
        ".home-charts",
        &output.join("after-chart-table.png"),
    )?;
    b.click("a.home-metric[href*='metric=review']")?;
    b.wait("document.querySelectorAll('.home-cohort .home-ticket-row').length===5")?;
    b.go(&format!("{}/", server.url))?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    b.click("[aria-label='Home project']")?;
    b.key("ArrowDown", 40, 0)?;
    b.key("Enter", 13, 0)?;
    b.wait("location.search==='?project=2' || location.search==='?project=1'")?;
    b.go(&format!("{}/home?project=1", server.url))?;
    b.wait("document.querySelectorAll('.project-chart-row').length===1 && document.querySelector('.home-metric strong').textContent==='7'")?;
    // Hold a scope request, fail a refresh, then retry without replacing saved data.
    b.eval("window.homeFetch=window.fetch;window.fetch=async(input,init)=>{const url=input instanceof Request?input.url:String(input);if(url.includes('/analytics/home'))throw Error('Fixture offline');return window.homeFetch(input,init)};true")?;
    b.click("[aria-label='Refresh Home']")?;
    b.wait("document.querySelector('.home-error')?.textContent.includes('Showing the last update') && document.querySelector('.home-metric strong').textContent==='7'")?;
    b.screenshot(&output.join("after-refresh-error.png"))?;
    b.eval("window.fetch=window.homeFetch;true")?;
    b.click(".home-error button")?;
    b.wait("!document.querySelector('.home-error') && document.querySelector('.home-page').getAttribute('aria-busy')==='false'")?;
    // Initial network failure is a settled error, not an endless skeleton.
    let failure_script=b.call("Page.addScriptToEvaluateOnNewDocument",json!({"source":"window.nativeFetch=window.fetch;window.fetch=async(input,init)=>{const url=input instanceof Request?input.url:String(input);if(url.includes('/analytics/home')&&!window.allowHome)throw Error('Fixture unavailable');return window.nativeFetch(input,init)};"}))?;
    b.go(&format!("{}/", server.url))?;
    b.wait("!!document.querySelector('.home-error') && !document.querySelector('.home-loading') && !document.querySelector('.home-metric strong')")?;
    b.screenshot(&output.join("after-initial-error.png"))?;
    b.eval("window.allowHome=true;true")?;
    b.click(".home-error button")?;
    b.wait(
        "!!document.querySelector('.home-metric strong') && !document.querySelector('.home-error')",
    )?;
    b.call(
        "Page.removeScriptToEvaluateOnNewDocument",
        json!({"identifier":failure_script["identifier"]}),
    )?;
    // A new document exercises real initial skeletons and scope cancellation.
    let script=b.call("Page.addScriptToEvaluateOnNewDocument",json!({"source":"window.nativeFetch=window.fetch;window.fetch=async(input,init)=>{const url=input instanceof Request?input.url:String(input);if(url.includes('/analytics/home'))await new Promise(resolve=>window.releaseHome=resolve);return window.nativeFetch(input,init)};"}))?;
    b.go(&format!("{}/", server.url))?;
    b.wait("!!window.releaseHome && !!document.querySelector('.home-loading')")?;
    b.screenshot(&output.join("after-loading.png"))?;
    b.eval("window.releaseHome();true")?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    b.call(
        "Page.removeScriptToEvaluateOnNewDocument",
        json!({"identifier":script["identifier"]}),
    )?;
    b.viewport(720, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.call(
        "Emulation.setEmulatedMedia",
        json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
    )?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    // Full document capture includes the queues on mobile.
    let height = b
        .eval("Math.ceil(document.querySelector('.home-page').scrollHeight + 96)")?
        .as_u64()
        .ok_or("document height")?;
    b.viewport(390, height as u32)?;
    b.screenshot(&output.join("after-home-mobile-full.png"))?;
    let mut empty: Value = serde_json::from_str(include_str!("../../docs/fixtures/home.json"))?;
    empty["projects"] = json!([]);
    let empty_server = FixtureServer::start_with_api(
        &assets,
        HashMap::from([
            (
                "/api/v1/me".into(),
                json!({"authenticated":true,"username":"sample-user","display_name":"Sample User"}),
            ),
            (
                "/api/v1/projects".into(),
                json!({"data":[],"next_cursor":null}),
            ),
            ("/api/v1/analytics/home".into(), empty),
        ]),
    )?;
    b.viewport(390, 900)?;
    b.go(&format!("{}/", empty_server.url))?;
    b.wait("!!document.querySelector('.home-empty a') && !document.querySelector('.home-charts') && document.querySelector('.home-empty a').getBoundingClientRect().bottom<innerHeight")?;
    b.screenshot(&output.join("after-empty-mobile.png"))?;
    println!(
        "PASS home-shot: fixture desktop/mobile, charts/table, keyboard focus, project scope, cohort navigation, refresh error/retry, loading, reduced motion and overflow"
    );
    Ok(())
}
