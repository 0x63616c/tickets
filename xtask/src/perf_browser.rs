//! Browser acceptance for shared stores and targeted live invalidations.
use crate::design::Browser;
use serde_json::{Value, json};
use std::{fs, path::Path};
pub fn check(root: &Path, password: &str, token: &str) -> super::Result {
    let client = reqwest::blocking::Client::new();
    let api = |method, path: &str, body: Value| -> Result<Value, Box<dyn std::error::Error>> {
        Ok(client
            .request(method, format!("http://127.0.0.1:8795/api/v1/{path}"))
            .bearer_auth(token)
            .json(&body)
            .send()?
            .error_for_status()?
            .json()?)
    };
    let p = api(
        reqwest::Method::POST,
        "projects",
        json!({"name":"Tickets","key_prefix":"TKT","color":"#5e6ad2"}),
    )?;
    let ticket = api(
        reqwest::Method::POST,
        "tickets",
        json!({"project_id":p["id"],"title":"Shared stores fixture","status":"todo"}),
    )?;
    let id = ticket["id"].as_i64().ok_or("ticket id")?;
    let mut b = Browser::start_named(root, "perf-browser")?;
    b.viewport(1280, 800)?;
    b.call("Page.addScriptToEvaluateOnNewDocument",json!({"source":r#"
        window.perfReads=[];window.perfErrors=[];window.perfSockets=[];
        const Socket=WebSocket;window.WebSocket=class extends Socket{constructor(...args){super(...args);window.perfSockets.push(this)}};
        addEventListener('error',e=>window.perfErrors.push(e.message));
        addEventListener('unhandledrejection',e=>window.perfErrors.push(String(e.reason)));
        const perfNativeFetch=window.fetch;window.fetch=(...args)=>{
            const request=args[0];const url=new URL(request instanceof Request?request.url:String(request),location.href);
            if((request.method||args[1]?.method||'GET')==='GET')window.perfReads.push(url.pathname);
            return perfNativeFetch(...args).then(response=>{if(url.pathname==='/api/v1/version')window.versionChecked=true;return response});
        };
    "#}))?;
    b.go("http://127.0.0.1:8795/login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    if let Err(error) =
        b.wait("!!document.querySelector('.home-metric strong') && window.versionChecked===true")
    {
        eprintln!("Perf startup: {}",b.eval("({reads:window.perfReads,errors:window.perfErrors,sockets:window.perfSockets?.map(s=>({url:s.url,state:s.readyState})),checked:window.versionChecked})")?);
        return Err(error);
    }
    b.eval("if(perfReads.filter(p=>p==='/api/v1/projects').length!==1)throw Error('Home duplicated projects: '+JSON.stringify(perfReads));true")?;
    let output = root.join("docs/screenshots/TIK-71");
    fs::create_dir_all(&output)?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    b.wait("Math.round(document.querySelector('.sidebar').getBoundingClientRect().width)===240 && getComputedStyle(document.documentElement).backgroundColor==='rgb(0, 0, 0)'")?;
    b.wait("!document.querySelector('[data-ar-t], .ar-overlay canvas')")?;
    b.screenshot(&output.join("after-home.png"))?;
    b.click(".sidebar a[href='/board']")?;
    b.wait("document.querySelectorAll('.ticket-card').length===1")?;
    b.eval("window.collectionReads=perfReads.filter(p=>p==='/api/v1/tickets').length;if(collectionReads!==1)throw Error('initial collection fetched more than once');true")?;
    b.click(".sidebar a[href='/list']")?;
    b.wait("!!document.querySelector('.api-list-row')")?;
    b.click(".sidebar a[href='/board']")?;
    b.wait("document.querySelectorAll('.ticket-card').length===1")?;
    b.eval("if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('navigation reloaded collection');window.dispatchEvent(new Event('focus'));true")?;
    b.eval("window.beforeLiveReads=perfReads.length;true")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{id}"),
        json!({"title":"First burst fixture"}),
    )?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{id}"),
        json!({"title":"Patched live fixture"}),
    )?;
    b.eval("new Promise(resolve=>setTimeout(resolve,2000))")?;
    b.eval("if(perfReads.length!==beforeLiveReads)throw Error('live notice refreshed before debounce');true")?;
    b.wait("document.querySelector('.card-title')?.textContent==='Patched live fixture'")?;
    b.eval("if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('live update reloaded collection');true")?;
    b.click(".card-title")?;
    b.wait("!!document.querySelector('.ticket-detail') || !!document.querySelector('.ticket-detail-page') || !!document.querySelector('.activity')")?;
    b.eval("new Promise(resolve=>setTimeout(resolve,500))")?;
    b.eval("if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('detail reloaded collection');if(perfReads.filter(p=>p==='/api/v1/tickets/1/comments').length!==1)throw Error('detail replayed an old live revision');true")?;
    b.click(".sidebar a[href='/board']")?;
    b.wait("document.querySelectorAll('.ticket-card').length===1")?;
    let _hidden = b.call("Target.createTarget", json!({"url":"about:blank"}))?;
    b.wait("document.hidden")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{id}"),
        json!({"title":"Foreground catch-up fixture"}),
    )?;
    b.call("Page.bringToFront", json!({}))?;
    b.wait_for(
        "document.querySelector('.card-title')?.textContent==='Foreground catch-up fixture'",
        std::time::Duration::from_secs(40),
    )?;
    b.eval("if(perfErrors.length)throw Error(JSON.stringify(perfErrors));if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('foregrounding reloaded collection');true")?;
    b.click(".sidebar a[href='/']")?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    b.viewport(390, 844)?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    b.wait("!document.querySelector('[data-ar-t], .ar-overlay canvas')")?;
    b.screenshot(&output.join("after-home-mobile.png"))?;
    // A cron/bulk invalidation has no record IDs. Home must keep aggregate-only
    // reads and mark an already-loaded collection stale for the next Board visit.
    b.eval("window.bulkHomeReads=perfReads.filter(p=>p==='/api/v1/analytics/home').length;for(const socket of perfSockets.filter(s=>s.readyState===1))socket.dispatchEvent(new MessageEvent('message',{data:JSON.stringify({project_id:1,entity:'tickets',id:0,action:'update'})}));true")?;
    b.wait_for("perfReads.filter(p=>p==='/api/v1/analytics/home').length>bulkHomeReads && !document.querySelector('.home-page[aria-busy=true]')", std::time::Duration::from_secs(40))?;
    b.eval("if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('bulk Home event downloaded collection');true")?;
    b.viewport(1280, 800)?;
    b.click(".sidebar a[href='/board']")?;
    b.wait("document.querySelectorAll('.ticket-card').length===1 && perfReads.filter(p=>p==='/api/v1/tickets').length===collectionReads+1")?;
    let session = b.eval("({reads:perfReads,errors:perfErrors})")?;
    // Race a write between the first ticket page and the later labels response,
    // before this new document opens any live socket. The initial version check
    // must recover it even though metadata has already seen the newer version.
    b.call("Page.addScriptToEvaluateOnNewDocument",json!({"source":r#"
        const raceNativeFetch=window.fetch;
        window.fetch=async(...args)=>{
            const response=await raceNativeFetch(...args);
            const url=new URL(args[0] instanceof Request?args[0].url:String(args[0]),location.href);
            if(url.pathname==='/api/v1/tickets' && !window.snapshotRaceInjected){
                window.snapshotRaceInjected=true;
                const write=await raceNativeFetch(new Request('/api/v1/tickets/1',{method:'PATCH',headers:{'Content-Type':'application/json'},body:JSON.stringify({title:'Initial snapshot race recovered'})}));
                if(!write.ok)throw Error('snapshot fixture write failed');
            }
            return response;
        };
    "#}))?;
    b.go("http://127.0.0.1:8795/board")?;
    b.wait(
        "document.querySelector('.card-title')?.textContent==='Initial snapshot race recovered'",
    )?;
    let race = b.eval("({reads:perfReads,errors:perfErrors})")?;
    fs::write(
        output.join("requests.json"),
        serde_json::to_string_pretty(&json!({"session":session,"initial_snapshot_race":race}))?,
    )?;
    println!(
        "PASS shared stores: Home projects once, Board/List/detail navigation one collection, targeted live patch, foreground catch-up, aggregate-only bulk Home recovery, initial snapshot race recovery, no browser errors"
    );
    Ok(())
}
