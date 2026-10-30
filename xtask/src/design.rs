//! Local Chromium checks and deterministic screenshots. No browser tooling in the app.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    env, fs,
    net::TcpStream,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(crate) struct Browser {
    _process: crate::chromium::Process,
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    id: u64,
    session: String,
    drag: Option<Value>,
}
impl Browser {
    pub(crate) fn start(root: &Path) -> Result<Self> {
        Self::start_named(root, "design-browser")
    }
    pub(crate) fn start_named(root: &Path, name: &str) -> Result<Self> {
        let chrome = env::var_os("CHROME_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/bin/google-chrome"));
        let (process, socket) = crate::chromium::launch(root, name, &chrome)?;
        let mut browser = Self {
            _process: process,
            socket,
            id: 0,
            session: String::new(),
            drag: None,
        };
        let target = browser.call("Target.createTarget", json!({"url":"about:blank"}))?;
        let session = browser.call(
            "Target.attachToTarget",
            json!({"targetId":target["targetId"], "flatten":true}),
        )?;
        browser.session = session["sessionId"]
            .as_str()
            .ok_or("missing session")?
            .into();
        browser.call("Page.enable", json!({}))?;
        browser.call("Runtime.enable", json!({}))?;
        browser.call("Network.enable", json!({}))?;
        browser.viewport(1440, 900)?;
        browser.call(
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":"prefers-color-scheme","value":"dark"}]}),
        )?;
        Ok(browser)
    }
    pub(crate) fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.id += 1;
        let mut request = json!({"id":self.id,"method":method,"params":params});
        if !self.session.is_empty() {
            request["sessionId"] = self.session.clone().into();
        }
        self.socket
            .send(Message::Text(request.to_string().into()))?;
        loop {
            let message = self.socket.read()?;
            if let Message::Text(text) = message {
                let response: Value = serde_json::from_str(&text)?;
                if response["method"] == "Input.dragIntercepted" {
                    self.drag = Some(response["params"]["data"].clone());
                }
                if response["method"] == "Runtime.exceptionThrown" {
                    return Err(format!("Browser exception: {}", response["params"]).into());
                }
                if response["method"] == "Runtime.consoleAPICalled"
                    && response["params"]["type"] == "error"
                {
                    eprintln!("Browser console error: {}", response["params"]["args"]);
                }
                if response["method"] == "Network.loadingFailed"
                    && response["params"]["canceled"] != true
                {
                    // Transport diagnostics only: never log URLs, bodies or headers.
                    eprintln!(
                        "Browser network failure: {} ({})",
                        response["params"]["errorText"], response["params"]["blockedReason"]
                    );
                }
                if response["id"] == self.id {
                    if !response["error"].is_null() {
                        return Err(format!("{method}: {}", response["error"]).into());
                    }
                    return Ok(response["result"].clone());
                }
            }
        }
    }
    pub(crate) fn eval(&mut self, expression: &str) -> Result<Value> {
        let result = self.call(
            "Runtime.evaluate",
            json!({"expression":expression,"returnByValue":true,"awaitPromise":true}),
        )?;
        if !result["exceptionDetails"].is_null() {
            return Err(format!("{expression}: {}", result["exceptionDetails"]).into());
        }
        Ok(result["result"]["value"].clone())
    }
    pub(crate) fn wait(&mut self, expression: &str) -> Result<()> {
        let start = Instant::now();
        loop {
            match self.eval(expression) {
                Ok(value) if value == true => return Ok(()),
                Ok(_) => {}
                // A hard navigation can replace the document between readiness polls.
                // Retry predicates only; actions and JavaScript exceptions still fail.
                Err(error)
                    if error.to_string().starts_with("Runtime.evaluate:")
                        && [
                            "Inspected target navigated or closed",
                            "Execution context was destroyed",
                            "Cannot find context with specified id",
                        ]
                        .iter()
                        .any(|message| error.to_string().contains(message)) => {}
                Err(error) => return Err(error),
            }
            if start.elapsed() > Duration::from_secs(15) {
                let _ = self.screenshot(Path::new("target/browser-failure.png"));
                let context=self.eval("({url:location.href,title:document.title,body:document.body.innerText.slice(0,1200)})").unwrap_or_default();
                return Err(format!("Timed out: {expression}; {context}").into());
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    pub(crate) fn go(&mut self, url: &str) -> Result<()> {
        // Page.navigate can return before the new document replaces the old one.
        // Never satisfy readiness from the previous route's shell or fixtures.
        self.eval("window.__ticketsHarnessNavigation = true")?;
        let navigation = self.call("Page.navigate", json!({"url":url}))?;
        if navigation["loaderId"].is_string() {
            self.wait("!window.__ticketsHarnessNavigation")?;
        }
        self.wait(
            "!!document.querySelector('.topbar, .auth-page, .login-page, .component-stage')",
        )?;
        self.wait("document.fonts.ready.then(() => true)")?;
        Ok(())
    }
    pub(crate) fn viewport(&mut self, width: u32, height: u32) -> Result<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":height,"deviceScaleFactor":1,"mobile":false}),
        )?;
        Ok(())
    }
    pub(crate) fn screenshot(&mut self, path: &Path) -> Result<()> {
        // Let layout and font metrics settle before capturing the viewport.
        self.eval("new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(true))))")?;
        // Capture finished UI, including the shared enter/exit transitions.
        self.eval("Promise.race([Promise.all(document.getAnimations().filter(a=>a.effect?.getTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{}))),new Promise(r=>setTimeout(r,400))]).then(()=>true)")?;
        // CDP clip coordinates are relative to the page, including scroll.
        let viewport =
            self.eval("({x:scrollX,y:scrollY,width:innerWidth,height:innerHeight,scale:1})")?;
        let result = self.call(
            "Page.captureScreenshot",
            json!({"format":"png","captureBeyondViewport":true,"clip":viewport}),
        )?;
        let png = STANDARD.decode(result["data"].as_str().ok_or("missing screenshot")?)?;
        let width = u32::from_be_bytes(png.get(16..20).ok_or("missing PNG width")?.try_into()?);
        let height = u32::from_be_bytes(png.get(20..24).ok_or("missing PNG height")?.try_into()?);
        if Some(u64::from(width)) != viewport["width"].as_u64()
            || Some(u64::from(height)) != viewport["height"].as_u64()
        {
            return Err(
                format!("screenshot dimensions {width}×{height} differ from {viewport}").into(),
            );
        }
        fs::write(path, png)?;
        println!("Screenshot: {}", path.display());
        Ok(())
    }
    fn full_screenshot(&mut self, path: &Path) -> Result<()> {
        // Reuse the preceding viewport PNG when it already covers the document.
        // Managed headless Chrome can lose cached glyphs on a second raster pass.
        if self.eval("scrollY===0 && document.documentElement.scrollHeight===innerHeight")? == true
        {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("missing PNG name")?;
            fs::copy(path.with_file_name(name.replace("-full.png", ".png")), path)?;
            return Ok(());
        }
        let size = self.eval(
            "({width:innerWidth,height:innerHeight,full:document.documentElement.scrollHeight})",
        )?;
        let width = size["width"].as_u64().ok_or("missing width")? as u32;
        let height = size["height"].as_u64().ok_or("missing height")? as u32;
        self.viewport(
            width,
            size["full"].as_u64().ok_or("missing full height")? as u32,
        )?;
        self.eval("window.scrollTo(0,0);true")?;
        self.screenshot(path)?;
        self.viewport(width, height)?;
        Ok(())
    }
    pub(crate) fn click(&mut self, selector: &str) -> Result<()> {
        let selector = serde_json::to_string(selector)?;
        // An outgoing native dialog intentionally keeps the page inert through exit.
        // Wait for an actual pointer target rather than clicking through that interval.
        self.wait(&format!("(()=>{{const e=document.querySelector({selector}),d=document.querySelector('dialog[open]');if(!e || !e.getClientRects().length || e.closest('[hidden],[inert],[data-state=closed]') || (d && !d.contains(e)))return false;e.scrollIntoView({{block:'nearest'}});const r=e.getBoundingClientRect(),hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return hit===e || e.contains(hit)}})()"))?;
        let point = self.eval(&format!("(() => {{ const e=document.querySelector({selector}); if(!e) throw Error('Missing control'); e.scrollIntoView({{block:'nearest'}}); const r=e.getBoundingClientRect(); return {{x:r.x+r.width/2,y:r.y+r.height/2}}; }})()"))?;
        for kind in ["mousePressed", "mouseReleased"] {
            self.call(
                "Input.dispatchMouseEvent",
                json!({"type":kind,"x":point["x"],"y":point["y"],"button":"left","clickCount":1}),
            )?;
        }
        Ok(())
    }
    pub(crate) fn choose(&mut self, selector: &str, value: &str) -> Result<()> {
        self.click(selector)?;
        let id = self.eval(&format!(
            "document.querySelector({}).getAttribute('aria-controls')",
            serde_json::to_string(selector)?
        ))?;
        let panel = id.as_str().ok_or("select has no listbox")?;
        let option = format!(
            "#{panel} [role=option][data-value={}]",
            serde_json::to_string(value)?
        );
        self.wait(&format!(
            "!!document.querySelector({})",
            serde_json::to_string(&option)?
        ))?;
        self.click(&option)?;
        self.wait(&format!(
            "document.querySelector({})?.getAttribute('aria-expanded')==='false'",
            serde_json::to_string(selector)?
        ))?;
        Ok(())
    }
    pub(crate) fn sign_out(&mut self) -> Result<()> {
        if self.eval("!!document.querySelector('.account-trigger')")? == true {
            if self.eval(
                "document.querySelector('.account-trigger').getBoundingClientRect().height===0",
            )? == true
            {
                self.click("[aria-label='Toggle sidebar']")?;
            }
            self.click(".account-trigger")?;
            self.wait(
                "document.querySelector('.account-trigger').getAttribute('aria-expanded')==='true'",
            )?;
            self.click(".user-menu [role=menuitem]:last-child")?;
        } else {
            self.eval("[...document.querySelectorAll('.sidebar-bottom button')].find(b=>b.textContent.includes('Sign out')).click();true")?;
        }
        Ok(())
    }
    pub(crate) fn text(&mut self, selector: &str, text: &str) -> Result<()> {
        self.click(selector)?;
        self.eval(&format!(
            "document.querySelector({}).select();true",
            serde_json::to_string(selector)?
        ))?;
        self.call("Input.insertText", json!({"text":text}))?;
        Ok(())
    }
    fn drag(&mut self, from: &str, to: &str) -> Result<()> {
        let point = |selector: &str| {
            format!(
                "(()=>{{const e=document.querySelector({});e.scrollIntoView({{block:'nearest'}});const r=e.getBoundingClientRect();return {{x:r.x+r.width/2,y:r.y+24}}}})()",
                serde_json::to_string(selector).unwrap()
            )
        };
        let target = self.eval(&point(to))?;
        let source = self.eval(&point(from))?;
        self.call("Input.setInterceptDrags", json!({"enabled":true}))?;
        self.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":source["x"],"y":source["y"]}),
        )?;
        self.call("Input.dispatchMouseEvent",json!({"type":"mousePressed","x":source["x"],"y":source["y"],"button":"left","clickCount":1}))?;
        for offset in [8., 16., 24.] {
            self.call("Input.dispatchMouseEvent",json!({"type":"mouseMoved","x":source["x"].as_f64().unwrap()+offset,"y":source["y"],"button":"left","buttons":1}))?;
        }
        self.eval("new Promise(r=>setTimeout(()=>r(true),100))")?;
        let data = self
            .drag
            .take()
            .ok_or("Mouse did not start a native drag")?;
        for kind in ["dragEnter", "dragOver", "drop"] {
            self.call(
                "Input.dispatchDragEvent",
                json!({"type":kind,"x":target["x"],"y":target["y"],"data":data}),
            )?;
        }
        self.call("Input.dispatchMouseEvent",json!({"type":"mouseReleased","x":target["x"],"y":target["y"],"button":"left","clickCount":1}))?;
        self.call("Input.setInterceptDrags", json!({"enabled":false}))?;
        Ok(())
    }
    pub(crate) fn key(&mut self, key: &str, code: u32, modifiers: u32) -> Result<()> {
        for kind in ["keyDown", "keyUp"] {
            self.call(
                "Input.dispatchKeyEvent",
                json!({"type":kind,"key":key,"windowsVirtualKeyCode":code,"modifiers":modifiers,"text":if kind=="keyDown" && key=="Enter" {"\r"}else{""}}),
            )?;
        }
        self.eval("new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(true))))")?;
        Ok(())
    }
}

