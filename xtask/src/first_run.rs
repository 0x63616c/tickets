//! A fresh install exercised exclusively through visible UI controls.
use crate::design::Browser;
use std::{fs, path::Path};

pub fn check(root: &Path) -> super::Result {
    let worker = crate::local_worker::LocalWorker::start(root, Default::default())?;
    let password = worker.password();
    let url = worker.url();
    let output = root.join("docs/screenshots/1");
    let links_output = root.join("docs/screenshots/147");
    fs::create_dir_all(&links_output)?;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let capture = if before { "before" } else { "after" };
    fs::create_dir_all(&output)?;
    // Keep cookies, cached assets and browser-version state out of later runs.
    let profile = format!("first-run-browser-{}", std::process::id());
    let mut b = Browser::start_named(root, &profile)?;
    b.viewport(1440, 900)?;
    b.go(&format!("{url}/login"))?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/' && document.body.innerText.includes('Start with a project')")?;
    b.screenshot(&output.join("first-run-empty.png"))?;
    b.click(".home-empty a")?;
    b.wait("!!document.querySelector('.projects-page') && !document.querySelector('.topbar .button-primary').disabled")?;
    b.wait("!!document.querySelector('#project-title')")?;
    b.text("#project-title", "First project")?;
    b.text("#project-prefix", "FIRST")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && document.querySelectorAll('.api-project-row').length===1 && document.querySelector('.project-title')?.textContent.includes('First project')")?;
    b.screenshot(&output.join("first-run-project.png"))?;
    b.click(".project-title")?;
    b.wait("location.pathname==='/board' && !document.querySelector('.api-ticket') && document.querySelector('[aria-label=\"Add ticket to Todo\"]')?.disabled===false")?;
    for title in ["Prepare release", "Publish release"] {
        b.click("[aria-label='Add ticket to Todo']")?;
        b.wait("!!document.querySelector('#board-title')")?;
        b.text("#board-title", title)?;
        b.click("dialog button[type=submit]")?;
        b.wait("!document.querySelector('dialog') && document.querySelector('[aria-label=\"Add ticket to Todo\"]')?.disabled===false")?;
    }
    b.wait("document.querySelectorAll('.api-ticket').length===2 && document.querySelector('#ticket-1 .card-title')?.textContent==='Prepare release' && document.querySelector('#ticket-2 .card-title')?.textContent==='Publish release'")?;
    b.screenshot(&output.join("first-run-tickets.png"))?;
    // The card's visible status select is also the touch-friendly Kanban move control.
    b.wait("!document.querySelector('#ticket-1 [role=combobox]').disabled")?;
    b.click("#ticket-1 [role=combobox]")?;
    b.wait("document.querySelector('#ticket-1 [role=combobox]').getAttribute('aria-expanded')==='true' && document.activeElement?.getAttribute('role')==='option'")?;
    b.key("ArrowDown", 40, 0)?;
    b.key("Enter", 13, 0)?;
    b.wait("document.querySelector('#ticket-1')?.closest('section').getAttribute('aria-label')==='In progress' && !document.querySelector('#ticket-1 [role=combobox]').disabled")?;
    b.screenshot(&output.join("first-run-kanban.png"))?;
    b.click("#ticket-2 .card-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Publish release'")?;
    b.click(".label-menu .menu-trigger")?;
    b.wait("document.querySelectorAll('.label-menu .menu-item').length===1 && document.querySelector('.label-menu .menu-item').textContent.includes('waiting-on-calum')")?;
    b.text("#label-filter", "Release")?;
    b.wait("document.querySelector('.label-menu [data-create]')?.textContent.includes('Create label Release')")?;
    b.click(".label-menu [data-create]")?;
    b.wait("document.querySelector('.property-labels .label-chip')?.textContent.includes('Release') && document.querySelector('.menu-trigger').getAttribute('aria-expanded')==='false'")?;
    b.screenshot(&output.join("first-run-label.png"))?;
    b.click(".dependencies .section-heading button")?;
    b.wait("document.querySelector('[aria-label=Direction]')?.value==='Is blocked by'")?;
    b.text("#dependency-search", "Prepare release")?;
    b.wait("document.querySelectorAll('.dependency-options button').length===1")?;
    b.click(".dependency-options button")?;
    b.wait("!document.querySelector('dialog') && document.querySelector('.dependency-row')?.textContent.includes('Blocked by') && document.querySelector('.dependency-title')?.textContent==='Prepare release'")?;
    b.screenshot(&output.join("first-run-dependency.png"))?;
    b.screenshot(&links_output.join(format!("{capture}-desktop.png")))?;
    b.click(".dependency-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Prepare release' && document.querySelector('[aria-label=Status]')?.value==='In progress' && document.querySelector('.dependency-row')?.textContent.includes('Blocks') && document.querySelector('.dependency-title')?.textContent==='Publish release'")?;
    b.click(".dependency-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Publish release' && document.querySelector('.property-labels .label-chip')?.textContent.includes('Release') && document.querySelector('.dependency-row')?.textContent.includes('Blocked by')")?;
    // #55: exercise the responsive layout with a real unresolved dependency.
    b.viewport(390, 844)?;
    b.wait("(()=>{const p=document.querySelector('.properties'),d=document.querySelector('.detail-description'),a=document.querySelector('.activity');return p.getBoundingClientRect().bottom<=d.getBoundingClientRect().top && p.getBoundingClientRect().top<a.getBoundingClientRect().top && !!(p.compareDocumentPosition(a) & Node.DOCUMENT_POSITION_FOLLOWING)})()")?;
    b.wait("(()=>{const r=document.querySelector('.dependency-kind').getBoundingClientRect();return r.width>0 && r.height>0})()")?;
    b.screenshot(&links_output.join(format!("{capture}-mobile.png")))?;
    b.go(&format!("{url}/list?project=1"))?;
    b.wait("(()=>{const row=document.querySelector('#list-ticket-2'),e=row?.querySelector('.row-blocked');if(!e)return false;const r=e.getBoundingClientRect(),p=row.getBoundingClientRect();return r.width>0 && r.height>0 && r.left>=p.left && r.right<=p.right && r.top>=p.top && r.bottom<=p.bottom && getComputedStyle(e).visibility==='visible' && document.documentElement.scrollWidth<=innerWidth})()")?;
    b.click("#list-ticket-2 .row-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Publish release'")?;
    b.viewport(1440, 900)?;
    // Hold the authoritative read after a visible property save until its
    // DetailPage has been disposed. Healthy focus no longer forces a reload.
    b.eval("window.detailFetch=window.fetch;window.fetch=async(input,init)=>{const response=await window.detailFetch(input,init);const url=input instanceof Request?input.url:String(input);if(url.includes('/tickets/2/comments?')){window.detailHeld=true;await new Promise(resolve=>window.releaseDetail=resolve);}return response;};true")?;
    b.wait("!document.querySelector('[aria-label=Priority]').disabled")?;
    b.choose("[aria-label=Priority]", "Medium")?;
    b.wait("window.detailHeld===true")?;
    b.click(".dependency-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Prepare release'")?;
    b.eval("window.fetch=window.detailFetch;window.releaseDetail();true")?;
    b.click(".dependency-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Publish release'")?;
    // Also navigate while a successful save response is still in flight.
    b.wait("!document.querySelector('[aria-label=Priority]').disabled")?;
    b.eval("window.fetch=async(input,init)=>{const response=await window.detailFetch(input,init);const method=input instanceof Request?input.method:init?.method;if(method==='PATCH'){window.saveHeld=true;await new Promise(resolve=>window.releaseSave=resolve);}return response;};true")?;
    b.choose("[aria-label=Priority]", "High")?;
    b.wait("window.saveHeld===true")?;
    b.click(".dependency-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Prepare release'")?;
    b.eval("window.fetch=window.detailFetch;window.releaseSave();true")?;
    b.click(".dependency-title")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Publish release' && document.querySelector('[aria-label=Priority]')?.value==='High'")?;
    if !before {
        b.go(&format!("{url}/t/first-2"))?;
        b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Publish release'")?;
        b.go(&format!("{url}/t/FIRST-999999"))?;
        b.wait(
        "document.querySelector('[role=alert]')?.textContent.toLowerCase().includes('not found')",
    )?;
        // Stop the previous page's live requests before expiring the session.
        b.call("Page.navigate", serde_json::json!({"url":"about:blank"}))?;
        b.wait("location.href==='about:blank'")?;
        b.call("Network.clearBrowserCookies", serde_json::json!({}))?;
        b.go(&format!("{url}/t/FIRST-2"))?;
        b.wait("location.pathname==='/login' && new URLSearchParams(location.search).get('next')==='/t/FIRST-2'")?;
        b.text("#username", "fixture")?;
        b.text("#password", password)?;
        b.click("button[type=submit]")?;
        b.wait("location.pathname==='/t/FIRST-2' && document.querySelector('.ticket-detail h1')?.textContent==='Publish release'")?;
    }
    // Home is the default landing route; all data above was created through the UI.
    b.go(&format!("{url}/"))?;
    b.wait("[...document.querySelectorAll('.home-metric strong')].map(e=>e.textContent).join(',')==='1,0,1'")?;
    b.click("a.home-metric[href*='metric=in_flight']")?;
    b.wait("document.querySelectorAll('.home-cohort .home-ticket-row').length===1 && document.querySelector('.home-cohort').textContent.includes('FIRST-1')")?;
    b.go(&format!("{url}/t/FIRST-1"))?;
    b.wait("!!document.querySelector('[aria-label=Status]')")?;
    b.choose("[aria-label=Status]", "Done")?;
    b.wait("document.querySelector('[aria-label=Status]').value==='Done' && !document.querySelector('[aria-label=Status]').disabled")?;
    b.go(&format!("{url}/"))?;
    b.wait("[...document.querySelectorAll('.home-metric strong')].map(e=>e.textContent).join(',')==='0,0,0'")?;
    b.go(&format!("{url}/analytics"))?;
    b.wait("document.querySelectorAll('.analytics-card').length===2 && !document.querySelector('.home-page[aria-busy=true]')")?;
    b.click(".analytics-card:first-child .chart-data summary")?;
    b.click(".chart-data a[href*='metric=completed'][href*='week=7']")?;
    b.wait("document.querySelectorAll('.home-cohort .home-ticket-row').length===1 && document.querySelector('.home-cohort').textContent.includes('Week of') && document.querySelector('.home-cohort').textContent.includes('FIRST-1')")?;
    println!(
        "PASS first-run: empty install, UI login, project, two tickets, Kanban move, new label and dependency verified on both tickets; Home and Analytics rollup/cohort; six dark 1440×900 screenshots"
    );
    Ok(())
}
