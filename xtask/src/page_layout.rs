//! TIK-69: fixture-only wide-window layout assertions and before/after evidence.
use crate::{component::FixtureServer, design::Browser};
use serde_json::json;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

pub fn check(root: &Path) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS");
    let phase = if before.is_some() { "before" } else { "after" };
    let assets = before
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let output = root.join("docs/screenshots/TIK-69");
    fs::create_dir_all(&output)?;
    let seed: serde_json::Value =
        serde_json::from_str(include_str!("../../docs/fixtures/home-api.json"))?;
    let home: serde_json::Value =
        serde_json::from_str(include_str!("../../docs/fixtures/home.json"))?;
    let server = FixtureServer::start_with_api(
        &assets,
        HashMap::from([
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
            ("/api/v1/tickets/TKT-1".into(), seed["tickets"][0].clone()),
            ("/api/v1/tickets/1".into(), seed["tickets"][0].clone()),
            ("/api/v1/analytics/home".into(), home),
        ]),
    )?;
    let mut b = Browser::start_named(root, "page-layout")?;
    b.call(
        "Emulation.setEmulatedMedia",
        json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
    )?;
    let mut measurements = Vec::new();
    for (name, route, ready) in [
        ("home", "/design/home", ".home-metric strong"),
        ("list", "/design/list", ".ticket-row"),
        ("ticket", "/design/ticket", ".ticket-detail"),
        ("settings", "/design/settings/account", ".settings-section"),
        ("board", "/design/board", ".board-column"),
    ] {
        for width in [1440, 1920, 2560, 1024, 390] {
            b.viewport(width, 900)?;
            b.go(&format!("{}{route}", server.url))?;
            if phase == "after" {
                remember_bounds(&mut b)?;
            }
            b.wait(&format!("!!document.querySelector('{ready}') && !document.querySelector('#boot-splash,[data-ar-t]')"))?;
            b.eval("document.fonts.ready.then(()=>true)")?;
            if phase == "after" {
                measurements.push(assert_layout(&mut b, route, width, name == "board")?);
            }
            // Capture the visible compositor surface; offscreen rasterization can
            // drop cached text in managed headless Chromium after navigation.
            crate::settings::visible_page(
                &mut b,
                &output.join(format!("{phase}-{name}-{width}.png")),
            )?;
            let height = b
                .eval("Math.max(innerHeight, document.documentElement.scrollHeight)")?
                .as_u64()
                .ok_or("document height")?;
            b.viewport(width, height as u32)?;
            crate::settings::visible_page(
                &mut b,
                &output.join(format!("{phase}-{name}-{width}-full.png")),
            )?;
        }
    }
    if phase == "after" {
        // API-backed implementations use the same container with immutable HTTP fixtures.
        for (route, ready, board) in [
            ("/", ".home-metric strong", false),
            ("/list", ".api-list-row", false),
            ("/t/TKT-1", ".ticket-detail", false),
            ("/board", ".board-column", true),
        ] {
            for width in [2560, 1024, 390] {
                b.viewport(width, 900)?;
                b.go(&format!("{}{route}", server.url))?;
                remember_bounds(&mut b)?;
                b.wait(&format!("!!document.querySelector('{ready}') && !document.querySelector('#boot-splash,[data-ar-t]')"))?;
                measurements.push(assert_layout(&mut b, route, width, board)?);
            }
        }
        // Every remaining surface uses the same shell, including recovery routes.
        for (route, ready) in [
            ("/design/analytics", ".analytics-card"),
            ("/design/projects", ".projects-page"),
            ("/design", ".gallery"),
            ("/design/empty", ".empty-project"),
            ("/design/mission", ".mission-page"),
            ("/design/settings/api-tokens", ".token-inventory"),
            ("/design/settings/projects", ".settings-section"),
            ("/design/settings/appearance", ".settings-section"),
            ("/design/settings/pickup-rules", ".pickup-settings"),
            ("/missing-page", ".empty-state"),
            ("/login", "#username"),
        ] {
            for width in [2560, 1024, 390] {
                b.viewport(width, 900)?;
                b.go(&format!("{}{route}", server.url))?;
                remember_bounds(&mut b)?;
                b.wait(&format!("!!document.querySelector('{ready}') && !document.querySelector('#boot-splash,[data-ar-t]')"))?;
                b.eval("document.fonts.ready.then(()=>true)")?;
                measurements.push(assert_layout(&mut b, route, width, false)?);
            }
        }
        fs::write(
            output.join("measurements.json"),
            serde_json::to_vec_pretty(&measurements)?,
        )?;
    }
    println!(
        "PASS page-layout-check: {phase} fixture screenshots; capped/centred pages, full-width board, sidebar and overflow at 2560/1920/1440/1024/390"
    );
    Ok(())
}

fn remember_bounds(b: &mut Browser) -> super::Result {
    b.wait("!!document.querySelector('main.page-container')")?;
    b.eval("(()=>{const r=document.querySelector('main.page-container').getBoundingClientRect();window.pageInitialBounds={left:r.left,width:r.width};return true})()")?;
    Ok(())
}

fn assert_layout(
    b: &mut Browser,
    route: &str,
    width: u32,
    board: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    b.eval(&format!(r#"(() => {{
        const main = document.querySelector('main.page-container');
        if (!main) throw Error('Missing shared page container: {route}');
        const r = main.getBoundingClientRect();
        if (Math.abs(r.left-window.pageInitialBounds.left)>1 || Math.abs(r.width-window.pageInitialBounds.width)>1)
            throw Error('Container shifted while content loaded: {route}');
        const parent = main.closest('.app-main') || document.body;
        const p = parent.getBoundingClientRect();
        const cap = parseFloat(getComputedStyle(main).getPropertyValue('--page-max-width'));
        if (!Number.isFinite(cap) || cap<=0 || cap>1360) throw Error('Page cap must remain bounded on ultrawide windows');
        const expected = {board} ? p.width : Math.min(p.width, cap);
        if (Math.abs(r.width-expected)>1 || Math.abs((r.left-p.left)-(p.right-r.right))>1)
            throw Error('Container not centred/capped: {route} '+JSON.stringify({{left:r.left,width:r.width,parent:p.width,cap}}));
        if (document.documentElement.scrollWidth>innerWidth+1) throw Error('Document overflow: {route}');
        const topbar = main.querySelector('.topbar');
        const gutter = parseFloat(getComputedStyle(main).getPropertyValue('--page-gutter'));
        if (topbar && Math.abs(parseFloat(getComputedStyle(topbar).paddingLeft)-gutter)>1) throw Error('Header gutter drift: {route}');
        if ({board}) {{
            for (const selector of ['.view-toolbar', '.board']) {{
                const e = main.querySelector(selector);
                if (!e || Math.abs(parseFloat(getComputedStyle(e).paddingLeft)-gutter)>1) throw Error('Board gutter drift: '+selector);
            }}
        }}
        const sidebar = document.querySelector('.sidebar');
        if (innerWidth>760 && sidebar && Math.abs(sidebar.getBoundingClientRect().width-240)>1) throw Error('Sidebar resized');
        return {{route:'{route}',viewport:{width},left:r.left,width:r.width,cap,board:{board}}};
    }})()"#))
}