pub fn check(root: &Path) -> super::Result {
    let base = env::var("TICKETS_DESIGN_URL").unwrap_or_else(|_| "http://127.0.0.1:8086".into());
    if !base.starts_with("http://127.0.0.1:") && !base.starts_with("http://localhost:") {
        return Err("design-check only accepts a local server".into());
    }
    let output = env::var_os("TICKETS_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("docs/screenshots/design"));
    fs::create_dir_all(&output)?;
    let mut browser = Browser::start(root)?;
    for (route, name) in [
        ("/design", "01-sidebar-shell"),
        ("/design/board", "02-kanban"),
        ("/design/list", "03-list"),
        ("/design/ticket", "04-ticket-detail"),
        ("/design/empty", "05-empty-states"),
        ("/design/projects", "06-projects"),
    ] {
        browser.go(&format!("{base}{route}"))?;
        browser.wait("document.fonts.check('13px \"Inter Variable\"')")?;
        browser.wait("document.documentElement.scrollWidth <= innerWidth")?;
        browser.screenshot(&output.join(format!("{name}.png")))?;
    }
    browser.go(&format!("{base}/design/board"))?;
    browser
        .eval("document.querySelector('button[role=combobox][aria-label=\"Move TKT-7 to status\"]').focus()")?;
    browser.key("ArrowDown", 40, 0)?;
    browser.key("ArrowDown", 40, 0)?;
    browser.key("Enter", 13, 0)?;
    browser.wait("document.querySelector('.board-column[aria-label=\"In review\"] button[role=combobox][aria-label=\"Move TKT-7 to status\"]')?.value === 'In review'")?;
    browser
        .wait("document.activeElement?.getAttribute('aria-label') === 'Move TKT-7 to status'")?;
    browser.key("ArrowDown", 40, 0)?;
    browser.key("ArrowDown", 40, 0)?;
    browser.key("Enter", 13, 0)?;
    browser.wait("document.querySelector('.board-column[aria-label=\"Done\"] button[role=combobox][aria-label=\"Move TKT-7 to status\"]')?.value === 'Done'")?;
    browser
        .wait("document.activeElement?.getAttribute('aria-label') === 'Move TKT-7 to status'")?;
    browser.eval("(() => { const card=document.querySelector('.ticket-card:has(button[role=combobox][aria-label=\"Move TKT-8 to status\"])'); const target=document.querySelector('.board-column[aria-label=\"In progress\"]'); const dataTransfer=new DataTransfer(); card.dispatchEvent(new DragEvent('dragstart',{bubbles:true,dataTransfer})); target.dispatchEvent(new DragEvent('dragover',{bubbles:true,cancelable:true,dataTransfer})); target.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer})); return true; })()")?;
    browser.wait("!!document.querySelector('.board-column[aria-label=\"In progress\"] button[role=combobox][aria-label=\"Move TKT-8 to status\"]')")?;
    browser.screenshot(&output.join("11-move-feedback.png"))?;
    browser.go(&format!("{base}/design/board"))?;
    browser.text("input[aria-label='Filter tickets']", "keyboard")?;
    browser.wait("document.querySelectorAll('.ticket-card').length === 1")?;
    browser.go(&format!("{base}/design/board"))?;
    browser.key("k", 75, 4)?;
    browser.wait("!!document.querySelector('dialog[open]')")?;
    browser.wait(
        "document.activeElement?.getAttribute('aria-label') === 'Search tickets or commands'",
    )?;
    browser.screenshot(&output.join("07-command-palette.png"))?;
    browser.key("ArrowDown", 40, 0)?;
    browser.wait("document.activeElement?.getAttribute('role')==='combobox' && !!document.querySelector('.command-option[aria-selected=true]')")?;
    browser.key("Escape", 27, 0)?;
    browser.wait("!document.querySelector('dialog')")?;
    browser.key("c", 67, 0)?;
    browser.wait("!!document.querySelector('#ticket-title')")?;
    browser.text("#ticket-title", "Restore saved filters on reload")?;
    browser.screenshot(&output.join("08-new-ticket.png"))?;
    browser.click("dialog button[type='submit']")?;
    browser
        .wait("document.querySelector('h1')?.textContent === 'Restore saved filters on reload'")?;
    browser.click(".label-menu .menu-trigger")?;
    browser.click(".label-menu .menu-item:last-child")?;
    browser.wait("document.querySelector('.property-labels').textContent.includes('Design')")?;
    browser.text(
        "textarea[aria-label='Write a comment']",
        "**Verified** the saved view.",
    )?;
    browser.click(".comment-form button[type='submit']")?;
    browser.wait("document.querySelector('.comment strong')?.textContent === 'Sample User' && document.querySelector('.comment .markdown strong')?.textContent === 'Verified'")?;
    browser.go(&format!("{base}/design/ticket"))?;
    browser.click(".dependencies .button")?;
    browser.text("#dependency-search", "TKT-6")?;
    browser.screenshot(&output.join("09-add-dependency.png"))?;
    browser.click(".dependency-option:not(:disabled)")?;
    browser.wait("document.querySelector('.dependencies').textContent.includes('TKT-6')")?;
    browser.click(".sidebar .nav-item.quiet")?;
    browser.text("#project-title", "Editor")?;
    browser.click("dialog button[type='submit']")?;
    browser.wait("document.querySelector('.breadcrumb-current')?.textContent === 'Editor'")?;
    browser.click(".empty-state .button")?;
    browser.text("#ticket-title", "Save editor layout")?;
    browser.click("dialog button[type='submit']")?;
    browser.wait("document.querySelector('.property-project')?.textContent.includes('Editor')")?;
    for route in [
        "/design/board",
        "/design/list",
        "/design/ticket",
        "/design/empty",
        "/design",
    ] {
        browser.viewport(390, 844)?;
        browser.go(&format!("{base}{route}"))?;
        browser.wait("document.documentElement.scrollWidth <= innerWidth")?;
    }
    browser.screenshot(&output.join("10-mobile-gallery.png"))?;
    browser.viewport(1440, 900)?;
    browser.go(&format!("{base}/login"))?;
    browser.wait("!!document.querySelector('#username') && !!document.querySelector('#password[type=password]') && !document.querySelector('.preview-note')")?;
    browser.screenshot(&output.join("12-login.png"))?;
    let worker_api = browser.eval("fetch('/api/health').then(r => r.headers.get('content-type')?.includes('application/json')).catch(() => false)")? == true;
    browser.go(&format!("{base}/settings/tokens"))?;
    if worker_api {
        browser.wait("location.pathname==='/login' && !!document.querySelector('#username')")?;
        println!("PASS: the local Worker requires authentication for API tokens.");
    } else {
        browser.wait("!!document.querySelector('.token-inventory')")?;
    }
    browser.screenshot(&output.join("13-token-access.png"))?;
    browser.go(&format!("{base}/design"))?;
    browser.click("a[href='/design/c/select']")?;
    browser.wait(
        "!!document.querySelector('.component-stage') && !document.querySelector('.sidebar')",
    )?;
    println!(
        "PASS: routes, fonts, overflow at 1440/390px, keyboard status move with focus retained, drag/drop events, filtering, Cmd+K with search focus, arrow navigation, Escape, C, creation, labels, sanitized comments, dependencies, project creation and project selection, existing login and token screens."
    );
    Ok(())
}

/// Capture the actual root route before replacing the fixture-backed implementation.
pub fn board_before(root: &Path) -> super::Result {
    let mut browser = Browser::start(root)?;
    browser.go("http://127.0.0.1:8787/")?;
    browser.wait("document.querySelectorAll('.ticket-card').length > 0")?;
    let output = root.join("docs/screenshots/kanban");
    fs::create_dir_all(&output)?;
    browser.screenshot(&output.join("before-desktop.png"))?;
    browser.viewport(390, 900)?;
    browser.screenshot(&output.join("before-mobile.png"))?;
    Ok(())
}

/// Real API, real Worker, real keyboard events; isolated D1 lifecycle is owned by api_smoke.
pub fn board_check(root: &Path, password: &str, mode: &str) -> super::Result {
    let mut b = Browser::start(root)?;
    if mode == "detail"
        && (env::var_os("TICKETS_BEFORE_ASSETS").is_some()
            || env::var_os("TICKETS_DETAIL_CAPTURE_ONLY").is_some())
    {
        // Capture settled text in the supported reduced-motion mode. Functional
        // detail checks keep normal motion and verify comment reveal behavior.
        b.call("Emulation.setEmulatedMedia", json!({"features":[{"name":"prefers-color-scheme","value":"dark"},{"name":"prefers-reduced-motion","value":"reduce"}]}))?;
    }
    b.go("http://127.0.0.1:8795/login?next=%2Fsettings%2Ftokens")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("['/settings/tokens','/settings/api-tokens'].includes(location.pathname)")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("document.body.innerText.includes('No projects yet')")?;
    b.eval(r#"(async()=>{
        const api=async(path,body,method='POST')=>{const r=await fetch('/api/v1/'+path,{method,headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error('Seed request failed');return r.json()};
        await api('projects',{name:'Tickets',key_prefix:'TKT',color:'#7170ff'});
        await api('projects',{name:'Quantum',key_prefix:'QUA',color:'#4ea7fc'});
        await api('projects',{name:'Empty project',key_prefix:'EMP',color:'#7170ff'});
        await api('labels',{name:'Feature',color:'#7170ff'});
        const statuses=['backlog','todo','in_progress','in_review','done','canceled'];
        const titles=['Remember filters when switching views','Group the ticket list by status','Move tickets between board columns','Review imported labels','Set up the Rust workspace','Retire the old importer'];
        for(let i=0;i<6;i++){
            await api('tickets',{project_id:1,title:titles[i],status:statuses[i],priority:2,assignee:i%2?'agent':null,sort_order:1024});
            await api('tickets/'+(i+1)+'/labels/1',{});
        }
        await api('tickets',{project_id:1,title:'Add a keyboard shortcut reference',status:'todo',priority:4,sort_order:2048});
        await api('tickets/1/dependencies/2',{});
        await api('tickets',{project_id:2,title:'Quantum board stays separate',status:'todo'});
        // Cross a page boundary; the board must fetch every page, including resolved blockers.
        for(let i=0;i<94;i++)await api('tickets',{project_id:2,title:'Pagination ticket '+i});
        return true;
    })()"#)?;
    b.eval("document.querySelector('[role=alert]')?.textContent || true")?;
    if mode == "detail" {
        return detail_check(root, &mut b);
    }
    if mode == "list" {
        return list_check(root, &mut b);
    }
    b.go("http://127.0.0.1:8795/board")?;
    b.wait("document.querySelectorAll('.ticket-card').length===6")?;
    // A completed/canceled drag must never turn its trailing click into navigation.
    b.eval("(()=>{const c=document.querySelector('#ticket-1');c.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true}));c.dispatchEvent(new DragEvent('dragstart',{bubbles:true,dataTransfer:new DataTransfer()}));c.dispatchEvent(new DragEvent('dragend',{bubbles:true}));c.click();return true})()")?;
    b.wait("location.pathname==='/board' && document.querySelector('.board').getAttribute('aria-busy')!=='true'")?;
    // The body, not only the title link, opens the ticket; Enter does too.
    b.click("#ticket-1 .ticket-id")?;
    b.wait("location.pathname==='/t/TKT-1' && !!document.querySelector('.ticket-detail')")?;
    b.go("http://127.0.0.1:8795/board")?;
    b.wait("!!document.querySelector('#ticket-1')")?;
    b.eval("document.querySelector('#ticket-1').focus();true")?;
    b.key("Enter", 13, 0)?;
    b.wait("location.pathname==='/t/TKT-1'")?;
    b.go("http://127.0.0.1:8795/board")?;
    b.wait("document.querySelectorAll('.ticket-card').length===6")?;
    b.click(".canceled-toggle input")?;
    b.wait("location.search.includes('canceled=true') && document.querySelectorAll('.ticket-card').length===7")?;
    b.wait("document.querySelectorAll('.board-column').length===6 && document.body.innerText.includes('Blocked by TKT-1')")?;
    b.eval("document.querySelector('#ticket-1').focus()")?;
    b.key("ArrowRight", 39, 0)?;
    b.wait("document.activeElement.id==='ticket-2'")?;
    b.eval("document.querySelector('#ticket-2').focus()")?;
    b.key(" ", 32, 0)?;
    b.key("ArrowRight", 39, 0)?;
    b.key(" ", 32, 0)?;
    b.wait("document.body.innerText.includes('Order saved') && document.querySelector('.board').getAttribute('aria-busy')!=='true'")?;
    b.wait("document.querySelector('#ticket-2').closest('section').getAttribute('aria-label')==='In progress' && document.activeElement.id==='ticket-2'")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("document.querySelector('#ticket-2')?.closest('section').getAttribute('aria-label')==='In progress'")?;
    b.eval("document.querySelector('#ticket-2').focus()")?;
    b.key(" ", 32, 0)?;
    b.key("ArrowUp", 38, 0)?;
    b.key(" ", 32, 0)?;
    b.wait("document.body.innerText.includes('Order saved')")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("document.querySelector('[aria-label=\"In progress\"] .ticket-card')?.id==='ticket-2'")?;
    // Escape restores the draft, never issuing a write.
    b.eval("document.querySelector('#ticket-2').focus()")?;
    b.key(" ", 32, 0)?;
    b.key("ArrowLeft", 37, 0)?;
    b.key("Escape", 27, 0)?;
    b.wait("document.querySelector('#ticket-2')?.closest('section').getAttribute('aria-label')==='In progress'")?;
    // Actual HTML drag event handlers, including a within-column insertion target.
    b.drag("#ticket-7", "#ticket-2")?;
    b.wait("document.body.innerText.includes('Order saved') && document.querySelector('[aria-label=\"In progress\"] .ticket-card')?.id==='ticket-7'")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("document.querySelector('[aria-label=\"In progress\"] .ticket-card')?.id==='ticket-7'")?;
    b.drag("#ticket-2", "#ticket-7")?;
    b.wait("document.body.innerText.includes('Order saved') && document.querySelector('[aria-label=\"In progress\"] .ticket-card')?.id==='ticket-2'")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("document.querySelector('[aria-label=\"In progress\"] .ticket-card')?.id==='ticket-2'")?;
    b.wait("location.pathname==='/board'")?;
    // Shared status select supplies the touch / keyboard alternative.
    b.choose("#ticket-7 [role=combobox]", "Todo")?;
    b.wait("document.querySelector('#ticket-7')?.closest('section').getAttribute('aria-label')==='Todo' && document.body.innerText.includes('Order saved')")?;
    b.wait("location.pathname==='/board'")?;
    b.choose(r#"[aria-label="Filter by label"]"#, "1")?;
    b.wait("document.querySelectorAll('.ticket-card').length===6")?;
    b.choose(r#"[aria-label="Filter by assignee"]"#, "agent")?;
    b.wait("document.querySelectorAll('.ticket-card').length===3")?;
    b.text("input[aria-label='Filter tickets']", "Review imported")?;
    b.wait("document.querySelectorAll('.ticket-card').length===1 && !!document.querySelector('#ticket-4')")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("document.querySelectorAll('.ticket-card').length===7")?;
    // Reject a move at the API boundary, then verify optimistic rollback and focus.
    b.eval("window.boardFetch=window.fetch;window.fetch=(input,init)=>((input instanceof Request?input.method:init?.method)==='PATCH'?Promise.resolve(new Response(JSON.stringify({error:{message:'Verification rejection'}}),{status:409,headers:{'Content-Type':'application/json'}})):window.boardFetch(input,init));document.querySelector('#ticket-2').focus();true")?;
    b.key(" ", 32, 0)?;
    b.key("ArrowRight", 39, 0)?;
    b.key(" ", 32, 0)?;
    b.wait("document.body.innerText.includes('Previous order restored') && document.querySelector('#ticket-2')?.closest('section').getAttribute('aria-label')==='In progress' && document.activeElement.id==='ticket-2'")?;
    b.eval("window.fetch=window.boardFetch;true")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("document.querySelectorAll('.ticket-card').length===7")?;
    // Reject the second write of a reorder, exercising compensation of a saved rank.
    b.eval("window.boardFetch=window.fetch;window.patchCount=0;window.fetch=(input,init)=>((input instanceof Request?input.method:init?.method)==='PATCH' && ++window.patchCount===2?Promise.resolve(new Response(JSON.stringify({error:{message:'Second-write rejection'}}),{status:409,headers:{'Content-Type':'application/json'}})):window.boardFetch(input,init));document.querySelector('#ticket-2').focus();true")?;
    b.key(" ", 32, 0)?;
    b.key("ArrowDown", 40, 0)?;
    b.key(" ", 32, 0)?;
    b.wait("document.body.innerText.includes('Previous order restored') && window.patchCount===3 && document.activeElement.id==='ticket-2'")?;
    b.eval("window.fetch=window.boardFetch;true")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("document.querySelector('[aria-label=\"In progress\"] .ticket-card')?.id==='ticket-2'")?;
    b.eval("document.querySelector('[aria-label=\"Add ticket to In review\"]').focus();true")?;
    b.key("c", 67, 0)?;
    b.wait("!!document.querySelector('#board-title') && document.querySelectorAll('dialog').length===1 && document.querySelector('[aria-label=\"New ticket status\"]').value==='Todo'")?;
    b.key("Escape", 27, 0)?;
    b.click("[aria-label='Add ticket to Todo']")?;
    b.text("#board-title", "Created from the board")?;
    b.click("dialog button[type=submit]")?;
    b.wait(
        "document.querySelectorAll('.ticket-card').length===8 && !document.querySelector('dialog')",
    )?;
    b.click(".canceled-toggle input")?;
    b.wait("!location.search.includes('canceled') && document.querySelectorAll('.board-column').length===5 && document.querySelector('.board').scrollWidth>document.querySelector('.board').clientWidth && document.querySelector('.sidebar').getBoundingClientRect().width===240 && document.documentElement.scrollWidth<=innerWidth")?;
    b.wait("document.querySelector('.view-footer').textContent.includes('7 tickets') && document.querySelector('.toast').textContent.includes('Ticket created')")?;
    b.key("k", 75, 4)?;
    b.wait("!!document.querySelector('dialog[aria-label=\"Search and commands\"]')")?;
    b.key("Escape", 27, 0)?;
    b.click(".search-trigger")?;
    b.text(
        "input[aria-label='Search tickets or commands']",
        "Review imported",
    )?;
    b.wait("document.querySelectorAll('.command-results .menu-item').length===1 && document.querySelector('.command-results').textContent.includes('TKT-4')")?;
    b.key("Escape", 27, 0)?;
    b.wait("!document.querySelector('dialog') && !!document.querySelector('.user-menu .account-trigger')")?;
    b.click(".toast button")?;
    let output = root.join("docs/screenshots/kanban");
    fs::create_dir_all(&output)?;
    b.screenshot(&output.join("after-desktop.png"))?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth <= innerWidth")?;
    b.screenshot(&output.join("after-mobile.png"))?;
    // Project switching must clear filters and never leak tickets from another project.
    b.eval("document.querySelectorAll('.project-nav')[1].click();true")?;
    b.wait("document.querySelectorAll('.ticket-card').length===95 && !document.body.innerText.includes('Created from the board')")?;
    b.eval("document.querySelectorAll('.project-nav')[2].click();true")?;
    b.wait("document.querySelectorAll('.ticket-card').length===0 && document.querySelectorAll('.board-column').length===4 && !document.querySelector('.add-card').disabled && document.body.innerText.includes('No tickets yet')")?;
    check_composer(&mut b)?;
    for route in ["list?project=1", "t/TKT-1", "home"] {
        b.go(&format!("http://127.0.0.1:8795/{route}"))?;
        b.wait("document.querySelector('.topbar .button-primary')?.textContent.includes('New ticket') && !document.querySelector('.topbar .button-primary').disabled")?;
        b.click(".topbar .button-primary")?;
        b.wait("document.querySelector('[aria-label=\"New ticket status\"]')?.value==='Todo'")?;
        b.key("Escape", 27, 0)?;
        b.wait("!document.querySelector('dialog')")?;
    }
    b.sign_out()?;
    b.wait("location.pathname==='/login'")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("location.pathname==='/login' && !document.querySelector('.sidebar, .ticket-card')")?;
    println!(
        "PASS: local API seed; populated review column + URL canceled toggle; 240px sidebar and contained board scrolling; palette shortcut/search/Escape; sidebar logout; card metadata; >100-ticket pagination; project isolation; keyboard move/reorder/cancel/focus; drag insertion; status select; label+assignee filters; rejected-write rollback; composer; desktop/mobile screenshots"
    );
    Ok(())
}

fn list_check(root: &Path, b: &mut Browser) -> super::Result {
    let output = root.join("docs/screenshots/list");
    fs::create_dir_all(&output)?;
    b.go("http://127.0.0.1:8795/list")?;
    if env::var_os("TICKETS_LIST_BEFORE").is_some() {
        b.wait("document.body.innerText.includes('Page not found')")?;
        b.screenshot(&output.join("before-desktop.png"))?;
        b.viewport(390, 900)?;
        b.screenshot(&output.join("before-mobile.png"))?;
        return Ok(());
    }
    b.wait("document.querySelectorAll('.api-list-row').length===101")?;
    b.go("http://127.0.0.1:8795/list?project=1")?;
    b.wait("document.querySelectorAll('.api-list-row').length===6 && document.querySelectorAll('.list-group').length===5")?;
    b.wait("document.querySelectorAll('#list-ticket-1 .row-labels > .label-chip').length===1 && !document.querySelector('#list-ticket-1 button.row-label-overflow')")?;
    b.click(".canceled-toggle input")?;
    b.wait("document.querySelectorAll('.api-list-row').length===7 && document.querySelectorAll('.list-group').length===6")?;
    b.click(".canceled-toggle input")?;
    b.wait("document.querySelectorAll('.api-list-row').length===6 && !document.querySelector('[aria-label=Canceled]')")?;
    b.eval("document.querySelector('#list-ticket-1').focus();true")?;
    b.key("j", 74, 0)?;
    b.wait("document.activeElement.id==='list-ticket-2'")?;
    b.key("k", 75, 0)?;
    b.wait("document.activeElement.id==='list-ticket-1'")?;
    b.key("Enter", 13, 0)?;
    b.wait("location.pathname==='/t/TKT-1' && document.querySelector('.ticket-detail h1')?.textContent.includes('Remember filters')")?;
    b.go("http://127.0.0.1:8795/list?project=1")?;
    b.wait("!!document.querySelector('#list-ticket-1')")?;
    b.click("#list-ticket-1 .ticket-id")?;
    b.wait("location.pathname==='/t/TKT-1' && !!document.querySelector('.ticket-detail')")?;
    b.go("http://127.0.0.1:8795/list?project=1")?;
    b.wait("!!document.querySelector('#list-ticket-1')")?;
    b.choose(r#"[aria-label="Status for TKT-1"]"#, "Todo")?;
    b.wait("document.querySelector('#list-ticket-1')?.closest('section').getAttribute('aria-label')==='Todo' && document.querySelector('.ticket-list').getAttribute('aria-busy')==='false'")?;
    b.wait("location.pathname==='/list'")?;
    b.choose(r#"[aria-label="Priority for TKT-1"]"#, "Urgent")?;
    b.wait("document.querySelector('[aria-label=\"Priority for TKT-1\"]').value==='Urgent' && document.querySelector('.ticket-list').getAttribute('aria-busy')==='false'")?;
    b.go("http://127.0.0.1:8795/list?project=1&status=Todo&sort=priority")?;
    b.wait("document.querySelectorAll('.api-list-row').length===3 && document.querySelector('.api-list-row').id==='list-ticket-1'")?;
    b.choose(r#"[aria-label="All labels"]"#, "1")?;
    b.wait("location.search.includes('label=1') && document.querySelectorAll('.api-list-row').length===2")?;
    b.choose(r#"[aria-label="All assignees"]"#, "agent")?;
    b.wait("location.search.includes('assignee=agent') && document.querySelectorAll('.api-list-row').length===1")?;
    b.go("http://127.0.0.1:8795/list?project=1&status=Todo&sort=priority&label=1&assignee=agent")?;
    b.wait("document.querySelectorAll('.api-list-row').length===1 && !!document.querySelector('#list-ticket-2')")?;
    for sort in ["updated", "created"] {
        b.go(&format!("http://127.0.0.1:8795/list?project=1&sort={sort}"))?;
        b.wait("document.querySelectorAll('.api-list-row').length===6")?;
        let expected = if sort == "updated" { 1 } else { 7 };
        b.wait(&format!("document.querySelector('[aria-label=Todo] .api-list-row').id==='list-ticket-{expected}'"))?;
    }
    b.eval("window.savedFetch=window.fetch;window.fetch=(i,o)=>((i instanceof Request?i.method:o?.method)==='PATCH'?Promise.resolve(new Response(JSON.stringify({error:{message:'Verification rejection'}}),{status:409,headers:{'Content-Type':'application/json'}})):window.savedFetch(i,o));true")?;
    b.choose(r#"[aria-label="Status for TKT-2"]"#, "Done")?;
    b.wait("document.querySelector('[role=alert]')?.textContent.includes('Verification rejection') && document.querySelector('[aria-label=\"Status for TKT-2\"]').value==='Todo'")?;
    b.eval("window.fetch=window.savedFetch;true")?;
    b.go("http://127.0.0.1:8795/list?project=1")?;
    b.wait("document.querySelectorAll('.api-list-row').length===6")?;
    b.wait("document.querySelector('[aria-label=\"All projects\"]').value==='1'")?;
    b.screenshot(&output.join("after-desktop.png"))?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join("after-mobile.png"))?;
    b.go("http://127.0.0.1:8795/list?project=1&q=keyboard&assignee=__unassigned")?;
    b.wait("document.querySelectorAll('.api-list-row').length===1 && !!document.querySelector('#list-ticket-7')")?;
    b.key("k", 75, 4)?;
    b.text(
        "input[aria-label='Search tickets or commands']",
        "Review imported",
    )?;
    b.wait("document.querySelectorAll('.command-results .menu-item').length===1")?;
    b.click(".command-results .menu-item")?;
    b.wait("location.pathname==='/t/TKT-4' && document.querySelector('.ticket-detail h1')?.textContent==='Review imported labels'")?;
    b.go("http://127.0.0.1:8795/list?project=3")?;
    b.wait("document.body.innerText.includes('No tickets match') && document.querySelectorAll('.api-list-row').length===0")?;
    b.key("c", 67, 0)?;
    b.wait("!!document.querySelector('#board-title')")?;
    b.text("#board-title", "Created from the list")?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog') && document.querySelectorAll('.api-list-row').length===1 && document.querySelector('.api-list-row').textContent.includes('EMP-1')")?;
    b.go("http://127.0.0.1:8795/list?project=3")?;
    b.wait("document.querySelectorAll('.api-list-row').length===1 && document.body.innerText.includes('Created from the list')")?;
    // Regression: real chip widths, overflow names, resize stability and a label
    // wider than the whole track. This state is isolated local test data.
    b.eval(r#"(async()=>{
        await fetch('/api/v1/tickets/2/labels/1',{method:'DELETE'});
        for(const [name,ticket] of [['area:ui',1],['model:sol',1],['model:astra',1],['A label deliberately longer than the entire labels column',2]]) {
            const response=await fetch('/api/v1/labels',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name,color:'#5e6ad2'})});
            if(!response.ok) throw new Error('Cannot seed list label');
            const label=await response.json();
            const attached=await fetch(`/api/v1/tickets/${ticket}/labels/${label.id}`,{method:'POST'});
            if(!attached.ok) throw new Error('Cannot attach list label');
        }
        return true;
    })()"#)?;
    b.go("http://127.0.0.1:8795/list?project=1")?;
    for width in [1280, 1100, 1280] {
        b.viewport(width, 900)?;
        b.wait("document.querySelector('#list-ticket-1 .row-label-overflow')!==null && document.querySelector('#list-ticket-2 .row-label-overflow')!==null")?;
        b.wait(r#"Array.from(document.querySelectorAll('.row-labels')).every(cell=>{
            const rect=cell.getBoundingClientRect();
            const chips=Array.from(cell.querySelectorAll(':scope > .label-chip'));
            const overflow=cell.querySelector(':scope > .tooltip-trigger button');
            const all=Array.from(cell.querySelectorAll('.row-label-measure .label-chip'));
            return rect.width>=150 && [...chips,...(overflow?[overflow]:[])].every(chip=>chip.getBoundingClientRect().right<=rect.right+0.5)
                && chips.length+(overflow?Number(overflow.textContent.slice(1)):0)===all.length;
        })"#)?;
        b.eval("document.querySelector('#list-ticket-1 button.row-label-overflow').focus();true")?;
        b.wait("Array.from(document.querySelectorAll('[role=tooltip]')).some(t=>!t.hidden && t.textContent.includes('model:sol') && t.textContent.includes('model:astra'))")?;
        b.key("Escape", 27, 0)?;
        b.eval("document.activeElement.blur();true")?;
        b.wait("document.querySelector('#list-ticket-1 .row-labels > .label-chip')!==null && document.querySelector('#list-ticket-2 .row-labels > .label-chip')===null")?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    }
    println!(
        "PASS list: pagination, grouping, combined URL filters/reload, sorting, inline persistence, rejected-save restoration, j/k/Enter detail navigation, desktop/mobile, whole label chips and keyboard overflow tooltip at 1280/1100px"
    );
    Ok(())
}

/// Issue #9: browser-level component safety and persistent editing against local D1.
fn detail_check(root: &Path, b: &mut Browser) -> super::Result {
    let output = root.join("docs/screenshots/TIK-60");
    fs::create_dir_all(&output)?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("document.querySelector('.ticket-detail h1')?.textContent==='Group the ticket list by status' && !!document.querySelector('[role=listbox][aria-label=Project] [role=option]')")?;
    // Same fixture and saved changes for both real asset versions.
    b.eval("(async()=>{const t=await fetch('/api/v1/tickets/2').then(r=>r.json());for(const patch of [{priority:1,status:'in_progress'}, {priority:t.priority,status:t.status}]) {const r=await fetch('/api/v1/tickets/2',{method:'PATCH',headers:{'Content-Type':'application/json'},body:JSON.stringify(patch)});if(!r.ok)throw Error('history fixture failed');}return true})()")?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("!!document.querySelector('.activity') && document.querySelector('.detail-layout')?.getAttribute('aria-busy')==='false'")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    let before = env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let prefix = if before { "before" } else { "after" };
    if !before {
        b.wait("document.querySelectorAll('.history-event').length>=5")?;
    }
    b.wait("!document.querySelector('[data-ar-t]')")?;
    b.screenshot(&output.join(format!("{prefix}-desktop.png")))?;
    b.full_screenshot(&output.join(format!("{prefix}-desktop-full.png")))?;
    b.eval("document.querySelector('.activity').scrollIntoView({block:'center'});true")?;
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1440,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    super::component::crop(
        b,
        ".activity",
        &output.join(format!("{prefix}-activity.png")),
    )?;
    b.viewport(390, 900)?;
    b.eval("document.querySelector('.ticket-detail h1').scrollIntoView({block:'start'});true")?;
    b.screenshot(&output.join(format!("{prefix}-mobile.png")))?;
    b.full_screenshot(&output.join(format!("{prefix}-mobile-full.png")))?;
    b.eval("document.querySelector('.activity').scrollIntoView({block:'start'});true")?;
    b.screenshot(&output.join(format!("{prefix}-mobile-activity.png")))?;
    if before || env::var_os("TICKETS_DETAIL_CAPTURE_ONLY").is_some() {
        return Ok(());
    }
    b.viewport(1440, 900)?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("document.querySelector('.detail-layout')?.getAttribute('aria-busy')==='false'")?;
    click_text(b, "button", "Edit ticket")?;
    b.wait("!!document.querySelector('#detail-title')")?;
    b.text("#detail-title", "Make every status change accessible")?;
    let markdown = "Keep status changes **accessible** from every view.\n\n### Acceptance\n- [x] Keyboard focus stays visible\n- [ ] Announce the saved status\n\n| Input | Action |\n| --- | --- |\n| Space | Pick up |\n\n```rust\nlet accessible = true;\n```";
    b.text("textarea[aria-label=Description]", markdown)?;
    click_text(b, "dialog button", "Preview")?;
    b.wait("!!document.querySelector('dialog .markdown table') && !!document.querySelector('dialog .markdown input[disabled]') && !!document.querySelector('dialog .markdown pre code')")?;
    click_text(b, "dialog button", "Save ticket")?;
    b.wait("!document.querySelector('dialog') && document.querySelector('.ticket-detail h1')?.textContent==='Make every status change accessible' && document.querySelector('.detail-layout')?.getAttribute('aria-busy')==='false'")?;
    select_value(b, "Status", "In progress")?;
    b.wait("document.querySelector('[aria-label=Status]').value==='In progress' && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    select_value(b, "Priority", "Urgent")?;
    b.wait("document.querySelector('[aria-label=Priority]').value==='Urgent' && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.text("input[aria-label=Assignee]", "fixture-agent")?;
    b.eval("document.querySelector('.assignee-editor').requestSubmit()")?;
    b.wait("document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.click(".label-menu .menu-trigger")?;
    b.click(".label-menu .menu-item")?;
    b.wait("!document.querySelector('.property-labels .label-chip') && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.click(".label-menu .menu-trigger")?;
    b.click(".label-menu .menu-item")?;
    b.wait("!!document.querySelector('.property-labels .label-chip') && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    // Existing 1 -> 2; 2 -> 1 must report the real API's cycle rejection inline.
    click_text(b, "button", "Add dependency")?;
    select_value(b, "Direction", "Blocks")?;
    b.text("#dependency-search", "TKT-1 ")?;
    b.click(".dependency-options button")?;
    b.wait("document.querySelector('dialog [role=alert]')?.textContent.toLowerCase().includes('cycle') && !document.querySelector('dialog button[disabled]')")?;
    b.eval("document.querySelector('[aria-label=\"Dismiss notification\"]')?.click();true")?;
    b.screenshot(&output.join("cycle-error.png"))?;
    b.text("#dependency-search", "TKT-3 ")?;
    b.click(".dependency-options button")?;
    b.wait("!document.querySelector('dialog') && document.querySelectorAll('.dependency-row').length===2 && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    // Component-level XSS tests exercise live preview AND persisted comment rendering.
    for payload in [
        "<script>window.xss=1</script>",
        "<img src=x onerror='window.xss=1'>",
        "[bad](javascript:alert%281%29)",
        "<div onclick='window.xss=1'>raw html</div>",
    ] {
        b.text("textarea[aria-label='Write a comment']", payload)?;
        click_text(b, ".comment-form button", "Preview")?;
        b.wait("!!document.querySelector('.markdown-preview') && !document.querySelector('.markdown-preview script,.markdown-preview [onerror],.markdown-preview [onclick],.markdown-preview a[href^=\"javascript:\"]') && !window.xss")?;
        click_text(b, ".comment-form button", "Comment")?;
        b.wait("document.querySelector('.detail-layout').getAttribute('aria-busy')==='false' && !!document.querySelector('.comment')")?;
        b.wait("!document.querySelector('.comment script,.comment [onerror],.comment [onclick],.comment a[href^=\"javascript:\"]') && !window.xss")?;
        click_text(b, ".comment button", "Delete")?;
        b.wait("!document.querySelector('.comment') && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
        click_text(b, ".comment-form button", "Write")?;
    }
    // New/edited comment bodies decode without replaying the ticket heading.
    b.wait("!document.querySelector('[data-ar-t]')")?;
    b.eval("window.commentDecoded=false;window.headingDecoded=false;window.commentObserver=new MutationObserver(()=>{if(document.querySelector('.comment .markdown [data-ar-t]'))window.commentDecoded=true;if(document.querySelector('.ticket-detail h1[data-ar-t],.ticket-detail h1 [data-ar-t]'))window.headingDecoded=true;});window.commentObserver.observe(document,{subtree:true,childList:true,attributes:true,attributeFilter:['data-ar-t']});true")?;
    b.text("textarea[aria-label='Write a comment']","The status menu should work everywhere the ticket appears. **Keyboard focus** stays visible.")?;
    click_text(b, ".comment-form button", "Comment")?;
    b.wait("!!document.querySelector('.comment strong') && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.wait(
        "window.commentDecoded && !window.headingDecoded && !document.querySelector('[data-ar-t]')",
    )?;
    b.eval("window.commentDecoded=false;true")?;
    click_text(b, ".comment button", "Edit")?;
    b.text(
        ".comment textarea",
        "Reviewed: **keyboard focus** stays visible after a move.",
    )?;
    click_text(b, ".comment button", "Preview")?;
    b.wait("document.querySelector('.comment .markdown-preview strong')?.textContent==='keyboard focus'")?;
    click_text(b, ".comment button", "Save comment")?;
    b.wait("document.querySelector('.comment .markdown')?.textContent.includes('Reviewed:') && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.wait(
        "window.commentDecoded && !window.headingDecoded && !document.querySelector('[data-ar-t]')",
    )?;
    b.eval("window.commentObserver.disconnect();true")?;
    select_value(b, "Project", "2")?;
    b.wait("document.querySelector('.detail-overline .mono')?.textContent==='QUA-96' && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("document.querySelector('.detail-overline .mono')?.textContent==='QUA-96' && document.querySelector('[aria-label=Priority]')?.value==='Urgent' && document.querySelector('input[aria-label=Assignee]')?.value==='fixture-agent' && document.querySelector('.comment .markdown')?.textContent.includes('Reviewed:')")?;
    select_value(b, "Project", "1")?;
    b.wait("document.querySelector('.detail-overline .mono')?.textContent==='TKT-8' && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.wait("document.querySelector('.toast')?.textContent.includes('Saved')")?;
    b.click("[aria-label='Dismiss notification']")?;
    b.wait("!document.querySelector('[data-ar-t]')")?;
    b.screenshot(&output.join("verified-desktop.png"))?;
    b.viewport(390, 900)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join("verified-mobile.png"))?;
    b.eval("document.querySelector('.properties').scrollIntoView();true")?;
    b.click(".properties-toggle")?;
    b.screenshot(&output.join("after-mobile-properties.png"))?;
    b.viewport(1440, 900)?;
    b.go("http://127.0.0.1:8795/list?project=1")?;
    b.wait("!!document.querySelector('#list-ticket-2')")?;
    b.eval("document.querySelector('#list-ticket-2').focus();true")?;
    b.key("Enter", 13, 0)?;
    b.wait("location.pathname==='/t/TKT-8' && !!document.querySelector('.ticket-detail h1')")?;
    b.wait("!!document.querySelector('.topbar .button-primary:not([disabled])') && document.querySelector('.detail-layout')?.getAttribute('aria-busy')==='false'")?;
    b.key("c", 67, 0)?;
    b.wait("!!document.querySelector('#board-title')")?;
    b.text("#board-title", "Created from detail")?;
    select_value(b, "New ticket project", "2")?;
    b.screenshot(&output.join("create-dialog.png"))?;
    b.click("dialog button[type=submit]")?;
    b.wait("!document.querySelector('dialog')")?;
    b.wait("fetch('/api/v1/tickets?project=2&limit=100').then(r=>r.json()).then(p=>p.data.some(t=>t.title==='Created from detail' && t.status==='todo'))")?;
    // Failed property saves restore the saved selection; failed comments preserve the draft.
    b.eval("window.savedFetch=window.fetch;window.fetch=(i,o)=>((i instanceof Request?i.method:o?.method)==='PATCH'?Promise.resolve(new Response(JSON.stringify({error:{message:'Verification rejection'}}),{status:409,headers:{'Content-Type':'application/json'}})):window.savedFetch(i,o));true")?;
    select_value(b, "Status", "Done")?;
    b.wait("document.querySelector('[role=alert]')?.textContent.includes('Verification rejection') && document.querySelector('[aria-label=Status]').value==='In progress'")?;
    b.eval("window.fetch=window.savedFetch;true")?;
    b.wait("fetch('/api/v1/tickets/2/history?limit=100').then(r=>r.json()).then(p=>['created','title','description','status','priority','assignee','label_added','label_removed','blocks_added','project'].every(f=>p.data.some(e=>e.field===f)) && p.data.some(e=>e.actor_kind==='user') && p.data.filter(e=>e.actor_kind==='user').every(e=>e.actor_label==='Owner') && !p.data.some(e=>e.field==='status' && e.new_value==='done'))")?;
    b.eval(r#"(async()=>{const t=await fetch('/api/v1/tickets/2').then(r=>r.json());for(const body of ["<script>window.historyXss=1</script><img src=x onerror='window.historyXss=1'>",t.body]) {const r=await fetch('/api/v1/tickets/2',{method:'PATCH',headers:{'Content-Type':'application/json'},body:JSON.stringify({body})});if(!r.ok)throw Error('history XSS seed');}return true})()"#)?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("!!document.querySelector('.history-diff summary') && document.querySelector('.detail-layout')?.getAttribute('aria-busy')==='false'")?;
    b.click(".history-diff summary")?;
    b.wait("document.querySelector('.history-diff[open] pre')?.textContent.includes('<script>') && !window.historyXss")?;
    b.wait("!!document.querySelector('.history-diff[open] pre') && !document.querySelector('.history-diff script,.history-diff [onerror]')")?;
    // Refetch via existing invalidations and page backwards without duplicates.
    b.eval("(async()=>{for(let i=0;i<52;i++){const r=await fetch('/api/v1/tickets/2',{method:'PATCH',headers:{'Content-Type':'application/json'},body:JSON.stringify({assignee:'fixture-'+i})});if(!r.ok)throw Error('history pagination seed');}return true})()")?;
    b.go("http://127.0.0.1:8795/ticket?id=2")?;
    b.wait("document.querySelectorAll('.history-event').length===50 && !!document.querySelector('.history-more')")?;
    b.eval("window.savedFetch=window.fetch;window.fetch=(i,o)=>((i instanceof Request?i.url:String(i)).includes('/history?cursor=')?Promise.resolve(new Response(JSON.stringify({error:{message:'Activity fixture failure'}}),{status:503,headers:{'Content-Type':'application/json'}})):window.savedFetch(i,o));true")?;
    b.click(".history-more")?;
    b.wait("document.querySelector('.history-error')?.textContent.includes('Activity fixture failure') && document.querySelectorAll('.history-event').length===50")?;
    b.eval("window.fetch=window.savedFetch;true")?;
    click_text(b, ".history-error button", "Retry activity")?;
    b.wait("document.querySelectorAll('.history-event').length>50 && !document.querySelector('.history-more') && new Set([...document.querySelectorAll('.history-event')].map(e=>e.dataset.historyId)).size===document.querySelectorAll('.history-event').length")?;
    b.eval("window.loadedHistoryIds=[...document.querySelectorAll('.history-event')].map(e=>e.dataset.historyId);true")?;
    b.click(".history-diff summary")?;
    b.click(".history-event .activity-time")?;
    b.wait("document.querySelector('.history-event .activity-time[aria-pressed=true]')?.textContent.includes('UTC')")?;

    b.click("button[aria-label='Remove dependency 3']")?;
    b.wait("document.querySelectorAll('.dependency-row').length===1 && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.wait("window.loadedHistoryIds.every(id=>document.querySelector('[data-history-id=\"'+id+'\"]')) && !!document.querySelector('.history-diff[open]') && !document.querySelector('.history-more')")?;
    b.wait("fetch('/api/v1/tickets/2').then(r=>r.json()).then(t=>t.blocks.length===0 && t.blocked_by.length===1)")?;
    click_text(b, "button", "Add dependency")?;
    b.text("#dependency-search", "TKT-4 ")?;
    b.click(".dependency-options button")?;
    b.wait("!document.querySelector('dialog') && document.querySelectorAll('.dependency-row').length===2 && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.wait("fetch('/api/v1/tickets/2').then(r=>r.json()).then(t=>t.blocked_by.includes(4))")?;
    b.click("button[aria-label='Remove dependency 4']")?;
    b.wait("document.querySelectorAll('.dependency-row').length===1 && document.querySelector('.detail-layout').getAttribute('aria-busy')==='false'")?;
    b.eval("window.savedFetch=window.fetch;window.fetch=(i,o)=>((i instanceof Request?i.method:o?.method)==='POST'?Promise.resolve(new Response(JSON.stringify({error:{message:'Comment verification rejection'}}),{status:422,headers:{'Content-Type':'application/json'}})):window.savedFetch(i,o));true")?;
    b.text(
        "textarea[aria-label='Write a comment']",
        "Keep this draft after failure",
    )?;
    click_text(b, ".comment-form button", "Comment")?;
    b.wait("document.querySelector('[role=alert]')?.textContent.includes('Comment verification rejection') && document.querySelector('textarea[aria-label=\"Write a comment\"]').value==='Keep this draft after failure'")?;
    b.eval("window.fetch=window.savedFetch;true")?;
    b.go("http://127.0.0.1:8795/t/UNKNOWN-999999")?;
    b.wait("document.querySelector('[role=alert]')?.textContent.toLowerCase().includes('not found') && [...document.querySelectorAll('button')].some(b=>b.textContent==='Edit ticket' && b.disabled)")?;
    b.call("Network.clearBrowserCookies", json!({}))?;
    b.go("http://127.0.0.1:8795/t/qua-87")?;
    b.wait("location.pathname==='/login' && new URLSearchParams(location.search).get('next')==='/t/qua-87'")?;
    println!(
        "PASS detail: fields, project move/reload, labels attach/detach, both dependency directions and inline 409, dependency removal, CommonMark/GFM, preview and persisted component XSS (script/onerror/javascript/raw HTML), comment CRUD, create shortcut/project selection, failed-save restoration and draft retention, list Enter, desktop/mobile screenshots"
    );
    Ok(())
}
fn click_text(b: &mut Browser, selector: &str, text: &str) -> Result<()> {
    b.eval(&format!("(()=>{{const e=[...document.querySelectorAll({})].find(e=>e.textContent.trim()==={});if(!e)throw Error('Missing button');e.click();return true;}})()",serde_json::to_string(selector)?,serde_json::to_string(text)?))?;
    Ok(())
}
pub(crate) fn select_value(b: &mut Browser, label: &str, value: &str) -> Result<()> {
    b.choose(
        &format!(
            "button[role=combobox][aria-label={}]",
            serde_json::to_string(label)?
        ),
        value,
    )
}

/// Creation exercises the real POST contract, context defaults and retryable label writes.
fn check_composer(b: &mut Browser) -> Result<()> {
    b.eval("fetch('/api/v1/labels',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name:'Review',color:'#c6a1b9'})}).then(r=>{if(!r.ok)throw Error('Could not seed composer label');return true})")?;
    b.viewport(1440, 900)?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("!!document.querySelector('[aria-label=\"Add ticket to Backlog\"]') && !document.querySelector('[aria-label=\"Add ticket to Backlog\"]').disabled")?;
    b.wait("!document.querySelector('.topbar button') && !document.querySelector('.topbar .toolbar-divider') && document.querySelector('.topbar').getBoundingClientRect().height===44")?;
    for (label, value) in [
        ("Backlog", "backlog"),
        ("Todo", "todo"),
        ("In progress", "in_progress"),
        ("In review", "in_review"),
        ("Done", "done"),
        ("Canceled", "canceled"),
    ] {
        b.click(&format!("[aria-label='Add ticket to {label}']"))?;
        b.wait(&format!("document.activeElement.id==='board-title' && document.querySelector('[aria-label=\"New ticket status\"]').value==='{label}'"))?;
        b.text("#board-title", &format!("Composer {value}"))?;
        if value == "backlog" {
            b.key("Tab", 9, 0)?;
            b.wait("document.activeElement.textContent==='Write'")?;
            b.key("Tab", 9, 0)?;
            b.wait("document.activeElement.id==='composer-body'")?;
            b.key("Tab", 9, 0)?;
            b.wait("document.activeElement.getAttribute('aria-label')==='New ticket project'")?;
        }
        select_value(b, "New ticket status", label)?;
        select_value(b, "New ticket priority", "High")?;
        b.text("#composer-body", "Ready for **review**.")?;
        if value == "backlog" {
            b.click(".composer-labels .menu-trigger")?;
            b.wait("document.querySelector('.composer-labels .menu-trigger').getAttribute('aria-expanded')==='true' && document.activeElement.id==='composer-label-filter'")?;
            b.key("ArrowDown", 40, 0)?;
            b.wait("document.activeElement.getAttribute('role')==='option'")?;
            b.key("Enter", 13, 0)?;
            b.key("ArrowDown", 40, 0)?;
            b.key("Enter", 13, 0)?;
            b.wait("document.querySelectorAll('.composer-labels > .label-chip').length===2")?;
            b.key("Escape", 27, 0)?;
            // Hold creation to verify busy/locking, then fail one label attachment.
            b.eval("window.composerFetch=window.fetch;window.fetch=async(input,init)=>{const url=input instanceof Request?input.url:String(input);const method=input instanceof Request?input.method:init?.method;if(method==='POST' && url.endsWith('/api/v1/tickets') && !window.composerHeld){window.composerHeld=true;await new Promise(r=>window.releaseComposer=r);}if(method==='POST' && /tickets\\/[^/]+\\/labels\\//.test(url) && !window.labelRejected){window.labelRejected=true;return new Response(JSON.stringify({error:{code:'unavailable',message:'Label save interrupted'}}),{status:503});}return window.composerFetch(input,init);};true")?;
        }
        b.key("Enter", 13, 4)?;
        if value == "backlog" {
            b.wait("window.composerHeld && document.querySelector('dialog .busy-button').disabled && document.querySelector('dialog .busy-button').getAttribute('aria-busy')==='true'")?;
            b.key("Escape", 27, 0)?;
            b.wait("!!document.querySelector('dialog')")?;
            b.eval("window.releaseComposer();true")?;
            b.wait("document.querySelector('.composer-error')?.textContent.includes('was created') && !document.querySelector('dialog .busy-button').disabled")?;
            b.click("dialog button[type=submit]")?;
        }
        b.wait("!document.querySelector('dialog')")?;
        b.wait(&format!("[...document.querySelectorAll('.api-ticket')].some(e=>e.querySelector('.card-title')?.textContent==='Composer {value}' && e.closest('section').getAttribute('aria-label')==='{label}')"))?;
    }
    b.eval("window.fetch=window.composerFetch;true")?;
    b.go("http://127.0.0.1:8795/board?canceled=true")?;
    b.wait("[...document.querySelectorAll('.card-title')].filter(e=>e.textContent.startsWith('Composer ')).length===6")?;
    b.wait("fetch('/api/v1/tickets?project=1&limit=100').then(r=>r.json()).then(r=>{const ts=r.data.filter(t=>t.title.startsWith('Composer '));return ts.length===6 && ts.every(t=>t.title==='Composer '+t.status && t.priority===2 && t.body==='Ready for **review**.') && ts.find(t=>t.status==='backlog').labels.length===2})")?;
    println!(
        "PASS composer: all six API statuses persisted; column/Todo defaults; keyboard flow and submit; busy lock; label retry without duplicate creation"
    );
    Ok(())
}
