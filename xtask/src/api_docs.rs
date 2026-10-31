//! Real local Worker/D1 API reference checks and dark page evidence.
use crate::design::Browser;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;
use std::{fs, path::Path};

fn full_page(b: &mut Browser, output: &Path) -> super::Result {
    b.eval("scrollTo(0,0);true")?;
    let size = b.eval(
        "({width:innerWidth,height:Math.max(innerHeight,document.documentElement.scrollHeight)})",
    )?;
    let png = b.call("Page.captureScreenshot", json!({"format":"png","captureBeyondViewport":true,"clip":{"x":0,"y":0,"width":size["width"],"height":size["height"],"scale":1}}))?;
    fs::write(
        output,
        STANDARD.decode(png["data"].as_str().ok_or("missing PNG")?)?,
    )?;
    Ok(())
}
pub fn check(root: &Path, password: &str) -> super::Result {
    let output = root.join("docs/screenshots/TIK-97");
    fs::create_dir_all(&output)?;
    let base = "http://127.0.0.1:8795";
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let mut operation_count = 0;
    if !before {
        // No auth, invalid auth and no cookies all receive the same static document.
        for authorization in [None, Some("Bearer invalid")] {
            let client = reqwest::blocking::Client::new();
            let mut request = client.get(format!("{base}/api/openapi.json"));
            if let Some(value) = authorization {
                request = request.header("Authorization", value);
            }
            let response = request.send()?;
            assert_eq!(response.status().as_u16(), 200);
            assert_eq!(response.headers()["Cache-Control"], "public, max-age=3600");
            assert!(
                response.headers()["Content-Type"]
                    .to_str()?
                    .starts_with("application/json")
            );
            let spec: serde_json::Value = response.json()?;
            assert_eq!(spec["openapi"], "3.1.0");
            operation_count = spec["paths"]
                .as_object()
                .unwrap()
                .values()
                .map(|item| item.as_object().unwrap().len())
                .sum::<usize>();
        }
    }
    let prefix = if before { "before" } else { "after" };
    let mut b = Browser::start_named(root, "api-docs-check")?;
    b.viewport(1440, 900)?;
    b.go(&format!("{base}/login"))?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/'")?;
    for (name, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
        b.viewport(width, height)?;
        b.go(&format!("{base}/api-docs"))?;
        let ready = if before {
            "document.body.innerText.includes('Page not found')".to_owned()
        } else {
            format!("document.querySelectorAll('.api-docs-operation').length==={operation_count}")
        };
        b.wait(&ready)?;
        b.wait("document.fonts.ready.then(()=>true)")?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        b.screenshot(&output.join(format!("{prefix}-{name}.png")))?;
        full_page(&mut b, &output.join(format!("{prefix}-{name}-full.png")))?;
    }
    if before {
        return Ok(());
    }
    b.viewport(1440, 900)?;
    b.text(".api-docs-search input", "/api/v1/projects")?;
    b.wait("document.querySelectorAll('.api-docs-operation').length===4")?;
    b.click("#getApiV1Projects > summary")?;
    b.wait("document.querySelector('#getApiV1Projects').open")?;
    b.eval("(()=>{const e=document.querySelector('#getApiV1Projects');for(const d of e.querySelectorAll('details'))d.open=true;return true})()")?;
    b.wait("document.querySelector('#getApiV1Projects table')?.textContent.includes('cursor')")?;
    b.screenshot(&output.join("after-desktop-expanded.png"))?;
    full_page(&mut b, &output.join("after-desktop-expanded-full.png"))?;
    b.click("#getApiV1Projects button[type=submit]")?;
    b.wait("document.querySelector('#getApiV1Projects .api-docs-try pre')?.textContent.includes('HTTP 200') && document.querySelector('#getApiV1Projects .api-docs-try pre')?.textContent.includes('next_cursor')")?;
    b.wait("(()=>{const e=document.querySelector('.api-docs-try input');return !!e.closest('label.field') && parseFloat(getComputedStyle(e).minHeight)>=32})()")?;
    b.eval("document.querySelector('.api-docs-try').scrollIntoView({block:'center'});true")?;
    b.screenshot(&output.join("after-desktop-try.png"))?;
    b.viewport(390, 844)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.eval("document.querySelector('.api-docs-try').scrollIntoView({block:'center'});true")?;
    b.screenshot(&output.join("after-mobile-try.png"))?;
    b.eval("scrollTo(0,0);true")?;
    b.screenshot(&output.join("after-mobile-expanded.png"))?;
    b.text(".api-docs-search input", "no-such-endpoint")?;
    b.wait("document.querySelectorAll('.api-docs-operation').length===0 && document.querySelector('.api-docs-empty')?.textContent.includes('No endpoints match')")?;
    b.text(".api-docs-search input", "webhook")?;
    b.wait("document.querySelectorAll('.api-docs-operation').length===2")?;
    b.text(".api-docs-search input", "")?;
    b.wait(&format!(
        "document.querySelectorAll('.api-docs-operation').length==={operation_count}"
    ))?;
    println!(
        "PASS API reference: public static spec, headers, all {operation_count} operations, filter/empty/reset, disclosures, session GET, desktop/mobile overflow and dark evidence"
    );
    Ok(())
}
