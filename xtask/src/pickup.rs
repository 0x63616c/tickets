//! Fixture screenshots and the real local Settings rule lifecycle.
use crate::{
    component::{FixtureServer, visible_crop},
    design::Browser,
};
use std::{fs, path::Path};

pub fn shots(root: &Path, assets: &Path, output: &Path, before: bool) -> super::Result {
    fs::create_dir_all(output)?;
    let server = FixtureServer::start(assets)?;
    let prefix = if before { "before" } else { "after" };
    let route = if before {
        "/design/settings/appearance"
    } else {
        "/design/settings/pickup-rules"
    };
    let mut b = Browser::start_named(root, "pickup-shots")?;
    for (name, width, height) in [("desktop", 1440, 1000), ("mobile", 390, 844)] {
        b.viewport(width, height)?;
        b.go(&format!("{}{route}", server.url))?;
        b.wait("!!document.querySelector('.settings-section')")?;
        if !before {
            b.wait("document.querySelector('#pickup-cap')?.value==='3' && !document.querySelector('fieldset')?.disabled")?;
        }
        b.eval("document.fonts.ready.then(()=>true)")?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        capture_full(&mut b, &output.join(format!("{prefix}-rules-{name}.png")))?;
    }
    b.call(
        "Emulation.setDeviceMetricsOverride",
        serde_json::json!({"width":1200,"height":1100,"deviceScaleFactor":2,"mobile":false}),
    )?;
    b.go(&format!(
        "{}{}",
        server.url,
        if before {
            route
        } else {
            "/design/c/pickup-rules"
        }
    ))?;
    b.wait("!!document.querySelector('.settings-section')")?;
    if !before {
        b.wait("document.querySelector('#pickup-cap')?.value==='3'")?;
    }
    visible_crop(
        &mut b,
        ".settings-section",
        &output.join(format!("{prefix}-rules-component.png")),
    )?;
    if !before {
        b.choose("[aria-label='Rule scope']", "1")?;
        b.wait("document.querySelector('.pickup-inheritance').textContent.includes('Using workspace') && !document.querySelector('fieldset').disabled")?;
        b.text("#pickup-cap", "2")?;
        b.click(".settings-save button[type=submit]")?;
        b.wait("document.querySelector('.pickup-result').textContent.includes('Rules saved.')")?;
        b.eval("new Promise(r=>setTimeout(()=>r(true),650))")?;
        visible_crop(
            &mut b,
            ".settings-section",
            &output.join("after-rules-project.png"),
        )?;
    }
    println!("PASS pickup screenshots: fixtures, loaded fonts, desktop/mobile, 2× component");
    Ok(())
}
fn capture_full(b: &mut Browser, path: &Path) -> super::Result {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let size = b.eval(
        "({width:innerWidth,height:Math.max(innerHeight,document.documentElement.scrollHeight)})",
    )?;
    let png=b.call("Page.captureScreenshot",serde_json::json!({"format":"png","captureBeyondViewport":true,"clip":{"x":0,"y":0,"width":size["width"],"height":size["height"],"scale":1}}))?;
    fs::write(
        path,
        STANDARD.decode(png["data"].as_str().ok_or("missing PNG")?)?,
    )?;
    Ok(())
}
pub fn check(b: &mut Browser) -> super::Result {
    b.go("http://127.0.0.1:8795/settings/pickup-rules")?;
    b.wait("document.querySelector('#pickup-cap')?.value==='3' && !document.querySelector('fieldset').disabled")?;
    b.text("#pickup-cap", "4")?;
    b.wait("document.querySelector('#pickup-scope-hint').textContent.includes('Save or discard')")?;
    b.text("#pickup-labels", "waiting-on-calum, waiting-on-calum")?;
    b.click(".settings-save button[type=submit]")?;
    b.wait("document.querySelector('#pickup-labels').getAttribute('aria-invalid')==='true' && document.querySelector('#pickup-labels-error').textContent.includes('duplicate label')")?;
    b.text("#pickup-labels", "waiting-on-calum")?;

    b.wait("document.querySelector('[aria-label=\"Rule scope\"]').disabled")?;
    b.eval("window.pickupFetch=window.fetch;window.fetch=(url,options)=>(url.url || String(url)).includes('/pickup-rules') && (options?.method || url.method)==='PUT' ? Promise.resolve(new Response(JSON.stringify({error:{message:'Fixture save failed'}}),{status:503,headers:{'Content-Type':'application/json'}})) : window.pickupFetch(url,options);true")?;
    b.click(".settings-save button[type=submit]")?;
    b.wait("document.querySelector('.pickup-editor form > .settings-error').textContent.includes('Fixture save failed') && document.querySelector('#pickup-cap').value==='4' && !document.querySelector('fieldset').disabled")?;
    b.eval("window.fetch=window.pickupFetch;true")?;
    b.click(".settings-save button[type=submit]")?;
    b.wait("document.querySelector('.pickup-result').textContent.includes('Rules saved.')")?;
    b.go("http://127.0.0.1:8795/settings/pickup-rules")?;
    b.wait("document.querySelector('#pickup-cap')?.value==='4' && !document.querySelector('fieldset').disabled")?;
    // Add a disposable project through the authenticated UI's same-origin API.
    b.eval("fetch('/api/v1/projects',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name:'Pickup fixture',key_prefix:'PICK',color:'#5e6ad2'})}).then(r=>r.json()).then(p=>{window.pickupProject=p.id;return true})")?;
    b.go("http://127.0.0.1:8795/settings/pickup-rules")?;
    b.wait("document.querySelector('#pickup-cap')?.value==='4' && !document.querySelector('fieldset').disabled")?;
    b.choose("[aria-label='Rule scope']", "1")?;
    b.wait("document.querySelector('.pickup-inheritance').textContent.includes('Using workspace') && document.querySelector('#pickup-cap').value==='4' && !document.querySelector('fieldset').disabled")?;
    b.text("#pickup-cap", "2")?;
    b.click(".settings-save button[type=submit]")?;
    b.wait("document.querySelector('.pickup-inheritance').textContent.includes('own rules') && document.querySelector('.pickup-result').textContent.includes('Rules saved.')")?;
    b.click(".settings-save .button-secondary")?;
    b.wait("document.querySelector('#pickup-cap').value==='4' && document.querySelector('.pickup-result').textContent.includes('Using workspace rules.')")?;
    b.viewport(390, 844)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.viewport(1440, 900)?;
    // Preserve the surrounding empty-workspace Settings journey.
    b.eval("fetch('/api/v1/projects/1',{method:'PATCH',headers:{'Content-Type':'application/json'},body:JSON.stringify({archived:true})}).then(r=>r.ok)")?;
    println!(
        "PASS pickup Settings: persistence, inheritance, override removal, failed-save retention, scope lock, mobile overflow"
    );
    Ok(())
}
