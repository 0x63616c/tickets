//! TIK-44 palette fixtures and isolated local API acceptance.
use crate::{
    component::{FixtureServer, crop},
    design::Browser,
};
use serde_json::json;
use std::{fs, path::Path};

pub fn shots(root: &Path, assets: &Path, output: &Path, before: bool) -> super::Result {
    fs::create_dir_all(output)?;
    let server = FixtureServer::start(assets)?;
    let mut b = Browser::start_named(root, "palette-shots")?;
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1200,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    let phase = if before { "before" } else { "after" };
    b.go(&format!("{}/design/board", server.url))?;
    b.click(".search-trigger")?;
    b.wait("!!document.querySelector('.command-results .menu-item')")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    if !before {
        b.eval("window.__searchTop=document.querySelector('.command-search').getBoundingClientRect().top;true")?;
    }
    crop(
        &mut b,
        "dialog",
        &output.join(format!("{phase}-palette.png")),
    )?;
    b.text(".command-search input", "TKT-7")?;
    if !before {
        b.wait("document.querySelector('.command-option[aria-selected=true]')?.textContent.includes('TKT-7')")?;
        b.wait("Math.abs(document.querySelector('.command-search').getBoundingClientRect().top-window.__searchTop)<1")?;
    }
    crop(
        &mut b,
        "dialog",
        &output.join(format!("{phase}-search.png")),
    )?;
    if !before {
        b.wait("document.activeElement.matches('.command-search input')")?;
        b.key("Enter", 13, 4)?;
        b.wait("!!document.querySelector('.command-context')")?;
        crop(&mut b, "dialog", &output.join("after-actions.png"))?;
        b.text(".command-search input", "status")?;
        b.key("Enter", 13, 0)?;
        b.wait("document.querySelectorAll('.command-option').length===6")?;
        b.wait("document.querySelector('.command-search input').getAttribute('aria-label')==='Choose a status' && document.querySelector('.command-footer').textContent.includes('Set') && !!document.getElementById(document.querySelector('.command-search input').getAttribute('aria-describedby'))")?;
        crop(&mut b, "dialog", &output.join("after-status.png"))?;
        b.key("Escape", 27, 0)?;
        b.key("Escape", 27, 0)?;
        b.wait("document.querySelector('.command-search input').value==='TKT-7'")?;
        b.text(".command-search input", "status:todo p:TKT")?;
        b.wait("document.querySelectorAll('.command-option').length>0 && [...document.querySelectorAll('.command-option')].every(e=>!!e.querySelector('.status-todo'))")?;
        b.wait("document.querySelector('.command-help').lastElementChild.textContent==='TKT'")?;
        crop(&mut b, "dialog", &output.join("after-filters.png"))?;
        b.text(".command-search input", "xyz-no-match")?;
        b.wait("!!document.querySelector('.command-empty')")?;
        b.wait("Math.abs(document.querySelector('.command-search').getBoundingClientRect().top-window.__searchTop)<1")?;
        crop(&mut b, "dialog", &output.join("after-empty.png"))?;
    }
    b.text(".command-search input", "")?;
    b.viewport(390, 844)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    crop(
        &mut b,
        "dialog",
        &output.join(format!("{phase}-mobile.png")),
    )?;
    if !before {
        b.key("Tab", 9, 0)?;
        b.wait("document.activeElement.matches('.command-row-actions')")?;
        b.click(".command-row-actions[aria-label='Actions for TKT-10']")?;
        b.wait("document.querySelector('.command-context').textContent.includes('TKT-10')")?;
        b.key("Escape", 27, 0)?;
        b.viewport(390, 400)?;
        b.wait("document.querySelector('.command-footer').getBoundingClientRect().bottom<=document.querySelector('dialog').getBoundingClientRect().bottom && document.querySelector('dialog').getBoundingClientRect().bottom<=innerHeight && document.documentElement.scrollWidth<=innerWidth")?;
    }
    b.viewport(1440, 900)?;
    b.screenshot(&output.join(format!("{phase}-full-page.png")))?;
    if !before {
        b.key("Escape", 27, 0)?;
        b.wait("!document.querySelector('dialog') && document.activeElement.matches('.search-trigger')")?;
        b.go(&format!("{}/design/c/palette", server.url))?;
        b.wait("!!document.querySelector('.command-option')")?;
        b.call(
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
        )?;
        b.wait("getComputedStyle(document.querySelector('dialog')).transform==='none'")?;
        b.wait("getComputedStyle(document.querySelector('.command-search input'),'::placeholder').color===getComputedStyle(document.querySelector('.command-help')).color")?;
    }
    println!(
        "PASS palette fixtures: key search, grouped filters, action levels, Escape/focus, mobile, reduced motion"
    );
    Ok(())
}
fn command(b: &mut Browser, text: &str) -> super::Result {
    b.text(".command-search input", text)?;
    b.wait("!!document.querySelector('.command-option[aria-selected=true]') && document.querySelector('.command-results').getAttribute('aria-busy')==='false'")?;
    b.key("Enter", 13, 0)?;
    Ok(())
}
fn open(b: &mut Browser) -> super::Result {
    b.wait("!document.querySelector('dialog')")?;
    b.key("k", 75, 4)?;
    b.wait("document.activeElement.matches('.command-search input')")?;
    Ok(())
}
pub fn check(root: &Path, password: &str) -> super::Result {
    let base = "http://127.0.0.1:8795";
    let mut b = Browser::start_named(root, "palette-check")?;
    b.go(&format!("{base}/login"))?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/'")?;
    b.eval(r#"(async()=>{const post=async(p,v)=>{const r=await fetch('/api/v1/'+p,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(v)});if(!r.ok)throw Error('Seed failed');return r.json()};await post('projects',{name:'Tickets',key_prefix:'TIK',color:'#5e6ad2'});await post('projects',{name:'Quantum',key_prefix:'QUA',color:'#4ea7fc'});await post('tickets',{project_id:1,title:'Command palette',body:'Search the workspace',status:'in_progress',assignee:'agent'});await post('tickets',{project_id:2,title:'Render voxel mesh',body:'Incremental mesh cache',status:'todo',external_url:'https://github.com/example/quantum/issues/78'});await post('tickets',{project_id:1,title:'Reference QUA-1',status:'todo'});return true})()"#)?;
    b.go(base)?;
    b.wait("document.querySelectorAll('.api-ticket').length===2")?;
    open(&mut b)?;
    b.wait(
        "document.querySelector('.command-results').textContent.includes('In progress & assigned')",
    )?;
    b.text(".command-search input", "qua-1")?;
    b.wait("document.querySelector('.command-option[aria-selected=true]')?.textContent.includes('Render voxel mesh')")?;
    b.key("Enter", 13, 0)?;
    b.wait("location.pathname==='/t/QUA-1' && !!document.querySelector('.ticket-detail')")?;
    b.wait("JSON.parse(localStorage.getItem('tickets.recently-viewed'))[0]===2")?;
    open(&mut b)?;
    b.wait("document.querySelector('.command-results').textContent.includes('Recently viewed')")?;
    b.text(".command-search input", "status:todo p:QUA msh cche")?;
    b.wait("document.querySelectorAll('.command-option').length===1 && document.querySelector('.command-option').textContent.includes('QUA-1')")?;
    b.key("Enter", 13, 4)?;
    b.wait("!!document.querySelector('.command-context')")?;
    b.eval("window.__opened='';window.open=(url)=>{window.__opened=url;return null};true")?;
    command(&mut b, "GitHub")?;
    b.wait("!document.querySelector('dialog') && window.__opened==='https://github.com/example/quantum/issues/78'")?;
    // Context actions on a directly opened detail; clipboard and submenus.
    open(&mut b)?;
    command(&mut b, "Actions for QUA-1")?;
    b.eval("window.__copied='';Object.defineProperty(navigator.clipboard,'writeText',{configurable:true,value:async(text)=>{window.__copied=text}});true")?;
    command(&mut b, "Copy key")?;
    b.wait("window.__copied==='QUA-1' && !document.querySelector('dialog')")?;
    open(&mut b)?;
    command(&mut b, "Actions for QUA-1")?;
    command(&mut b, "Copy link")?;
    b.wait("window.__copied.endsWith('/t/QUA-1') && !document.querySelector('dialog')")?;
    open(&mut b)?;
    command(&mut b, "Actions for QUA-1")?;
    command(&mut b, "Change status")?;
    command(&mut b, "In progress")?;
    b.wait("!document.querySelector('dialog')")?;
    b.wait("fetch('/api/v1/tickets/2').then(r=>r.json()).then(t=>t.status==='in_progress')")?;
    open(&mut b)?;
    command(&mut b, "Actions for QUA-1")?;
    command(&mut b, "Change priority")?;
    b.eval("window.__fetch=window.fetch;window.fetch=(url,opts)=>(url instanceof Request?url.url:String(url)).includes('/tickets/2')&&(url instanceof Request?url.method:opts?.method)==='PATCH'?Promise.resolve(new Response(JSON.stringify({error:{message:'Fixture rejected save'}}),{status:500})):window.__fetch(url,opts);true")?;
    command(&mut b, "Urgent")?;
    b.wait("document.querySelector('.command-error')?.textContent.includes('Could not save') && !!document.querySelector('dialog')")?;
    b.eval("window.fetch=window.__fetch;true")?;
    b.key("Enter", 13, 0)?;
    b.wait("!document.querySelector('dialog')")?;
    b.wait("fetch('/api/v1/tickets/2').then(r=>r.json()).then(t=>t.priority===1)")?;
    open(&mut b)?;
    command(&mut b, "Actions for QUA-1")?;
    command(&mut b, "Move to project")?;
    command(&mut b, "Tickets")?;
    b.wait("location.pathname==='/t/TIK-3' && !document.querySelector('dialog')")?;
    b.wait("fetch('/api/v1/tickets/2').then(r=>r.json()).then(t=>t.project_id===1 && t.external_url==='https://github.com/example/quantum/issues/78')")?;
    open(&mut b)?;
    command(&mut b, "Go to list")?;
    b.wait("location.pathname==='/list'")?;
    open(&mut b)?;
    command(&mut b, "Settings")?;
    b.wait("location.pathname==='/settings/account'")?;
    open(&mut b)?;
    command(&mut b, "Toggle theme")?;
    b.wait("document.documentElement.dataset.theme==='light'")?;
    open(&mut b)?;
    command(&mut b, "Toggle theme")?;
    b.wait("document.documentElement.dataset.theme==='dark'")?;
    open(&mut b)?;
    command(&mut b, "Quantum")?;
    b.wait("location.pathname==='/' && document.querySelector('.breadcrumb-current')?.textContent.includes('Quantum')")?;
    open(&mut b)?;
    command(&mut b, "New ticket")?;
    b.wait("!!document.querySelector('#board-title') && document.querySelectorAll('dialog').length===1")?;
    b.wait("document.querySelector('.composer-project')?.textContent.includes('Quantum') || document.querySelector('dialog').textContent.includes('Quantum')")?;
    b.text("#board-title", "Created through palette")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && document.body.textContent.includes('Created through palette')")?;
    b.wait("fetch('/api/v1/tickets?q=Created%20through%20palette').then(r=>r.json()).then(p=>p.data.length===1 && p.data[0].project_id===2)")?;
    // A delayed old query must never replace the new results or execute stale data.
    open(&mut b)?;
    b.eval("window.__fetch=window.fetch;window.fetch=async(url,opts)=>{if((url instanceof Request?url.url:String(url)).includes('/search?q=slow'))await new Promise(r=>setTimeout(r,600));return window.__fetch(url,opts)};true")?;
    b.text(".command-search input", "slow")?;
    b.eval("new Promise(r=>setTimeout(()=>r(true),180))")?;
    b.text(".command-search input", "TIK-3")?;
    b.wait("document.querySelector('.command-option[aria-selected=true]')?.textContent.includes('TIK-3')")?;
    b.eval("new Promise(r=>setTimeout(()=>r(true),700))")?;
    b.wait("document.querySelector('.command-option[aria-selected=true]')?.textContent.includes('TIK-3')")?;
    b.key("Escape", 27, 0)?;
    b.wait("!document.querySelector('dialog')")?;
    println!(
        "PASS palette API: cross-project exact/fuzzy/body search, filters, recent views, commands, mutations, rollback/retry, GitHub provenance, clipboard, theme, composer handoff, stale requests"
    );
    Ok(())
}
