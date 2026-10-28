//! A fresh install exercised exclusively through visible UI controls.
use crate::design::Browser;
use std::{fs, path::Path};

pub fn check(root: &Path, password: &str) -> super::Result {
    let output = root.join("docs/screenshots/1");
    fs::create_dir_all(&output)?;
    // Keep cookies, cached assets and browser-version state out of later runs.
    let profile = format!("first-run-browser-{}", std::process::id());
    let mut b = Browser::start_named(root, &profile)?;
    b.viewport(1440, 900)?;
    b.go("http://127.0.0.1:8795/login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/' && document.body.innerText.includes('No projects yet')")?;
    b.screenshot(&output.join("first-run-empty.png"))?;
    b.click(".empty-project a")?;
    b.wait("!!document.querySelector('.projects-page') && !document.querySelector('.topbar .button-primary').disabled")?;
    b.click(".topbar .button-primary")?;
    b.wait("!!document.querySelector('#project-title')")?;
    b.text("#project-title", "First project")?;
    b.text("#project-prefix", "FIRST")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && document.querySelectorAll('.api-project-row').length===1 && document.querySelector('.project-title')?.textContent.includes('First project')")?;
    b.screenshot(&output.join("first-run-project.png"))?;
    b.click(".project-title")?;
    b.wait("location.pathname==='/' && !document.querySelector('.api-ticket') && !document.querySelector('.topbar .button-primary').disabled")?;
    for title in ["Prepare release", "Publish release"] {
        b.click(".topbar .button-primary")?;
        b.wait("!!document.querySelector('#board-title')")?;
        b.text("#board-title", title)?;
        b.click("dialog button[type=submit]")?;
        b.wait("!document.querySelector('dialog') && !document.querySelector('.topbar .button-primary').disabled")?;
    }
    b.wait("document.querySelectorAll('.api-ticket').length===2 && document.querySelector('#ticket-1 .card-title')?.textContent==='Prepare release' && document.querySelector('#ticket-2 .card-title')?.textContent==='Publish release'")?;
    b.screenshot(&output.join("first-run-tickets.png"))?;
    // The card's visible status select is also the touch-friendly Kanban move control.
    b.click("#ticket-1 select")?;
    b.key("ArrowDown", 40, 0)?;
    b.key("Enter", 13, 0)?;
    b.wait("document.querySelector('#ticket-1')?.closest('section').getAttribute('aria-label')==='In progress' && !document.querySelector('#ticket-1 select').disabled")?;
    b.screenshot(&output.join("first-run-kanban.png"))?;
    b.click("#ticket-2 .card-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Publish release'")?;
    b.click(".label-menu summary")?;
    b.wait("document.querySelectorAll('.label-menu .menu-item').length===0")?;
    b.text("#label-filter", "Release")?;
    b.wait("document.querySelector('.label-menu [data-create]')?.textContent.includes('Create label Release')")?;
    b.click(".label-menu [data-create]")?;
    b.wait("document.querySelector('.property-labels .label-chip')?.textContent.includes('Release') && !document.querySelector('.label-menu').open")?;
    b.screenshot(&output.join("first-run-label.png"))?;
    b.click(".dependencies .section-heading button")?;
    b.wait("document.querySelector('[aria-label=Direction]')?.value==='Is blocked by'")?;
    b.text("#dependency-search", "Prepare release")?;
    b.wait("document.querySelectorAll('.dependency-options button').length===1")?;
    b.click(".dependency-options button")?;
    b.wait("!document.querySelector('dialog') && document.querySelector('.dependency-row')?.textContent.includes('Blocked by') && document.querySelector('.dependency-title')?.textContent==='Prepare release'")?;
    b.screenshot(&output.join("first-run-dependency.png"))?;
    b.click(".dependency-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Prepare release' && document.querySelector('[aria-label=Status]')?.value==='In progress' && document.querySelector('.dependency-row')?.textContent.includes('Blocks') && document.querySelector('.dependency-title')?.textContent==='Publish release'")?;
    b.click(".dependency-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Publish release' && document.querySelector('.property-labels .label-chip')?.textContent.includes('Release') && document.querySelector('.dependency-row')?.textContent.includes('Blocked by')")?;
    println!(
        "PASS first-run: empty install, UI login, project, two tickets, Kanban move, new label and dependency verified on both tickets; six dark 1440×900 screenshots"
    );
    Ok(())
}
