//! TIK-41 fixture-only full-page and cropped token states.
use crate::{
    component::{FixtureServer, crop},
    design::Browser,
};
use std::{fs, path::Path};

pub fn shots(root: &Path) -> super::Result {
    let output = std::env::var_os("TICKETS_SCREENSHOT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("docs/screenshots/TIK-41"));
    fs::create_dir_all(&output)?;
    let server = FixtureServer::start(&root.join("crates/web/dist"))?;
    for (size, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
        let mut b = Browser::start_named(root, "tokens-shots")?;
        b.viewport(width, height)?;
        b.go(&format!(
            "{}/design/settings/api-tokens?empty=true",
            server.url
        ))?;
        b.wait("document.body.innerText.includes('No active API tokens')")?;
        if width < 600 {
            b.wait(
                "document.querySelector('.token-empty .button').getBoundingClientRect().height>=44",
            )?;
        }
        capture(&mut b, &output, "empty", size, ".settings-content")?;
        b.go(&format!("{}/design/settings/api-tokens", server.url))?;
        b.wait("document.querySelectorAll('.tokens li').length===3")?;
        b.wait("!document.querySelector('.token-access') && !document.body.innerText.includes('No expiration')")?;
        b.wait("document.querySelector('.token-list-heading').getBoundingClientRect().top-document.querySelector('.settings-description').getBoundingClientRect().bottom<=16")?;
        capture(&mut b, &output, "list", size, ".settings-content")?;
        b.click(".token-history input")?;
        b.wait("document.querySelectorAll('.tokens li').length===4")?;
        capture(&mut b, &output, "history", size, ".settings-content")?;
        b.click(".token-create")?;
        b.wait("document.activeElement.id==='token-label'")?;
        b.wait("document.querySelector('.token-grant h3')?.textContent==='Full workspace access'")?;
        b.text("#token-label", "Fixture · Documentation example")?;
        capture(&mut b, &output, "create", size, "dialog")?;
        b.click(".settings-footer button[type=submit]")?;
        b.wait("document.querySelector('.secret textarea')?.value.startsWith('tk_FIXTURE_ONLY_') && document.activeElement.textContent.includes('Copy token')")?;
        b.key("Escape", 27, 0)?;
        b.wait("!!document.querySelector('.secret') && document.querySelector('.token-acknowledge').disabled")?;
        capture(&mut b, &output, "secret", size, "dialog")?;
        b.call("Browser.grantPermissions", serde_json::json!({"origin":server.url,"permissions":["clipboardReadWrite","clipboardSanitizedWrite"]}))?;
        b.click(".secret > button")?;
        b.wait("document.querySelector('#token-copy-status').textContent==='Token copied.'")?;
        b.wait("navigator.clipboard.readText().then(value=>value===document.querySelector('.secret textarea').value)")?;
        capture(&mut b, &output, "secret-copied", size, "dialog")?;
        b.click(".token-discard-trigger")?;
        b.wait("!!document.querySelector('.token-discard') && !!document.querySelector('.secret textarea')")?;
        capture(&mut b, &output, "secret-discard", size, "dialog")?;
        b.click(".token-keep")?;
        b.click(".token-saved input")?;
        b.click(".token-acknowledge")?;
        b.wait("!document.querySelector('dialog') && !document.querySelector('.secret') && document.activeElement.matches('.token-create')")?;
        b.click(".tokens li:first-child button")?;
        b.wait("document.querySelector('dialog')?.textContent.includes('Fixture · Documentation example')")?;
        capture(&mut b, &output, "revoke", size, "dialog")?;
        b.key("Escape", 27, 0)?;
        b.wait("!document.querySelector('dialog') && document.activeElement.matches('.tokens li:first-child button')")?;
        b.click(".tokens li:first-child button")?;
        b.click(".modal-footer .button-danger")?;
        b.wait("!document.querySelector('dialog') && document.querySelector('.token-result').textContent.includes('Revoked Fixture')")?;
        capture(&mut b, &output, "revoked", size, ".settings-content")?;
        b.go(&format!("{}/design/settings/api-tokens", server.url))?;
        b.wait("!document.querySelector('.secret') && document.querySelectorAll('.tokens li').length===3")?;
    }
    println!(
        "PASS token fixtures: empty/list/history/create/show-once/revoke, mobile overflow, focus return, Escape guard, acknowledgement and no secret persistence; invalid fixture secret only"
    );
    Ok(())
}
fn capture(
    b: &mut Browser,
    output: &Path,
    state: &str,
    size: &str,
    selector: &str,
) -> super::Result {
    b.eval("document.fonts.ready.then(()=>true)")?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join(format!("after-{state}-{size}.png")))?;
    let viewport = b.eval("({width:innerWidth,height:innerHeight})")?;
    b.call("Emulation.setDeviceMetricsOverride", serde_json::json!({"width":viewport["width"],"height":viewport["height"],"deviceScaleFactor":2,"mobile":false}))?;
    crop(
        b,
        selector,
        &output.join(format!("after-{state}-{size}-component.png")),
    )?;
    b.viewport(
        viewport["width"].as_u64().unwrap() as u32,
        viewport["height"].as_u64().unwrap() as u32,
    )?;
    Ok(())
}
