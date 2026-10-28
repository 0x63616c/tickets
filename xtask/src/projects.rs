//! Local D1 verification for project management and linking.
use crate::design::Browser;
use serde_json::json;
use std::{env, fs, path::Path};

pub fn check(root: &Path, password: &str) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let phase = if before { "before" } else { "after" };
    let output = root.join("docs/screenshots/projects");
    fs::create_dir_all(&output)?;
    let mut b = Browser::start(root)?;
    b.call("Network.clearBrowserCookies", json!({}))?;
    b.go("http://127.0.0.1:8795/login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/settings/tokens'")?;
    b.eval(r#"(async()=>{
        const api=async(path,body)=>{const r=await fetch('/api/v1/'+path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error(path);return r.json()};
        await api('projects',{name:'Tickets',key_prefix:'TKT',color:'#5e6ad2',github_owner:'example',github_project_number:7,github_project_kind:'orgs',github_repo:'tickets'});
        await api('projects',{name:'Quantum',key_prefix:'QUA',color:'#4ea7fc'});
        await api('tickets',{project_id:1,title:'Link projects to GitHub',status:'todo'});
        return true;
    })()"#)?;
    b.go("http://127.0.0.1:8795/projects")?;
    if before {
        b.wait("document.body.innerText.includes('Page not found')")?;
    } else {
        b.wait("document.querySelectorAll('.api-project-row').length===2")?;
    }
    for (size, width) in [("desktop", 1440), ("mobile", 390)] {
        b.viewport(width, 900)?;
        if !before && width == 390 {
            b.wait("document.documentElement.scrollWidth<=innerWidth && getComputedStyle(document.querySelector('.project-row-action')).display!=='none' && getComputedStyle(document.querySelector('.api-project-row > span:nth-child(3)')).display==='none'")?;
        }
        b.screenshot(&output.join(format!("projects-{phase}-{size}.png")))?;
    }
    b.viewport(1440, 900)?;
    if before {
        b.go("http://127.0.0.1:8795/design/projects")?;
        b.wait("document.querySelectorAll('.project-row').length===2")?;
        b.screenshot(&output.join("projects-target-desktop.png"))?;
        return Ok(());
    }
    b.wait("document.querySelector('.sidebar .nav-count')?.textContent==='2' && [...document.querySelectorAll('.sidebar a')].some(e=>e.textContent.includes('New project')) && !document.querySelector('.projects-page .view-footer')")?;
    b.wait("[...document.querySelectorAll('.api-project-row')].every(e=>e.querySelector('.project-title .mono') && e.querySelector('.project-mark').textContent.trim().length===1 && e.querySelector('button[aria-label] svg')) && getComputedStyle(document.querySelector('.sidebar')).width==='240px'")?;
    b.eval("document.querySelector('.project-row-action').focus();true")?;
    b.wait("getComputedStyle(document.querySelector('.project-row-action')).opacity==='1'")?;
    b.click(".sidebar a[href='/projects?new=true']")?;
    b.wait("document.activeElement.id==='project-name'")?;
    b.click("dialog .modal-footer .button-ghost")?;
    b.wait("!document.querySelector('dialog')")?;
    b.click(".sidebar a[href='/projects?new=true']")?;
    b.wait("document.activeElement.id==='project-name'")?;
    b.wait("!document.querySelector('#project-kind') && !document.querySelector('#project-number') && !document.querySelector('input[type=color]') && !document.querySelector('dialog').textContent.includes('Changing the prefix')")?;
    b.click("[aria-label='Infrastructure color']")?;
    b.wait("document.querySelector('[aria-label=\"Infrastructure color\"]').getAttribute('aria-pressed')==='true'")?;
    b.text("#project-name", "Engine")?;
    b.text("#project-prefix", "ENG")?;
    b.text(
        "#project-owner",
        "https://github.com/users/example/projects/9",
    )?;
    b.wait("!document.querySelector('#project-kind')")?;
    b.text("#project-repo", "engine")?;
    b.screenshot(&output.join("create-after-desktop.png"))?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join("create-after-mobile.png"))?;
    b.viewport(1440, 900)?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && document.querySelectorAll('.api-project-row').length===3")?;
    b.click("[aria-label='Edit project Engine']")?;
    b.wait("document.querySelector('dialog').textContent.includes('Changing the prefix') && document.querySelector('[aria-label=\"Infrastructure color\"]').getAttribute('aria-pressed')==='true'")?;
    b.screenshot(&output.join("edit-after-desktop.png"))?;
    b.text("#project-owner", "example/4")?;
    b.wait("!!document.querySelector('#project-kind')")?;
    b.eval("var s=document.querySelector('#project-kind');s.value='users';s.dispatchEvent(new Event('change',{bubbles:true}));true")?;
    b.text("#project-name", "Engine renamed")?;
    b.text("#project-prefix", "REN")?;
    b.text(
        "#project-owner",
        "https://evil.test/orgs/example/projects/4",
    )?;
    b.click("dialog button[type=submit]")?;
    b.wait("document.querySelector('dialog [role=alert]')?.textContent.includes('github.com')")?;
    b.text(
        "#project-owner",
        "https://github.com/orgs/example/projects/4",
    )?;
    b.wait("!document.querySelector('#project-kind')")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && !!document.querySelector('.api-project-row a[href=\"https://github.com/orgs/example/projects/4\"]')")?;
    b.click("[aria-label='Edit project Engine renamed']")?;
    b.text("#project-owner", "example/4")?;
    b.wait("document.querySelector('#project-kind')?.value==='orgs'")?;
    b.screenshot(&output.join("edit-short-after-desktop.png"))?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && !!document.querySelector('[aria-label=\"Edit project Engine renamed\"]')")?;
    b.go("http://127.0.0.1:8795/projects")?;
    b.wait("document.querySelectorAll('.api-project-row').length===3")?;
    b.wait("!!document.querySelector('.api-project-row a[href=\"https://github.com/orgs/example/projects/4\"]')")?;
    b.click("[aria-label='Edit project Engine renamed']")?;
    b.click("dialog input[type=checkbox]")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && document.querySelectorAll('.api-project-row').length===2 && ![...document.querySelectorAll('.sidebar .project-nav')].some(e=>e.textContent.includes('Engine'))")?;
    b.click(".projects-heading input")?;
    b.wait("document.querySelectorAll('.api-project-row').length===3 && document.body.innerText.includes('Archived')")?;
    b.screenshot(&output.join("archived-after-desktop.png"))?;
    b.click("[aria-label='Edit project Engine renamed']")?;
    b.click("dialog input[type=checkbox]")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && [...document.querySelectorAll('.sidebar .project-nav')].some(e=>e.textContent.includes('Engine'))")?;
    b.eval("[...document.querySelectorAll('.project-title')].find(e=>e.textContent.includes('Tickets')).click();true")?;
    b.wait("location.pathname==='/' && !!document.querySelector('.api-ticket')")?;
    b.wait("!!document.querySelector('.view-tabs a[href=\"https://github.com/orgs/example/projects/7\"]')")?;
    b.screenshot(&output.join("header-after-desktop.png"))?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join("header-after-mobile.png"))?;
    b.viewport(1440, 900)?;
    b.click(".topbar .button-primary")?;
    b.text("#board-title", "Project workflow ticket")?;
    b.eval("var s=document.querySelector('[aria-label=\"New ticket project\"]');s.value='3';s.dispatchEvent(new Event('change',{bubbles:true}));true")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog')")?;
    b.eval("[...document.querySelectorAll('.sidebar .project-nav')].find(e=>e.textContent.includes('Engine')).click();true")?;
    b.wait("document.querySelector('#ticket-2')?.textContent.includes('Project workflow ticket')")?;
    b.eval("var s=document.querySelector('#ticket-2 select');s.value='In progress';s.dispatchEvent(new Event('change',{bubbles:true}));true")?;
    b.wait("document.querySelector('#ticket-2')?.closest('section').getAttribute('aria-label')==='In progress' && !document.querySelector('#ticket-2 select').disabled")?;
    b.eval("(async()=>{const r=await fetch('/api/v1/labels',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name:'Feature',color:'#8b82d9'})});if(!r.ok)throw Error('label seed');return true})()")?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("document.querySelector('[aria-label=Project]')?.value==='3'")?;
    b.click(".label-menu summary")?;
    b.click(".label-menu .menu-item")?;
    b.wait("!!document.querySelector('.property-labels .label-chip')")?;
    b.eval("[...document.querySelectorAll('button')].find(e=>e.textContent.includes('Add dependency')).click();true")?;
    b.wait("!!document.querySelector('#dependency-search')")?;
    b.click(".dependency-options button")?;
    b.wait("!document.querySelector('dialog') && !!document.querySelector('.dependency-row')")?;
    b.eval("var s=document.querySelector('[aria-label=Project]');s.value='2';s.dispatchEvent(new Event('change',{bubbles:true}));true")?;
    b.wait("document.querySelector('[aria-label=Project]')?.value==='2' && document.querySelector('.breadcrumbs')?.textContent.includes('QUA-1')")?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("document.querySelector('[aria-label=Project]')?.value==='2' && !!document.querySelector('.property-labels .label-chip') && !!document.querySelector('.dependency-row')")?;
    println!(
        "PASS projects: UI create, rename, prefix, URL validation, user/org linking, repo, persistence, archive/restore, sidebar and header; ticket project selection and reassignment, Kanban move, label and dependency; desktop/mobile screenshots"
    );
    Ok(())
}
