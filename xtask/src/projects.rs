//! Local D1 verification for project management and linking.
use crate::design::Browser;
use serde_json::json;
use std::{env, fs, path::Path};

pub fn check(root: &Path, password: &str) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let phase = "after";
    let output = root.join("docs/screenshots/projects");
    fs::create_dir_all(&output)?;
    let mut b = Browser::start_named(root, "projects-browser")?;
    b.call("Network.clearBrowserCookies", json!({}))?;
    b.go("http://127.0.0.1:8795/login?next=%2Fsettings%2Ftokens")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("['/settings/tokens','/settings/api-tokens'].includes(location.pathname)")?;
    b.eval(r#"(async()=>{
        const api=async(path,body)=>{const r=await fetch('/api/v1/'+path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error(path);return r.json()};
        await api('projects',{name:'Tickets',key_prefix:'TKT',color:'#5e6ad2',github_owner:'example',github_project_number:7,github_project_kind:'orgs',github_repo:'tickets'});
        await api('projects',{name:'Quantum',key_prefix:'QUA',color:'#4ea7fc',github_owner:'example',github_repo:'quantum'});
        await api('tickets',{project_id:1,title:'Link projects to GitHub',status:'todo'});
        return true;
    })()"#)?;
    let labels_output = root.join("docs/screenshots/45");
    fs::create_dir_all(&labels_output)?;
    if before {
        b.eval(r#"(async()=>{
            const api=async(path,body)=>{const r=await fetch('/api/v1/'+path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error(path);return r.json()};
            await api('projects',{name:'Engine renamed',key_prefix:'REN',color:'#4ea7fc',github_owner:'example',github_project_number:4,github_project_kind:'orgs',github_repo:'engine'});
            await api('tickets',{project_id:3,title:'Project workflow ticket',status:'in_progress'});
            return true;
        })()"#)?;
        b.go("http://127.0.0.1:8795/ticket?id=2")?;
        b.wait("!!document.querySelector('.label-menu .menu-trigger')")?;
        b.click(".label-menu .menu-trigger")?;
        for (size, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
            b.viewport(width, height)?;
            b.eval("document.querySelector('.label-menu').scrollIntoView({block:'center'});true")?;
            b.screenshot(&labels_output.join(format!("before-{size}.png")))?;
        }
        b.viewport(1440, 900)?;
        b.go("http://127.0.0.1:8795/design/ticket")?;
        b.wait("!!document.querySelector('.label-menu')")?;
        b.screenshot(&labels_output.join("target-desktop.png"))?;
        return Ok(());
    }
    b.go("http://127.0.0.1:8795/projects")?;
    b.wait("document.querySelectorAll('.api-project-row').length===2")?;
    for (size, width) in [("desktop", 1440), ("mobile", 390)] {
        b.viewport(width, 900)?;
        if width == 390 {
            b.wait("document.documentElement.scrollWidth<=innerWidth && getComputedStyle(document.querySelector('.project-row-action')).display!=='none' && getComputedStyle(document.querySelector('.api-project-row > span:nth-child(3)')).display!=='none'")?;
        }
        b.screenshot(&output.join(format!("projects-{phase}-{size}.png")))?;
    }
    b.viewport(1440, 900)?;
    b.wait("document.querySelector('.sidebar .nav-count')?.textContent==='1' && [...document.querySelectorAll('.sidebar a')].some(e=>e.textContent.includes('New project')) && !document.querySelector('.projects-page .view-footer')")?;
    b.wait("[...document.querySelectorAll('.api-project-row')].every(e=>e.querySelector('.project-title .mono') && e.querySelector('.project-mark').textContent.trim().length===1 && e.querySelector('button[aria-label] svg')) && getComputedStyle(document.querySelector('.sidebar')).width==='240px'")?;
    b.wait("!!document.querySelector('.api-project-row a[href=\"https://github.com/example/quantum\"]')")?;
    b.click("[aria-label='Edit project Quantum']")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && !!document.querySelector('.api-project-row a[href=\"https://github.com/example/quantum\"]')")?;
    b.eval("document.querySelector('.project-row-action').focus();true")?;
    b.wait("getComputedStyle(document.querySelector('.project-row-action')).opacity==='1'")?;
    b.click(".sidebar a[href='/projects?new=true']")?;
    b.wait("document.activeElement.id==='project-title'")?;
    b.click("dialog .modal-footer .button-ghost")?;
    b.wait("!document.querySelector('dialog')")?;
    b.click(".sidebar a[href='/projects?new=true']")?;
    b.wait("document.activeElement.id==='project-title'")?;
    b.wait("!document.querySelector('[aria-label=\"Owner type\"]') && !document.querySelector('#project-number') && !document.querySelector('input[type=color]') && !document.querySelector('dialog').textContent.includes('Changing the prefix')")?;
    b.click("[aria-label='Infrastructure color']")?;
    b.wait("document.querySelector('[aria-label=\"Infrastructure color\"]').getAttribute('aria-pressed')==='true'")?;
    b.text("#project-title", "Engine")?;
    b.text("#project-prefix", "ENG")?;
    b.choose("[aria-label='Project icon']", "box")?;
    b.text(
        "#project-owner",
        "https://github.com/users/example/projects/9",
    )?;
    b.wait("!document.querySelector('[aria-label=\"Owner type\"]')")?;
    b.text("#project-repo", "engine")?;
    b.screenshot(&output.join("create-after-desktop.png"))?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join("create-after-mobile.png"))?;
    b.viewport(1440, 900)?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && document.querySelectorAll('.api-project-row').length===3")?;
    b.wait("document.querySelector('[aria-label=\"Edit project Engine\"]').closest('.project-row').querySelector('.project-mark svg')!==null")?;
    b.click("[aria-label='Edit project Engine']")?;
    b.wait("document.querySelector('dialog').textContent.includes('Changing the prefix') && document.querySelector('[aria-label=\"Infrastructure color\"]').getAttribute('aria-pressed')==='true'")?;
    b.screenshot(&output.join("edit-after-desktop.png"))?;
    b.text("#project-owner", "example/4")?;
    b.wait("!!document.querySelector('[aria-label=\"Owner type\"]')")?;
    b.choose("[aria-label='Owner type']", "users")?;
    b.text("#project-title", "Engine renamed")?;
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
    b.wait("!document.querySelector('[aria-label=\"Owner type\"]')")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && !!document.querySelector('.api-project-row a[href=\"https://github.com/orgs/example/projects/4\"]')")?;
    b.click("[aria-label='Edit project Engine renamed']")?;
    b.text("#project-owner", "example/4")?;
    b.wait("document.querySelector('[aria-label=\"Owner type\"]')?.value==='orgs'")?;
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
    b.wait("[...document.querySelectorAll('.project-title')].some(e=>e.textContent.includes('Tickets'))")?;
    b.eval("[...document.querySelectorAll('.project-title')].find(e=>e.textContent.includes('Tickets')).click();true")?;
    b.wait("location.pathname==='/board' && !!document.querySelector('.api-ticket')")?;
    b.wait("!!document.querySelector('.view-tabs a[href=\"https://github.com/orgs/example/projects/7\"]')")?;
    b.screenshot(&output.join("header-after-desktop.png"))?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join("header-after-mobile.png"))?;
    b.viewport(1440, 900)?;
    b.click("[aria-label='Add ticket to Todo']")?;
    b.text("#board-title", "Project workflow ticket")?;
    b.choose("[aria-label=\"New ticket project\"]", "3")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog')")?;
    b.eval("[...document.querySelectorAll('.sidebar .project-nav')].find(e=>e.textContent.includes('Engine')).click();true")?;
    b.wait("document.querySelector('#ticket-2')?.textContent.includes('Project workflow ticket')")?;
    b.choose("#ticket-2 [role=combobox]", "In progress")?;
    b.wait("document.querySelector('#ticket-2')?.closest('section').getAttribute('aria-label')==='In progress' && !document.querySelector('#ticket-2 [role=combobox]').disabled")?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("document.querySelector('[aria-label=Project]')?.value==='3'")?;
    b.eval("(async()=>{const r=await fetch('/api/v1/labels');const body=await r.json();if(body.data.length!==0)throw Error('expected no labels');return true})()")?;
    b.click(".label-menu .menu-trigger")?;
    b.wait("document.activeElement.id==='label-filter'")?;
    b.eval("(()=>{const i=document.querySelector('#label-filter');if(i.getAttribute('aria-label')!=='Filter or create label'||i.placeholder!=='Filter labels…')throw Error('label filter accessibility');const r=i.getBoundingClientRect(),m=i.closest('.popup').getBoundingClientRect();if(r.left-5<m.left||r.right+5>m.right)throw Error('label filter focus overflow');return true})()")?;
    b.text("#label-filter", "Feature")?;
    b.wait("document.querySelector('.label-menu .menu-item')?.textContent.includes('Create label Feature')")?;
    for (size, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
        b.viewport(width, height)?;
        b.eval("document.querySelector('.label-menu').scrollIntoView({block:'center'});true")?;
        b.screenshot(&labels_output.join(format!("after-{size}.png")))?;
    }
    b.viewport(1440, 900)?;
    b.eval("document.querySelector('#label-filter').focus();true")?;
    b.key("ArrowDown", 40, 0)?;
    b.wait("document.activeElement.classList.contains('menu-item')")?;
    b.key("ArrowUp", 38, 0)?;
    b.wait("document.activeElement.id==='label-filter'")?;
    b.key("Enter", 13, 0)?;
    b.wait("!!document.querySelector('.property-labels .label-chip') && document.activeElement.matches('.label-menu .menu-trigger')")?;
    b.click(".label-menu .menu-trigger")?;
    b.wait("document.querySelector('#label-filter').value===''")?;
    b.text("#label-filter", "FEATURE")?;
    b.wait("document.querySelectorAll('.label-menu .menu-item').length===1 && !document.querySelector('.label-menu').textContent.includes('Create label')")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.querySelector('.menu-trigger').getAttribute('aria-expanded')==='false' && document.activeElement.matches('.label-menu .menu-trigger')")?;
    // Remove, then attach the case-insensitive match with Enter; no second label.
    b.click(".label-menu .menu-trigger")?;
    b.click(".label-menu .menu-item")?;
    b.wait("!document.querySelector('.property-labels .label-chip')")?;
    b.click(".label-menu .menu-trigger")?;
    b.wait("document.querySelector('#label-filter').value===''")?;
    b.text("#label-filter", "feature")?;
    b.key("Enter", 13, 0)?;
    b.wait("!!document.querySelector('.property-labels .label-chip')")?;
    b.eval("(async()=>{const r=await fetch('/api/v1/labels');const body=await r.json();if(body.data.length!==1 || body.data[0].name!=='Feature')throw Error('duplicate label');return true})()")?;
    b.click(".label-menu .menu-trigger")?;
    b.wait("document.querySelector('#label-filter').value===''")?;
    b.text("#label-filter", "Bug")?;
    b.click(".label-menu [data-create]")?;
    b.wait("document.querySelectorAll('.property-labels .label-chip').length===2 && document.activeElement.matches('.label-menu .menu-trigger')")?;
    for (size, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
        b.viewport(width, height)?;
        b.eval("document.querySelector('.label-menu').scrollIntoView({block:'center'});true")?;
        b.screenshot(&labels_output.join(format!("attached-{size}.png")))?;
    }
    b.viewport(1440, 900)?;
    b.click(".label-menu .menu-trigger")?;
    b.wait("document.querySelector('#label-filter').value==='' && document.querySelectorAll('.label-menu .menu-item').length===2")?;
    b.text("#label-filter", "feat")?;
    b.wait("document.querySelectorAll('.label-menu .menu-item').length===2 && !document.querySelector('.label-menu .popup').textContent.includes('Bug')")?;
    b.text("#label-filter", "   ")?;
    b.wait("document.querySelectorAll('.label-menu .menu-item').length===2 && !document.querySelector('.label-menu [data-create]')")?;
    b.text("#label-filter", "FEATURE")?;
    b.key("Enter", 13, 0)?;
    b.wait("document.querySelectorAll('.property-labels .label-chip').length===2 && document.activeElement.matches('.label-menu .menu-trigger')")?;
    b.eval("[...document.querySelectorAll('button')].find(e=>e.textContent.includes('Add dependency')).click();true")?;
    b.wait("!!document.querySelector('#dependency-search')")?;
    b.click(".dependency-options button")?;
    b.wait("!document.querySelector('dialog') && !!document.querySelector('.dependency-row')")?;
    b.choose("[aria-label=Project]", "2")?;
    b.wait("document.querySelector('[aria-label=Project]')?.value==='2' && document.querySelector('.breadcrumbs')?.textContent.includes('QUA-1')")?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("document.querySelector('[aria-label=Project]')?.value==='2' && !!document.querySelector('.property-labels .label-chip') && !!document.querySelector('.dependency-row')")?;
    println!(
        "PASS projects: UI create, rename, prefix, URL validation, user/org linking, repo, persistence, archive/restore, sidebar and header; ticket project selection and reassignment, Kanban move, label creation from empty (Enter/click), filtering, duplicate reuse, arrows/Escape/focus and dependency; desktop/mobile screenshots"
    );
    Ok(())
}
