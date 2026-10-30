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
    b.go(&format!("{URL}/login?next=%2Fsettings%2Ftokens"))?;
    pair(&mut b, &output, phase, "login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("['/settings/tokens','/settings/api-tokens'].includes(location.pathname)")?;
    b.go(&format!("{URL}/board"))?;
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
    b.go(&format!("{URL}/board"))?;
    b.wait("document.querySelectorAll('.api-ticket').length===15")?;
    pair(&mut b, &output, phase, "kanban")?;
    b.click("[aria-label='Add ticket to Todo']")?;
    b.wait("!!document.querySelector('#board-title')")?;
    pair(&mut b, &output, phase, "create-dialog")?;
    b.click("[aria-label='Close dialog']")?;
    b.click(".search-trigger")?;
    b.wait("!!document.querySelector('.command-results .menu-item')")?;
    pair(&mut b, &output, phase, "command-palette")?;
    b.text(".command-search input", "zzzz-no-match-zzzz")?;
    pair(&mut b, &output, phase, "command-empty")?;
    b.click("[aria-label='Close dialog']")?;
    b.go(&format!("{URL}/list?project=1"))?;
    b.wait("document.querySelectorAll('.api-list-row').length===15")?;
    pair(&mut b, &output, phase, "list")?;
    b.go(&format!("{URL}/list?project=1&q=No%20matching%20ticket"))?;
    b.wait("!document.querySelector('.api-list-row') && document.body.innerText.includes('No tickets')")?;
    pair(&mut b, &output, phase, "list-empty")?;
    b.go(&format!("{URL}/ticket?id=9"))?;
    b.wait("document.querySelectorAll('.comment').length===2 && document.querySelectorAll('.property [role=option]').length>10")?;
    pair(&mut b, &output, phase, "ticket-detail")?;
    b.viewport(390, 900)?;
    b.eval("document.querySelector('.properties').scrollIntoView();true")?;
    b.screenshot(&output.join(format!("ticket-properties-{phase}-mobile.png")))?;
    b.viewport(1440, 900)?;
    b.go(&format!("{URL}/board"))?;
    b.wait("document.querySelectorAll('.api-ticket').length===15")?;
    b.eval("[...document.querySelectorAll('.project-nav')].find(e=>e.textContent.includes('Quantum')).click();true")?;
    b.wait("document.querySelector('.breadcrumb-current')?.textContent.endsWith('Quantum') && !document.querySelector('.api-ticket')")?;
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
    } else {
        interactions(&mut b)?;
    }
    if !before {
        // The broad interaction audit deliberately ends logged out.
        b.text("#username", "fixture")?;
        b.text("#password", password)?;
        b.click("button[type=submit]")?;
        b.wait("location.pathname!='/login'")?;
    }
    super::cleanup::check(&mut b, root)?;
    println!(
        "PASS polish: {phase} captures of real board, list, detail, creation, commands, login, tokens and empty states at 1440×900 / 390×900; Projects preview explicitly labelled"
    );
    Ok(())
}

