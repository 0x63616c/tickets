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
        window.perfReads=[];window.perfErrors=[];
        addEventListener('error',e=>window.perfErrors.push(e.message));
        addEventListener('unhandledrejection',e=>window.perfErrors.push(String(e.reason)));
        const fetch=window.fetch;window.fetch=(...args)=>{
            const request=args[0];const url=new URL(request instanceof Request?request.url:String(request),location.href);
            if((request.method||args[1]?.method||'GET')==='GET')window.perfReads.push(url.pathname);
            return fetch(...args);
        };
    "#}))?;
    b.go("http://127.0.0.1:8795/login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("!!document.querySelector('.home-metric strong') && !!document.querySelector('.connection.connected')")?;
    b.eval("if(perfReads.filter(p=>p==='/api/v1/projects').length!==1)throw Error('Home duplicated projects: '+JSON.stringify(perfReads));true")?;
    let output = root.join("docs/screenshots/TIK-71");
    fs::create_dir_all(&output)?;
    b.screenshot(&output.join("after-home.png"))?;
    b.click(".sidebar a[href='/board']")?;
    b.wait("document.querySelectorAll('.ticket-card').length===1")?;
    b.eval("window.collectionReads=perfReads.filter(p=>p==='/api/v1/tickets').length;if(collectionReads!==1)throw Error('initial collection fetched more than once');true")?;
    b.click(".sidebar a[href='/list']")?;
    b.wait("!!document.querySelector('.list-row')")?;
    b.click(".sidebar a[href='/board']")?;
    b.wait("document.querySelectorAll('.ticket-card').length===1")?;
    b.eval("if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('navigation reloaded collection');window.dispatchEvent(new Event('focus'));true")?;
    api(
        reqwest::Method::PATCH,
        &format!("tickets/{id}"),
        json!({"title":"Patched live fixture"}),
    )?;
    b.wait("document.querySelector('.card-title')?.textContent==='Patched live fixture'")?;
    b.eval("if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('live update reloaded collection');true")?;
    b.click(".card-title")?;
    b.wait("!!document.querySelector('.ticket-detail') || !!document.querySelector('.ticket-detail-page') || !!document.querySelector('.activity')")?;
    b.eval("if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('detail reloaded collection');true")?;
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
    b.wait("document.querySelector('.card-title')?.textContent==='Foreground catch-up fixture'")?;
    b.eval("if(perfErrors.length)throw Error(JSON.stringify(perfErrors));if(perfReads.filter(p=>p==='/api/v1/tickets').length!==collectionReads)throw Error('foregrounding reloaded collection');true")?;
    b.viewport(390, 844)?;
    b.click(".sidebar a[href='/']")?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    b.screenshot(&output.join("after-home-mobile.png"))?;
    fs::write(
        output.join("requests.json"),
        serde_json::to_string_pretty(&b.eval("({reads:perfReads,errors:perfErrors})")?)?,
    )?;
    println!(
        "PASS shared stores: Home projects once, Board/List/detail navigation one collection, targeted live patch, foreground catch-up, no browser errors"
    );
    Ok(())
}
