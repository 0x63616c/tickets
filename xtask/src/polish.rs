//! #15 visual audit of real routes, using isolated local Worker/D1 data.
use crate::design::Browser;
use serde_json::json;
use std::{env, fs, path::Path};

type Result = super::Result;
const URL: &str = "http://127.0.0.1:8795";

fn pair(b: &mut Browser, output: &Path, phase: &str, screen: &str) -> Result {
    for (size, width) in [("desktop", 1440), ("mobile", 390)] {
        b.viewport(width, 900)?;
        if phase == "after" {
            b.wait("document.documentElement.scrollWidth <= innerWidth")?;
        }
        b.screenshot(&output.join(format!("{screen}-{phase}-{size}.png")))?;
    }
    b.viewport(1440, 900)?;
    Ok(())
}

pub fn check(root: &Path, password: &str) -> Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let phase = if before { "before" } else { "after" };
    let output = env::var_os("TICKETS_SCREENSHOT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("docs/screenshots/15"));
    fs::create_dir_all(&output)?;
    let mut b = Browser::start(root)?;
    b.call("Network.clearBrowserCookies", json!({}))?;
    b.go(&format!("{URL}/login"))?;
    pair(&mut b, &output, phase, "login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname === '/settings/tokens'")?;
    b.go(&format!("{URL}/"))?;
    b.wait("document.body.innerText.includes('No projects yet')")?;
    pair(&mut b, &output, phase, "empty-workspace")?;
    b.eval(r#"(async()=>{
        const api=async(path,body,method='POST')=>{const r=await fetch('/api/v1/'+path,{method,headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error('Seed request failed: '+path);return r.json()};
        await api('projects',{name:'Tickets',key_prefix:'TKT',color:'#5e6ad2'});
        await api('projects',{name:'Quantum',key_prefix:'QUA',color:'#4ea7fc'});
        for(const [name,color] of [['Feature','#8b82d9'],['Bug','#eb5757'],['Infrastructure','#4ea7fc'],['Design','#c6a1b9']])await api('labels',{name,color});
        const groups=[
          ['backlog', ['Remember filters when switching views','Import labels from GitHub issues','Add a keyboard shortcut reference','Archive completed projects']],
          ['todo', ['Group the ticket list by status','Create and edit tickets','Link projects to GitHub','Keep open views in sync']],
          ['in_progress', ['Move tickets between board columns','Define the dark-mode design system','Expose the tickets REST API','Preserve focus after closing a dialog']],
          ['done', ['Set up the Rust workspace','Model tickets and dependencies','Add password login and API tokens']]
        ];
        let n=0;
        for(const [status,titles] of groups)for(const title of titles){
          n++;
          const ticket=await api('tickets',{project_id:1,title,status,priority:n===4?0:n<5?4:2,sort_order:n*1024});
          const label=n===3||n===10?4:n===12?2:[8,11,13,14].includes(n)?3:1;
          await api('tickets/'+ticket.id+'/labels/'+label,{});
        }
        await api('tickets/11/dependencies/6',{});
        await api('tickets/11/dependencies/8',{});
        await api('tickets/14/dependencies/9',{});
        await api('tickets/9/dependencies/8',{});
        await api('tickets/9',{body:'Drag a ticket to another column to change its status. Update the board immediately and keep other open views in sync.\n\n### Acceptance criteria\n- [x] Group tickets by status, with priority and labels visible\n- [x] Offer a **status menu** as an alternative to dragging\n- [ ] Keep keyboard focus on the ticket after a move\n- [ ] Restore the previous status if saving fails\n\nIf a move fails, show `Couldn’t move ticket` with a **Retry** action. Keep the ticket visible in its original column.',external_url:'https://github.com/0x63616c/tickets/issues/7'},'PATCH');
        await api('tickets/9/comments',{body:'The status menu should work everywhere the ticket appears. Dragging can stay a convenience.'});
        await api('tickets/9/comments',{body:'Keep **keyboard focus** visible and announce the new status after a move.'});
        return true;
    })()"#)?;
    b.go(&format!("{URL}/"))?;
    b.wait("document.querySelectorAll('.api-ticket').length===15")?;
    pair(&mut b, &output, phase, "kanban")?;
    b.click(".topbar .button-primary")?;
    b.wait("!!document.querySelector('#board-title')")?;
    pair(&mut b, &output, phase, "create-dialog")?;
    b.click("[aria-label='Close dialog']")?;
    b.click(".search-trigger")?;
    b.wait("!!document.querySelector('.command-results .menu-item')")?;
    pair(&mut b, &output, phase, "command-palette")?;
    b.text(".command-search input", "No matching ticket")?;
    pair(&mut b, &output, phase, "command-empty")?;
    b.click("[aria-label='Close dialog']")?;
    b.go(&format!("{URL}/list?project=1"))?;
    b.wait("document.querySelectorAll('.api-list-row').length===15")?;
    pair(&mut b, &output, phase, "list")?;
    b.go(&format!("{URL}/list?project=1&q=No%20matching%20ticket"))?;
    b.wait("!document.querySelector('.api-list-row') && document.body.innerText.includes('No tickets')")?;
    pair(&mut b, &output, phase, "list-empty")?;
    b.go(&format!("{URL}/ticket?id=9"))?;
    b.wait("document.querySelectorAll('.comment').length===2 && document.querySelectorAll('.property select option').length>10")?;
    pair(&mut b, &output, phase, "ticket-detail")?;
    b.viewport(390, 900)?;
    b.eval("document.querySelector('.properties').scrollIntoView();true")?;
    b.screenshot(&output.join(format!("ticket-properties-{phase}-mobile.png")))?;
    b.viewport(1440, 900)?;
    b.go(&format!("{URL}/"))?;
    b.wait("document.querySelectorAll('.api-ticket').length===15")?;
    b.eval("[...document.querySelectorAll('.project-nav')].find(e=>e.textContent.includes('Quantum')).click();true")?;
    b.wait("document.querySelector('.breadcrumb-current')?.textContent==='Quantum' && !document.querySelector('.api-ticket')")?;
    pair(&mut b, &output, phase, "empty-project")?;
    b.go(&format!("{URL}/settings/tokens"))?;
    b.wait("!!document.querySelector('.tokens li')")?;
    pair(&mut b, &output, phase, "api-tokens")?;
    // Projects remains a fixture until #10. Capture it honestly as a preview.
    b.go(&format!("{URL}/design/projects"))?;
    b.wait("document.querySelectorAll('.project-row').length===2")?;
    pair(&mut b, &output, phase, "projects-preview")?;
    if before {
        for (route, name) in [
            ("design", "shell"),
            ("design/board", "kanban"),
            ("design/list", "list"),
            ("design/ticket", "ticket-detail"),
            ("design/empty", "empty-project"),
        ] {
            b.go(&format!("{URL}/{route}"))?;
            b.screenshot(&output.join(format!("{name}-target-desktop.png")))?;
        }
    }
    println!(
        "PASS polish: {phase} captures of real board, list, detail, creation, commands, login, tokens and empty states at 1440×900 / 390×900; Projects preview explicitly labelled"
    );
    Ok(())
}
