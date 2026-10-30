//! Mission Control fixture evidence and real approval UI checks.
use crate::{
    component::{FixtureServer, visible_crop},
    design::Browser,
};
use serde_json::json;
use std::{fs, path::Path};

pub fn shots(root: &Path, assets: &Path, output: &Path, before: bool) -> super::Result {
    use base64::{Engine, engine::general_purpose::STANDARD};
    fs::create_dir_all(output)?;
    let server = FixtureServer::start(assets)?;
    let mut b = Browser::start_named(root, "mission-shots")?;
    let prefix = if before { "before" } else { "after" };
    for (name, width, height) in [("desktop", 1440, 1000), ("mobile", 390, 844)] {
        b.viewport(width, height)?;
        b.go(&format!(
            "{}{}",
            server.url,
            if before {
                "/design/home"
            } else {
                "/design/mission"
            }
        ))?;
        b.wait(if before {
            "!!document.querySelector('.home-page h2')"
        } else {
            "document.querySelectorAll('.mission-section').length===7"
        })?;
        if before {
            b.wait("!Array.from(document.querySelectorAll('.sidebar a')).some(a=>a.textContent.includes('Mission Control'))")?;
        }
        b.eval("document.fonts.ready.then(()=>true)")?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        b.eval("new Promise(r=>setTimeout(()=>r(true),400))")?;
        let size=b.eval("({width:innerWidth,height:Math.max(innerHeight,document.documentElement.scrollHeight)})")?;
        let png=b.call("Page.captureScreenshot",json!({"format":"png","captureBeyondViewport":true,"clip":{"x":0,"y":0,"width":size["width"],"height":size["height"],"scale":1}}))?;
        fs::write(
            output.join(format!("{prefix}-mission-{name}.png")),
            STANDARD.decode(png["data"].as_str().ok_or("missing PNG")?)?,
        )?;
        if width == 390 {
            b.click("[aria-label='Toggle sidebar']")?;
            b.eval("document.activeElement?.blur();new Promise(r=>setTimeout(()=>r(true),200))")?;
            b.screenshot(&output.join(format!("{prefix}-mission-mobile-sidebar.png")))?;
        }
    }
    if !before {
        b.call(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":1200,"height":1400,"deviceScaleFactor":2,"mobile":false}),
        )?;
        b.go(&format!("{}/design/c/mission", server.url))?;
        b.wait("document.querySelectorAll('.mission-section').length===7")?;
        visible_crop(
            &mut b,
            ".component-stage",
            &output.join("after-mission-component.png"),
        )?;
        b.click("[aria-label='Approve TIK-79']")?;
        b.wait("!document.querySelector('[aria-label=\"Approve TIK-79\"]')")?;
    }
    println!(
        "PASS Mission Control {} screenshots: desktop/mobile, loaded fonts, no overflow",
        if before { "baseline" } else { "fixture" }
    );
    Ok(())
}

pub fn check(root: &Path, password: &str) -> super::Result {
    let mut b = Browser::start_named(root, "mission-check")?;
    b.viewport(1440, 1000)?;
    b.go("http://127.0.0.1:8795/login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/'")?;
    b.eval(r#"(async()=>{
      const write=async(path,body,method='POST')=>{const r=await fetch('/api/v1/'+path,{method,headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw new Error('Fixture write failed');return r.json()};
      const p=await write('projects',{name:'Mission fixtures',key_prefix:'MC',color:'#5e6ad2'});
      const t=await write('tickets',{project_id:p.id,title:'Approve Mission release',status:'todo'});
      const labels=await (await fetch('/api/v1/labels')).json();
      window.missionLabel=labels.data.find(l=>l.name==='waiting-on-calum').id;
      await write('tickets/'+t.key+'/labels/'+window.missionLabel,{});
      await write('mission/reports/box',{jobs:[{ticket:t.key,state:'running',detail:'Checking approval'}]},'PUT');
      return true;
    })()"#)?;
    b.go("http://127.0.0.1:8795/mission")?;
    b.wait("document.querySelectorAll('.mission-section').length===7 && !!document.querySelector('[aria-label=\"Approve MC-1\"]')")?;
    // Fail only the comment write: label compensation must preserve the approval queue.
    b.eval(r#"window.missionFetch=window.fetch;window.fetch=(url,opts)=>{const path=url.url||String(url);return path.endsWith('/comments') && (opts?.method||url.method)==='POST'?Promise.resolve(new Response(JSON.stringify({error:{message:'Comment fixture failure'}}),{status:503,headers:{'Content-Type':'application/json'}})):window.missionFetch(url,opts)};true"#)?;
    b.click("[aria-label='Approve MC-1']")?;
    b.wait("document.querySelector('[role=alert]')?.textContent.includes('approval was undone') && !!document.querySelector('[aria-label=\"Approve MC-1\"]')")?;
    b.eval(r#"fetch('/api/v1/tickets/MC-1').then(r=>r.json()).then(t=>{if(!t.labels.some(l=>l.name==='waiting-on-calum'))throw new Error('Label compensation missing');return true})"#)?;
    b.eval("window.fetch=window.missionFetch;true")?;
    b.click("[aria-label='Approve MC-1']")?;
    b.wait("!document.querySelector('[aria-label=\"Approve MC-1\"]') && document.querySelector('.mission-section').textContent.includes('Nothing needs your approval')")?;
    b.eval(r#"(async()=>{const t=await(await fetch('/api/v1/tickets/MC-1')).json();const c=await(await fetch('/api/v1/tickets/MC-1/comments')).json();if(t.labels.some(l=>l.name==='waiting-on-calum')||c.data.filter(c=>c.body==='Approved from Mission Control.').length!==1)throw new Error('Approval did not persist');return true})()"#)?;
    // Focus refresh catches a report without waiting for the poll interval.
    b.eval(r#"fetch('/api/v1/mission/reports/box',{method:'PUT',headers:{'Content-Type':'application/json'},body:JSON.stringify({needs_calum:[{title:'Focus refresh arrived',ticket_key:'MC-1'}]})}).then(()=>{dispatchEvent(new Event('focus'));return true})"#)?;
    b.wait(
        "document.querySelector('.mission-page').textContent.includes('Focus refresh arrived')",
    )?;
    b.eval(r#"window.fetch=(url,opts)=>(url.url||String(url)).endsWith('/api/v1/mission')?Promise.resolve(new Response(JSON.stringify({error:{message:'Refresh fixture failure'}}),{status:503,headers:{'Content-Type':'application/json'}})):window.missionFetch(url,opts);dispatchEvent(new Event('focus'));true"#)?;
    b.wait("document.querySelector('[role=alert]')?.textContent.includes('Refresh fixture failure') && document.querySelector('.mission-page').textContent.includes('Focus refresh arrived')")?;
    b.eval("window.fetch=window.missionFetch;true")?;
    b.viewport(390, 844)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    println!(
        "PASS Mission Control live UI: approval/comment persistence, rollback, focus refresh, retained data after refresh failure, mobile overflow"
    );
    Ok(())
}
