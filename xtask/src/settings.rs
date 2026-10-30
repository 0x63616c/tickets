//! TIK-40: fixture screenshots and an isolated authenticated settings journey.
use crate::{
    component::{FixtureServer, visible_crop},
    design::Browser,
};
use std::{fs, path::Path};

pub fn shots(root: &Path) -> super::Result {
    let output = root.join("docs/screenshots/TIK-40");
    fs::create_dir_all(&output)?;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS");
    let assets = before
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let server = FixtureServer::start(&assets)?;
    let prefix = if before.is_some() { "before" } else { "after" };
    for section in ["account", "api-tokens", "projects", "appearance"] {
        let mut b = Browser::start_named(root, "settings-shots")?;
        b.viewport(1440, 900)?;
        // These sections did not exist before TIK-40: show the actual prior settings entry point.
        let route = if before.is_some() {
            "/settings/tokens".to_string()
        } else {
            format!("/design/settings/{section}")
        };
        b.go(&format!("{}{route}", server.url))?;
        b.wait("!!document.querySelector('.settings-panel, .settings-section')")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        visible_page(
            &mut b,
            &output.join(format!("{prefix}-settings-{section}.png")),
        )?;
        visible_crop(
            &mut b,
            if before.is_some() {
                ".auth-page"
            } else {
                ".settings-content"
            },
            &output.join(format!("{prefix}-section-{section}.png")),
        )?;
    }
    let mut b = Browser::start_named(root, "settings-shell")?;
    b.viewport(1440, 900)?;
    b.go(&format!("{}/design/board", server.url))?;
    b.wait("!!document.querySelector('.sidebar')")?;
    visible_page(&mut b, &output.join(format!("{prefix}-shell.png")))?;
    drop(b);
    let mut b = Browser::start_named(root, "settings-mobile")?;
    b.viewport(390, 844)?;
    b.go(&format!(
        "{}{}",
        server.url,
        if before.is_some() {
            "/settings/tokens"
        } else {
            "/design/settings/account"
        }
    ))?;
    b.wait("!!document.querySelector('.settings-panel, .settings-section')")?;
    visible_page(
        &mut b,
        &output.join(format!("{prefix}-settings-mobile.png")),
    )?;
    println!("PASS settings screenshots: fixture-only, dark, desktop and mobile");
    Ok(())
}

pub fn check(root: &Path, password: &str) -> super::Result {
    let mut b = Browser::start_named(root, "settings-check")?;
    b.viewport(1440, 900)?;
    b.go("http://127.0.0.1:8795/login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/' && !!document.querySelector('.user-menu')")?;
    b.go("http://127.0.0.1:8795/settings")?;
    b.wait("location.pathname==='/settings/account' && !document.querySelector('#display-name')?.disabled")?;
    b.text("#display-name", "Sample User")?;
    b.click(".settings-save button")?;
    b.wait("document.querySelector('.settings-save').textContent.includes('Changes saved.')")?;
    b.go("http://127.0.0.1:8795/settings/account")?;
    b.wait("document.querySelector('#display-name')?.value==='Sample User'")?;
    b.click(".user-menu .account-trigger")?;
    b.wait("document.querySelector('.user-menu .account-trigger').getAttribute('aria-expanded')==='true' && document.querySelector('.account-menu-heading').textContent.includes('Sample User')")?;
    b.wait("document.activeElement.matches('.user-menu [role=menuitem]')")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.matches('.user-menu .account-trigger')")?;
    b.click("[aria-controls=sidebar-projects]")?;
    b.wait("document.querySelector('#sidebar-projects').hidden")?;
    b.go("http://127.0.0.1:8795/settings/account")?;
    b.wait("document.querySelector('#sidebar-projects')?.hidden===true")?;
    b.click("[aria-controls=sidebar-projects]")?;
    b.go("http://127.0.0.1:8795/settings/tokens")?;
    b.wait("location.pathname==='/settings/api-tokens' && !document.querySelector('#token-label')?.disabled")?;
    b.text("#token-label", "Settings check")?;
    b.click(".settings-footer button")?;
    b.wait(
        "!!document.querySelector('.secret') && document.querySelectorAll('.tokens li').length===1",
    )?;
    b.click(".secret .button-ghost")?;
    b.wait("!document.querySelector('.secret')")?;
    b.click(".tokens button")?;
    b.wait("document.querySelector('.tokens').textContent.includes('Revoked')")?;
    b.go("http://127.0.0.1:8795/settings/appearance")?;
    b.wait("!!document.querySelector('[aria-label=Motion]')")?;
    b.choose("[aria-label=Motion]", "reduce")?;
    b.wait("document.documentElement.dataset.motion==='reduce'")?;
    b.go("http://127.0.0.1:8795/settings/appearance")?;
    b.wait("document.documentElement.dataset.motion==='reduce' && document.querySelector('[aria-label=Motion]')?.textContent.includes('Reduce motion')")?;
    b.choose("[aria-label=Theme]", "light")?;
    b.wait("document.documentElement.dataset.theme==='light'")?;
    b.choose("[aria-label=Theme]", "dark")?;
    b.go("http://127.0.0.1:8795/home")?;
    b.wait("!!document.querySelector('.empty-project')")?;
    b.eval("document.activeElement.blur();true")?;
    b.key("g", 71, 0)?;
    b.key("l", 76, 0)?;
    b.wait("location.pathname==='/list'")?;
    b.key("g", 71, 0)?;
    b.key("b", 66, 0)?;
    b.wait("location.pathname==='/'")?;
    b.key("?", 191, 8)?;
    b.wait("!!document.querySelector('dialog[open]') && document.querySelector('dialog').textContent.includes('Keyboard shortcuts')")?;
    b.key("Escape", 27, 0)?;
    b.viewport(390, 844)?;
    b.go("http://127.0.0.1:8795/settings/account")?;
    b.wait("!!document.querySelector('#display-name')")?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.click("[aria-label='Toggle sidebar']")?;
    b.wait("document.querySelector('.app-shell').classList.contains('sidebar-open')")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.getAttribute('aria-label')==='Toggle sidebar'")?;
    b.sign_out()?;
    b.wait("location.pathname==='/login'")?;
    println!(
        "PASS settings: account persistence, menu focus, collapsed sections, token redirect/lifecycle, appearance persistence, Home and G shortcuts, mobile overflow/focus"
    );
    Ok(())
}

fn visible_page(b: &mut Browser, path: &Path) -> super::Result {
    use base64::{Engine, engine::general_purpose::STANDARD};
    b.eval("new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(()=>r(true))))")?;
    let png = b.call(
        "Page.captureScreenshot",
        serde_json::json!({"format":"png","fromSurface":false}),
    )?;
    fs::write(
        path,
        STANDARD.decode(png["data"].as_str().ok_or("missing PNG")?)?,
    )?;
    Ok(())
}
