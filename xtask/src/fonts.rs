//! TIK-83: font persistence, pre-mount boot behavior and fixture-only evidence.
use crate::{component::FixtureServer, design::Browser};
use serde_json::json;
use std::{fs, path::Path};

pub fn check(root: &Path) -> super::Result {
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS");
    let assets = before
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let server = FixtureServer::start(&assets)?;
    let output = root.join("docs/screenshots/TIK-83");
    fs::create_dir_all(&output)?;
    if before.is_some() {
        for (route, name) in [("settings/appearance", "settings"), ("board", "board")] {
            capture(
                root,
                &server.url,
                None,
                route,
                &output.join(format!("before-{name}.png")),
                false,
            )?;
        }
        println!("PASS baseline screenshots");
        return Ok(());
    }
    let mut b = Browser::start_named(root, "font-check")?;
    let mut families = std::collections::HashSet::new();
    for (font, family) in [
        ("geist", "Geist"),
        ("geist-mono", "Geist Mono"),
        ("inter", "Inter Variable"),
        ("system", "-apple-system"),
    ] {
        b.go(&format!("{}/design/settings/appearance", server.url))?;
        b.wait("!!document.querySelector('[aria-label=Font]:not(:disabled)')")?;
        b.choose("[aria-label=Font]", font)?;
        b.wait(&format!("document.documentElement.dataset.font==='{font}' && localStorage.getItem('tickets.font')==='{font}'"))?;
        let computed = b.eval("getComputedStyle(document.body).fontFamily")?;
        if !computed.as_str().is_some_and(|s| s.contains(family))
            || !families.insert(computed.to_string())
        {
            return Err(
                format!("font choice {font} did not change body family: {computed}").into(),
            );
        }
        boot_check(root, &server.url, font)?;
        b.go(&format!("{}/design/settings/appearance", server.url))?;
        b.wait(&format!(
            "document.querySelector('[aria-label=Font]')?.textContent.includes('{}')",
            if font == "geist" {
                "Geist"
            } else if font == "geist-mono" {
                "Geist Mono"
            } else if font == "inter" {
                "Inter"
            } else {
                "System"
            }
        ))?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        b.wait("performance.getEntriesByType('resource').filter(e=>/\\.woff2/.test(e.name)).every(e=>new URL(e.name).origin===location.origin)")?;
        b.wait("getComputedStyle(document.querySelector('.font-preview')).fontFamily===getComputedStyle(document.body).fontFamily && document.documentElement.scrollWidth<=innerWidth")?;

        b.go(&format!("{}/design/board", server.url))?;
        b.wait("!!document.querySelector('.board')")?;
        b.wait("getComputedStyle(document.querySelector('.mono, code')).fontFamily.includes('Geist Mono')")?;

        b.viewport(390, 844)?;
        b.go(&format!("{}/design/settings/appearance", server.url))?;
        b.wait("!!document.querySelector('.font-preview') && document.documentElement.scrollWidth<=innerWidth")?;
        b.viewport(1440, 900)?;
    }
    for value in ["unknown", "Inter", "", "old-font"] {
        b.eval(&format!(
            "localStorage.setItem('tickets.font',{});true",
            serde_json::to_string(value)?
        ))?;
        b.go(&format!("{}/design/settings/appearance", server.url))?;
        b.wait("document.documentElement.dataset.font==='geist' && getComputedStyle(document.body).fontFamily.startsWith('Geist,')")?;
    }
    b.eval("localStorage.removeItem('tickets.font');true")?;
    b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":"Object.defineProperty(window, 'localStorage', {get(){throw new DOMException('Storage disabled', 'SecurityError')}});"}))?;
    b.viewport(390, 844)?;
    b.go(&format!("{}/design/settings/appearance", server.url))?;
    b.wait("!!document.querySelector('.font-preview') && document.documentElement.scrollWidth<=innerWidth")?;
    b.wait("getComputedStyle(document.body).fontFamily.startsWith('Geist,')")?;
    drop(b);
    // A fresh profile per PNG avoids Chromium losing cached SVG glyphs after a second raster.
    for font in ["geist", "geist-mono", "inter", "system"] {
        for (route, name) in [("settings/appearance", "settings"), ("board", "board")] {
            capture(
                root,
                &server.url,
                Some(font),
                route,
                &output.join(format!("after-{font}-{name}.png")),
                false,
            )?;
        }
    }
    capture(
        root,
        &server.url,
        Some("geist"),
        "settings/appearance",
        &output.join("after-geist-settings-mobile.png"),
        true,
    )?;
    println!(
        "PASS fonts: four distinct UI families, shared Select, preview, mono invariance, persistence, pre-mount boot, invalid defaults and mobile fit"
    );
    Ok(())
}

fn capture(
    root: &Path,
    url: &str,
    font: Option<&str>,
    route: &str,
    output: &Path,
    mobile: bool,
) -> super::Result {
    let mut b = Browser::start_named(root, "font-shot")?;
    if let Some(font) = font {
        b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":format!("localStorage.setItem('tickets.font',{});", serde_json::to_string(font)?)}))?;
    }
    if mobile {
        b.viewport(390, 844)?;
    }
    b.go(&format!("{url}/design/{route}"))?;
    b.wait("!!document.querySelector('.font-preview, .board, [aria-label=Motion]')")?;
    b.eval("Promise.all(document.getAnimations().filter(a=>a.effect?.getTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{}))).then(()=>true)")?;
    b.eval("document.fonts.ready.then(()=>new Promise(r=>setTimeout(()=>r(true),300)))")?;
    crate::settings::visible_page(&mut b, output)
}

fn boot_check(root: &Path, url: &str, font: &str) -> super::Result {
    // Keep intentionally failed module loads out of the persistence journey's cache.
    let mut b = Browser::start_named(root, "font-boot")?;
    b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":format!("localStorage.setItem('tickets.font',{});", serde_json::to_string(font)?)}))?;
    b.call("Network.setBlockedURLs", json!({"urls":["*.js", "*.wasm"]}))?;
    b.call(
        "Page.navigate",
        json!({"url":format!("{url}/design/settings/appearance")}),
    )?;
    b.wait(&format!("document.documentElement?.dataset.font==='{font}' && !!document.body && !document.querySelector('.app-shell')"))
}
