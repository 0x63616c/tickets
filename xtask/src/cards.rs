//! Same sample data and viewports for compact-card before/after evidence.
use crate::{
    component::{FixtureServer, crop},
    design::Browser,
};
use serde_json::json;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

pub fn shots(root: &Path) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS");
    let assets = before
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let phase = if before.is_some() { "before" } else { "after" };
    let output = root.join("docs/screenshots/TIK-57");
    fs::create_dir_all(&output)?;
    let labels = [
        ("type:feat", "#8b82d9"),
        ("area:render", "#4ea7fc"),
        ("model:astra", "#8a8f98"),
        ("priority:p1", "#f0bf00"),
        ("area:performance", "#c6a1b9"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (name, color))| json!({"id":i+1,"name":name,"color":color}))
    .collect::<Vec<_>>();
    let tickets = [
        (14,"Cache compiled shader variants per material so scene reloads stop recompiling","todo",2,2,vec![]),
        (31,"Fix flicker when resizing the viewport","in_progress",1,0,vec![]),
        (52,"Docs: profiling guide","in_review",4,5,vec![]),
        (54,"Coordinate renderer work across two dependencies","todo",3,0,vec![31,52]),
        (53,"Unblock the renderer without losing the saved shader cache","todo",3,1,vec![31]),
    ].into_iter().map(|(id,title,status,priority,comments,blockers)|json!({"id":id,"project_id":1,"key":format!("TKT-{id}"),"title":title,"status":status,"priority":priority,"assignee":null,"sort_order":id*1024,"labels":if id==14 {labels.clone()}else{labels[..1].to_vec()},"blocked_by":blockers,"blocks":[],"comment_count":comments,"created_at":"2026-10-01T00:00:00.000Z","updated_at":"2026-10-02T00:00:00.000Z","body":"Sample data"})).collect::<Vec<_>>();
    let fixtures = HashMap::from([
        (
            "/api/v1/tickets".into(),
            json!({"data":tickets,"next_cursor":null}),
        ),
        (
            "/api/v1/labels".into(),
            json!({"data":labels,"next_cursor":null}),
        ),
    ]);
    let server = FixtureServer::start_with_api(&assets, fixtures)?;
    let mut b = Browser::start_named(root, "cards")?;
    b.viewport(1440, 900)?;
    b.go(&format!("{}/board?project=1", server.url))?;
    b.wait("document.querySelectorAll('.api-ticket').length===5")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    b.wait("!document.querySelector('[data-ar-t]')")?;
    if before.is_none() {
        b.wait("document.querySelectorAll('#ticket-14 .card-label-dot').length===4 && document.querySelector('#ticket-14 .card-label-names').textContent==='feat, render, astra' && !document.querySelector('#ticket-14 .label-chip')")?;
        b.wait("getComputedStyle(document.querySelector('#ticket-14 .card-title')).webkitLineClamp==='2' && document.querySelector('#ticket-14 .card-title').title.includes('stop recompiling')")?;
        b.wait("document.querySelector('#ticket-14 .card-label-summary').title.includes('area:performance') && !document.querySelector('#ticket-14 .card-label-summary').title.includes('priority:')")?;
    }
    b.screenshot(&output.join(format!("{phase}-board-desktop.png")))?;
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1440,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    for (name, id) in [
        ("long-title", 14),
        ("many-labels", 14),
        ("urgent", 31),
        ("blocked", 53),
        ("blocked-multiple", 54),
    ] {
        crop(
            &mut b,
            &format!("#ticket-{id}"),
            &output.join(format!("{phase}-{name}.png")),
        )?;
    }
    if before.is_none() {
        let point=b.eval("(()=>{const r=document.querySelector('#ticket-54 .blocked-indicator').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()")?;
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":point["x"],"y":point["y"]}),
        )?;
        b.wait("[...document.querySelectorAll('[role=tooltip]')].some(e=>!e.hidden && e.textContent.includes('TKT-31: Fix flicker') && e.textContent.includes('TKT-52: Docs: profiling guide'))")?;
        b.screenshot(&output.join("after-blocked-tooltip.png"))?;
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":0,"y":0}),
        )?;
        b.wait("[...document.querySelectorAll('[role=tooltip]')].every(e=>e.hidden)")?;
    }
    b.viewport(390, 900)?;
    b.screenshot(&output.join(format!("{phase}-board-mobile.png")))?;
    b.go(&format!("{}/list?project=1", server.url))?;
    b.wait("document.querySelectorAll('.api-list-row').length===5")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    b.wait("!document.querySelector('[data-ar-t]')")?;
    b.screenshot(&output.join(format!("{phase}-list-mobile.png")))?;
    b.viewport(1440, 900)?;
    b.screenshot(&output.join(format!("{phase}-list-desktop.png")))?;
    Ok(())
}
