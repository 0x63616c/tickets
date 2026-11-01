//! TIK-136: compare rendered colors against the real pre-consolidation app.
use crate::{component::FixtureServer, design::Browser};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub fn check(root: &Path) -> crate::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS");
    let phase = if before.is_some() { "before" } else { "after" };
    let assets = before
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let output = root.join("target/TIK-136/colors");
    fs::create_dir_all(&output)?;
    let server = FixtureServer::start(&assets)?;
    let mut browser = Browser::start_named(root, "palette-colors")?;
    browser.call(
        "Emulation.setEmulatedMedia",
        json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
    )?;
    for (name, route, ready) in [
        ("board", "/design/board", ".board-column"),
        ("list", "/design/list", ".ticket-row"),
        ("ticket", "/design/ticket", ".ticket-detail"),
        ("home", "/design/home", ".home-metric strong"),
        ("mission", "/design/mission", ".mission-page"),
        (
            "settings",
            "/design/settings/appearance",
            ".settings-section",
        ),
        ("projects", "/design/projects", ".projects-page"),
        ("icons", "/design/c/icons", ".sample-controls"),
    ] {
        for width in [1440, 390] {
            browser.viewport(width, 900)?;
            browser.go(&format!("{}{route}", server.url))?;
            browser.wait(&format!("!!document.querySelector('{ready}') && !document.querySelector('#boot-splash,[data-ar-t]')"))?;
            browser.eval("document.fonts.ready.then(()=>true)")?;
            // Snapshot every element, including SVG fills and tinted label chips.
            // This catches aliases that compile but don't resolve in the browser.
            let colors = browser.eval(r#"Array.from(document.querySelectorAll('body *')).filter(e=>!e.closest('script,style,.ascii-reveal-overlay')).map(e=>{const s=getComputedStyle(e);return {tag:e.tagName,colors:['color','background-color','border-top-color','border-right-color','border-bottom-color','border-left-color','outline-color','fill','stroke','box-shadow','text-shadow'].map(p=>s.getPropertyValue(p))}})"#)?;
            fs::write(
                output.join(format!("{phase}-{name}-{width}.json")),
                serde_json::to_vec_pretty(&colors)?,
            )?;
            crate::settings::visible_page(
                &mut browser,
                &output.join(format!("{phase}-{name}-{width}.png")),
            )?;
            if phase == "after" {
                let baseline: Value = serde_json::from_slice(&fs::read(
                    output.join(format!("before-{name}-{width}.json")),
                )?)?;
                if colors != baseline {
                    return Err(format!("rendered color regression on {route} at {width}px; compare {}/{{before,after}}-{name}-{width}.json", output.display()).into());
                }
            }
        }
    }
    println!(
        "color-check: {phase} desktop/mobile colors and PNGs saved in {}",
        output.display()
    );
    Ok(())
}
