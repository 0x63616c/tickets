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
    let output = root.join("docs/screenshots/TIK-66");
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
    for field in ["review_queue", "blocked_queue"] {
        scoped[field] = Value::Array(
            scoped[field]
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| t["key"].as_str().unwrap_or("").starts_with("TKT-"))
                .cloned()
                .collect(),
        );
    }

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
    b.wait(if phase=="after"{"document.querySelectorAll('.home-metric strong').length===3 && !document.querySelector('.home-page[aria-busy=true]')"}else{"document.querySelectorAll('.home-metric strong').length===4"})?;
    b.wait("!document.querySelector('#boot-splash,[data-ar-t]')")?;
    b.eval("new Promise(r=>setTimeout(()=>r(true),750))")?;
    for (size, width) in [("desktop", 1440), ("mobile", 390)] {
        b.viewport(width, 900)?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        b.screenshot(&output.join(format!("{phase}-home-{size}.png")))?;
    }
    if phase == "before" {
        return Ok(());
    }
    b.viewport(1440, 900)?;
    b.wait("!document.querySelector('.home-charts') && !document.querySelector('.analytics-card') && document.querySelectorAll('.home-waiting tbody tr').length===5")?;
    b.eval("window.analyticsReads=[];window.analyticsMinute=Math.floor((performance.timeOrigin+performance.getEntriesByType('resource').find(r=>r.name.includes('/api/v1/analytics/home')).startTime)/60000);window.beforeAnalyticsFetch=window.fetch;window.fetch=(input,init)=>{const url=input instanceof Request?input.url:String(input);window.analyticsReads.push(url);return window.beforeAnalyticsFetch(input,init)};true")?;
    b.click("a[href='/analytics']")?;
    b.wait("document.querySelectorAll('.analytics-card').length===2 && !document.querySelector('.home-page[aria-busy=true]')")?;
    b.eval("if(analyticsReads.some(p=>p.includes('/tickets'))||(Math.floor(Date.now()/60000)===analyticsMinute&&analyticsReads.some(p=>p.includes('/analytics/home'))))throw Error('Navigation redundantly fetched analytics or tickets: '+JSON.stringify(analyticsReads));if(!document.querySelector('a[href=\"/analytics\"][aria-current=page]'))throw Error('Analytics nav inactive');true")?;
    b.eval("new Promise(r=>setTimeout(()=>r(true),750))")?;
    b.wait("!document.querySelector('[data-ar-t]')")?;
    b.call(
        "Input.dispatchMouseEvent",
        json!({"type":"mouseMoved","x":1100,"y":16}),
    )?;
    // A fresh desktop document avoids headless compositor text-cache artifacts
    // after crossing the sidebar breakpoint; cache reuse was asserted above.
    b.go(&format!("{}/analytics", server.url))?;
    b.wait("document.querySelectorAll('.analytics-card').length===2 && !document.querySelector('#boot-splash,[data-ar-t]')")?;
    b.eval("new Promise(r=>setTimeout(()=>r(true),750))")?;
    for (size, width) in [("desktop", 1440), ("tablet", 720), ("mobile", 390)] {
        b.viewport(width, 900)?;
        b.wait("document.documentElement.scrollWidth<=innerWidth && [...document.querySelectorAll('.chart-plot-wrap')].every(p=>p.scrollWidth<=p.clientWidth+1)")?;
        b.wait("(()=>{const labels=[...document.querySelectorAll('.flow-week-label')].filter(e=>getComputedStyle(e).display!=='none').map(e=>e.getBoundingClientRect());return labels.every((r,i)=>!i||r.left>labels[i-1].right+1)})()")?;
        b.screenshot(&output.join(format!("after-analytics-{size}.png")))?;
    }
    b.viewport(1440, 900)?;
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1440,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    crop(&mut b, ".analytics-grid", &output.join("after-charts.png"))?;
    b.eval("document.querySelector('.chart-point').focus();true")?;
    b.wait("!!document.querySelector('.chart-tooltip') && document.querySelector('.chart-tooltip').textContent.includes('Unavailable')")?;
    crop(
        &mut b,
        ".analytics-grid",
        &output.join("after-chart-focus.png"),
    )?;
    b.key("Escape", 27, 0)?;
    b.wait("!document.querySelector('.chart-tooltip') && document.activeElement.matches('.chart-point')")?;
    b.click(".chart-data summary")?;
    b.wait("document.querySelector('.chart-data tbody').rows.length===8 && document.querySelector('.chart-data tbody').textContent.includes('Unavailable')")?;
    b.key("Tab", 9, 0)?;
    b.wait("document.activeElement.matches('.chart-data a') && getComputedStyle(document.activeElement).outlineStyle!=='none'")?;
    crop(
        &mut b,
        ".analytics-grid",
        &output.join("after-chart-table.png"),
    )?;
    b.viewport(1440, 900)?;
    b.go(&format!("{}/", server.url))?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    b.click("a.home-metric[href*='metric=review']")?;
    b.wait("document.querySelectorAll('.home-cohort .home-ticket-row').length===5")?;
    b.go(&format!("{}/", server.url))?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    b.choose("[aria-label='Home project']", "1")?;
    b.wait("location.search==='?project=1'")?;
    b.go(&format!("{}/home?project=1", server.url))?;
    b.wait("document.querySelectorAll('.home-waiting tbody tr').length===1 && document.querySelector('.home-metric strong').textContent==='7'")?;
    b.wait(
        "document.querySelector('.home-section-heading a[href=\"/analytics?project=1\"]')!==null",
    )?;
    b.go(&format!("{}/analytics?project=1", server.url))?;
    b.wait("document.querySelectorAll('.inventory-plot .chart-point').length===1 && !!document.querySelector('a[href=\"/?project=1\"]')")?;
    b.choose("[aria-label='Analytics project']", "")?;
    b.wait("location.pathname==='/analytics' && location.search==='' && document.querySelectorAll('.inventory-plot .chart-point').length===5")?;
    b.go(&format!("{}/home?project=1", server.url))?;
    b.wait("document.querySelector('.home-metric strong').textContent==='7'")?;
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
    b.go(&format!("{}/analytics", server.url))?;
    b.wait("!!window.releaseHome && !!document.querySelector('.analytics-loading') && !document.querySelector('.analytics-card svg')")?;
    b.screenshot(&output.join("after-analytics-loading.png"))?;
    b.eval("window.releaseHome();true")?;
    b.wait("document.querySelectorAll('.analytics-card svg').length===2 && !document.querySelector('.analytics-loading')")?;
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
    b.go(&format!("{}/", server.url))?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    // Full document capture includes the queues on mobile.
    let height = b
        .eval("Math.ceil(document.querySelector('.home-page').scrollHeight + 96)")?
        .as_u64()
        .ok_or("document height")?;
    b.viewport(390, height as u32)?;
    b.screenshot(&output.join("after-home-mobile-full.png"))?;
    b.go(&format!("{}/analytics", server.url))?;
    b.wait("document.querySelectorAll('.analytics-card').length===2")?;
    let height = b
        .eval("Math.ceil(document.querySelector('.home-page').scrollHeight+96)")?
        .as_u64()
        .ok_or("analytics height")?;
    b.viewport(390, height as u32)?;
    b.screenshot(&output.join("after-analytics-mobile-full.png"))?;
    let mut empty: Value = serde_json::from_str(include_str!("../../docs/fixtures/home.json"))?;
    empty["projects"] = json!([]);
    for week in empty["weeks"].as_array_mut().unwrap() {
        week["created"] = Value::Null;
        week["completed"] = Value::Null;
        week["coverage"] = json!("none");
    }
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
    b.go(&format!("{}/analytics", empty_server.url))?;
    b.wait("!!document.querySelector('.chart-empty') && document.querySelectorAll('.analytics-card').length===2")?;
    b.screenshot(&output.join("after-analytics-empty.png"))?;
    b.go(&format!("{}/design/home", server.url))?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    let modifier = if b.eval("navigator.platform.includes('Mac')")? == true {
        4
    } else {
        2
    };
    b.key("2", 50, modifier)?;
    b.wait(
        "location.pathname==='/design/analytics' && !!document.querySelector('.analytics-card')",
    )?;
    b.key("k", 75, 4)?;
    b.wait("!!document.querySelector('.command-search input')")?;
    b.text(".command-search input", "Go to analytics")?;
    b.wait("[...document.querySelectorAll('.command-option')].some(e=>e.textContent.includes('Go to analytics'))")?;
    b.key("End", 35, 4)?;
    b.wait("document.querySelector('.command-option[aria-selected=true]')?.textContent.includes('Go to analytics')")?;
    b.key("Enter", 13, 0)?;
    b.wait("!document.querySelector('dialog') && location.pathname==='/design/analytics'")?;
    println!(
        "PASS home-shot: fixture desktop/mobile, charts/table, keyboard focus, project scope, cohort navigation, refresh error/retry, loading, reduced motion and overflow"
    );
    Ok(())
}