fn interactions(b: &mut Browser) -> Result {
    b.go(&format!("{URL}/board"))?;
    b.wait("document.querySelectorAll('.api-ticket').length===15 && document.querySelectorAll('.board-column').length===4")?;
    b.wait("document.querySelector('.sidebar').getBoundingClientRect().width===240 && [...document.querySelectorAll('.board-column')].every(e=>e.getBoundingClientRect().width>=240)")?;
    // Picking up a card does not shift columns. Moving to review reveals its group.
    b.eval("document.querySelector('#ticket-9').focus();true")?;
    b.key(" ", 32, 0)?;
    b.wait("document.querySelectorAll('.board-column').length===4 && document.activeElement.id==='ticket-9'")?;
    b.key("ArrowRight", 39, 0)?;
    b.wait("document.querySelector('#ticket-9').closest('section').getAttribute('aria-label')==='In review'")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.querySelectorAll('.board-column').length===4 && document.activeElement.id==='ticket-9' && document.querySelector('#ticket-9').closest('section').getAttribute('aria-label')==='In progress'")?;
    b.key("ArrowRight", 39, 0)?;
    b.wait("document.activeElement.id==='ticket-13'")?;
    b.click("[aria-label='Add ticket to Todo']")?;
    b.wait("document.activeElement.id==='board-title'")?;
    b.key("k", 75, 4)?;
    b.wait("document.querySelectorAll('dialog').length===1 && !!document.querySelector('#board-title')")?;
    b.key("Escape", 27, 0)?;
    b.wait("!document.querySelector('dialog') && document.activeElement.getAttribute('aria-label')==='Add ticket to Todo'")?;
    b.click(".search-trigger")?;
    b.text(".command-search input", "zzzz-no-match-zzzz")?;
    b.wait("document.querySelector('.command-empty')?.textContent.includes('No results')")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.matches('.search-trigger')")?;
    b.click(".search-trigger")?;
    b.text(".command-search input", "Move tickets")?;
    b.wait("document.querySelector('.command-option[aria-selected=true]')?.textContent.includes('TKT-9') && document.activeElement.matches('.command-search input')")?;
    b.key("Enter", 13, 0)?;
    b.wait("location.pathname==='/t/TKT-9' && !!document.querySelector('.comment')")?;
    b.wait("document.querySelector('.comment-form button[type=submit]').getBoundingClientRect().bottom<=900 && !!document.querySelector('.property .select-trigger > .icon')")?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.click("[aria-label='Toggle sidebar']")?;
    b.wait("document.querySelector('[aria-label=\"Toggle sidebar\"]').getAttribute('aria-expanded')==='true' && document.querySelector('.sidebar').getBoundingClientRect().width===240")?;
    b.wait("document.activeElement.matches('.sidebar a[href=\"/\"]')")?;
    b.key("Tab", 9, 8)?;
    b.wait("document.activeElement.matches('.sidebar .account-trigger')")?;
    b.key("Tab", 9, 0)?;
    b.wait("document.activeElement.matches('.sidebar a[href=\"/\"]')")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.getAttribute('aria-label')==='Toggle sidebar' && !document.querySelector('.sidebar-dismiss')")?;
    b.go(&format!("{URL}/list?project=1"))?;
    b.wait("document.querySelectorAll('.api-list-row').length===15")?;
    b.wait("[...document.querySelectorAll('.board-filters .select-trigger')].every(e=>e.getBoundingClientRect().width>=140) && [...document.querySelectorAll('.api-list-row')].every(e=>e.getBoundingClientRect().height===36) && new Set([...document.querySelectorAll('.api-list-row .row-title')].map(e=>e.getBoundingClientRect().x)).size===1")?;
    // Long user-supplied text must remain contained at both viewport sizes.
    b.eval(r#"(async()=>{const r=await fetch('/api/v1/tickets/1',{method:'PATCH',headers:{'Content-Type':'application/json'},body:JSON.stringify({title:'Preserve keyboard focus when moving tickets across projects with a very long title — '+ 'LongTitle'.repeat(20)})});if(!r.ok)throw Error('long title seed failed');return true})()"#)?;
    for width in [1440, 390] {
        b.viewport(width, 900)?;
        for route in ["/board", "/list?project=1", "/ticket?id=1"] {
            b.go(&format!("{URL}{route}"))?;
            b.wait("document.body.innerText.includes('LongTitle') && document.documentElement.scrollWidth<=innerWidth")?;
        }
    }
    // Token creation is exercised, but a secret is never captured in an artifact.
    b.go(&format!("{URL}/settings/tokens"))?;
    b.wait("!!document.querySelector('.tokens li')")?;
    b.click(".token-create")?;
    b.text("#token-label", "Build agent")?;
    b.click(".settings-footer button[type=submit]")?;
    b.wait("document.querySelector('.secret textarea')?.value.startsWith('tk_')")?;
    b.click(".token-saved input")?;
    b.click(".token-acknowledge")?;
    b.wait("!document.querySelector('.secret textarea') && document.querySelectorAll('.tokens li').length===2")?;
    b.click(".tokens li:last-child button")?;
    b.wait("!!document.querySelector('.token-revoke')")?;
    b.click(".modal-footer .button-danger")?;
    b.wait("document.querySelector('.token-result').textContent.includes('Revoked') && document.querySelectorAll('.tokens li').length===1")?;
    b.call("Network.clearBrowserCookies", json!({}))?;
    b.go(&format!("{URL}/settings/tokens"))?;
    b.wait("location.pathname==='/login' && !document.querySelector('.sidebar')")?;
    b.go(&format!("{URL}/login?next=%2Fsettings%2Ftokens"))?;
    b.eval("window.signInTop=document.querySelector('button[type=submit]').getBoundingClientRect().top;true")?;
    b.text("#username", "fixture")?;
    b.text("#password", "incorrect")?;
    b.click("button[type=submit]")?;
    b.wait("document.querySelector('[role=alert]').textContent==='Incorrect username or password.' && document.querySelector('#password').value==='' && document.querySelector('button[type=submit]').getBoundingClientRect().top===window.signInTop")?;
    println!(
        "PASS polish interactions: 240px sidebar/columns, review move/cancel/focus, skip empty columns, modal focus restoration/no stacking, command empty/keyboard/open, visible comment submit, shared chevrons, mobile sidebar/Escape, readable mobile filters, aligned 36px rows, long text containment, token create/dismiss/revoke (no secret capture)"
    );
    Ok(())
}
