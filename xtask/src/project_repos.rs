//! Immutable API fixtures: repo-only links, aggregate counts and linked PR rendering.
use crate::{component::FixtureServer, design::Browser};
use serde_json::json;
use std::{collections::HashMap, env, fs, path::PathBuf};

pub fn check(root: &std::path::Path) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS");
    let phase = if before.is_some() { "before" } else { "after" };
    let assets = before
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let output = root.join("docs/screenshots/TIK-42");
    fs::create_dir_all(&output)?;
    let projects = json!([
        {"id":1,"name":"Tickets","key_prefix":"TKT","color":"#5e6ad2","icon":"ticket","archived":false,"github_owner":"example","github_repo":"tickets","github_project_number":null,"github_project_kind":"users","next_ticket_number":43,"open_ticket_count":12,"done_ticket_count":24,"last_activity_at":"2026-10-29T12:00:00.000Z"},
        {"id":2,"name":"Quantum","key_prefix":"QUA","color":"#4ea7fc","icon":"box","archived":false,"github_owner":"example","github_repo":"quantum","github_project_number":null,"github_project_kind":"users","next_ticket_number":1,"open_ticket_count":8,"done_ticket_count":16,"last_activity_at":"2026-10-28T12:00:00.000Z"},
        {"id":3,"name":"Apps","key_prefix":"APP","color":"#27a644","icon":"grid","archived":false,"github_owner":null,"github_repo":null,"github_project_number":null,"github_project_kind":"users","next_ticket_number":1,"open_ticket_count":0,"done_ticket_count":0,"last_activity_at":null}
    ]);
    let ticket = json!({"id":42,"project_id":1,"key":"TKT-42","title":"Link projects to repositories","body":"Show repository links and project activity.","status":"in_review","priority":3,"assignee":null,"sort_order":0,"labels":[],"blocks":[],"blocked_by":[],"comment_count":0,"created_at":"2026-10-28T12:00:00.000Z","updated_at":"2026-10-29T12:00:00.000Z","pull_requests":[
        {"repo":"example/tickets","number":123,"url":"https://github.com/example/tickets/pull/123","title":"Show project repositories","state":"open","closes":true},
        {"repo":"example/tickets","number":121,"url":"https://github.com/example/tickets/pull/121","title":"Add shared project icons","state":"merged","closes":false}
    ]});
    let fixtures = HashMap::from([
        (
            "/api/v1/projects".into(),
            json!({"data":projects,"next_cursor":null}),
        ),
        (
            "/api/v1/tickets".into(),
            json!({"data":[ticket.clone()],"next_cursor":null}),
        ),
        ("/api/v1/tickets/TKT-42".into(), ticket.clone()),
        ("/api/v1/tickets/42".into(), ticket),
    ]);
    let server = FixtureServer::start_with_api(&assets, fixtures)?;
    let mut b = Browser::start_named(root, "project-repos")?;
    b.go(&format!("{}/projects", server.url))?;
    b.wait("document.querySelectorAll('.api-project-row').length===3")?;
    if phase == "after" {
        b.wait("!!document.querySelector('.api-project-row a[href=\"https://github.com/example/tickets\"]') && !!document.querySelector('.api-project-row a[href=\"https://github.com/example/tickets/pulls\"]') && !!document.querySelector('.api-project-row a[href=\"https://github.com/example/tickets/actions\"]')")?;
        b.wait("document.querySelector('.project-counts').textContent==='12 open24 done' && document.querySelector('time').getAttribute('datetime')==='2026-10-29T12:00:00.000Z' && document.body.textContent.includes('No activity')")?;
    }
    for (size, width) in [("desktop", 1440), ("mobile", 390)] {
        b.viewport(width, 900)?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        if phase == "after" {
            b.wait(
                "getComputedStyle(document.querySelector('.project-repository')).display!=='none'",
            )?;
        }
        b.screenshot(&output.join(format!("{phase}-projects-{size}.png")))?;
        if width == 1440 {
            b.call(
                "Emulation.setDeviceMetricsOverride",
                json!({"width":1440,"height":900,"deviceScaleFactor":2,"mobile":false}),
            )?;
            crate::component::crop(
                &mut b,
                ".projects-table",
                &output.join(format!("{phase}-projects-table.png")),
            )?;
        }
    }
    if phase == "after" {
        b.viewport(800, 900)?;
        b.wait("document.documentElement.scrollWidth<=innerWidth && getComputedStyle(document.querySelector('.project-repository')).display!=='none' && getComputedStyle(document.querySelector('.project-activity'),'::before').content.includes('Last activity')")?;
        b.eval("document.querySelector('.repository-shortcuts a').focus();true")?;
        b.wait("document.activeElement.matches('.repository-shortcuts a') && getComputedStyle(document.activeElement).outlineStyle!=='none'")?;
    }
    b.viewport(1440, 900)?;
    b.go(&format!("{}/t/TKT-42", server.url))?;
    b.wait("document.querySelectorAll('.pr-link-row a').length===2")?;
    b.wait("document.querySelector('.pr-link-row a').getAttribute('href')==='https://github.com/example/tickets/pull/123' && document.querySelector('[aria-label=\"Pull requests\"]').textContent.includes('Closes ticket') && document.querySelector('[aria-label=\"Pull requests\"]').textContent.includes('Reference')")?;
    for (size, width) in [("desktop", 1440), ("mobile", 390)] {
        b.viewport(width, 900)?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        b.screenshot(&output.join(format!("{phase}-detail-{size}.png")))?;
    }
    println!(
        "PASS project-repos: immutable fixtures, repo-only metadata, deep links, aggregate counts, activity, linked PRs, desktop/mobile screenshots ({phase})"
    );
    Ok(())
}
