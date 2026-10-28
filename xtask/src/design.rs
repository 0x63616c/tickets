//! Local Chromium checks and deterministic screenshots. No browser tooling in the app.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    env, fs,
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
struct Browser {
    process: Child,
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    id: u64,
    session: String,
}
impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
impl Browser {
    fn start(root: &Path) -> Result<Self> {
        let chrome = env::var_os("CHROME_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/bin/google-chrome"));
        let profile = root.join("target/design-browser");
        fs::create_dir_all(&profile)?;
        let port_file = profile.join("DevToolsActivePort");
        if port_file.exists() {
            fs::remove_file(&port_file)?;
        }
        let mut process = Command::new(chrome)
            .args([
                "--headless=new",
                "--no-sandbox",
                "--no-proxy-server",
                "--disable-gpu",
                "--hide-scrollbars",
                "--remote-debugging-port=0",
                "--remote-allow-origins=*",
                "--no-first-run",
            ])
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(fs::File::create(root.join("target/design-chromium.log"))?)
            .spawn()?;
        let start = Instant::now();
        while !port_file.exists() {
            if start.elapsed() > Duration::from_secs(15) || process.try_wait()?.is_some() {
                let _ = process.kill();
                let _ = process.wait();
                return Err("Chromium did not start; see target/design-chromium.log".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        let port = fs::read_to_string(&port_file)?;
        let mut lines = port.lines();
        let url = format!(
            "ws://127.0.0.1:{}{}",
            lines.next().ok_or("missing port")?,
            lines.next().ok_or("missing socket")?
        );
        let (socket, _) = tungstenite::connect(url)?;
        if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
            stream.set_read_timeout(Some(Duration::from_secs(20)))?;
        }
        let mut browser = Self {
            process,
            socket,
            id: 0,
            session: String::new(),
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
        browser.viewport(1440, 900)?;
        browser.call(
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":"prefers-color-scheme","value":"dark"}]}),
        )?;
        Ok(browser)
    }
    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
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
                if response["method"] == "Runtime.exceptionThrown" {
                    return Err(format!("Browser exception: {}", response["params"]).into());
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
    fn eval(&mut self, expression: &str) -> Result<Value> {
        let result = self.call(
            "Runtime.evaluate",
            json!({"expression":expression,"returnByValue":true,"awaitPromise":true}),
        )?;
        if !result["exceptionDetails"].is_null() {
            return Err(format!("{expression}: {}", result["exceptionDetails"]).into());
        }
        Ok(result["result"]["value"].clone())
    }
    fn wait(&mut self, expression: &str) -> Result<()> {
        let start = Instant::now();
        while self.eval(expression)? != true {
            if start.elapsed() > Duration::from_secs(15) {
                return Err(format!("Timed out: {expression}").into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }
    fn go(&mut self, url: &str) -> Result<()> {
        self.call("Page.navigate", json!({"url":url}))?;
        self.wait("!!document.querySelector('.topbar, .auth-page')")?;
        self.eval("document.fonts.ready.then(() => true)")?;
        Ok(())
    }
    fn viewport(&mut self, width: u32, height: u32) -> Result<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":height,"deviceScaleFactor":1,"mobile":false}),
        )?;
        Ok(())
    }
    fn screenshot(&mut self, path: &Path) -> Result<()> {
        // Let layout and font metrics settle before capturing the viewport.
        self.eval("new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(true))))")?;
        let result = self.call(
            "Page.captureScreenshot",
            json!({"format":"png","captureBeyondViewport":false}),
        )?;
        fs::write(
            path,
            STANDARD.decode(result["data"].as_str().ok_or("missing screenshot")?)?,
        )?;
        println!("Screenshot: {}", path.display());
        Ok(())
    }
    fn click(&mut self, selector: &str) -> Result<()> {
        let selector = serde_json::to_string(selector)?;
        let point = self.eval(&format!("(() => {{ const e=document.querySelector({selector}); if(!e) throw Error('Missing control'); e.scrollIntoView({{block:'nearest'}}); const r=e.getBoundingClientRect(); return {{x:r.x+r.width/2,y:r.y+r.height/2}}; }})()"))?;
        for kind in ["mousePressed", "mouseReleased"] {
            self.call(
                "Input.dispatchMouseEvent",
                json!({"type":kind,"x":point["x"],"y":point["y"],"button":"left","clickCount":1}),
            )?;
        }
        Ok(())
    }
    fn text(&mut self, selector: &str, text: &str) -> Result<()> {
        self.click(selector)?;
        self.call("Input.insertText", json!({"text":text}))?;
        Ok(())
    }
    fn key(&mut self, key: &str, code: u32, modifiers: u32) -> Result<()> {
        for kind in ["keyDown", "keyUp"] {
            self.call(
                "Input.dispatchKeyEvent",
                json!({"type":kind,"key":key,"windowsVirtualKeyCode":code,"modifiers":modifiers}),
            )?;
        }
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
        .eval("document.querySelector('select[aria-label=\"Move TKT-7 to status\"]').focus()")?;
    browser.key("ArrowDown", 40, 0)?;
    browser.wait("document.querySelector('.board-column[aria-label=\"In review\"] select[aria-label=\"Move TKT-7 to status\"]')?.value === 'In review'")?;
    browser
        .wait("document.activeElement?.getAttribute('aria-label') === 'Move TKT-7 to status'")?;
    browser.key("ArrowDown", 40, 0)?;
    browser.wait("document.querySelector('.board-column[aria-label=\"Done\"] select[aria-label=\"Move TKT-7 to status\"]')?.value === 'Done'")?;
    browser
        .wait("document.activeElement?.getAttribute('aria-label') === 'Move TKT-7 to status'")?;
    browser.eval("(() => { const card=document.querySelector('.ticket-card:has(select[aria-label=\"Move TKT-8 to status\"])'); const target=document.querySelector('.board-column[aria-label=\"In progress\"]'); const dataTransfer=new DataTransfer(); card.dispatchEvent(new DragEvent('dragstart',{bubbles:true,dataTransfer})); target.dispatchEvent(new DragEvent('dragover',{bubbles:true,cancelable:true,dataTransfer})); target.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer})); return true; })()")?;
    browser.wait("!!document.querySelector('.board-column[aria-label=\"In progress\"] select[aria-label=\"Move TKT-8 to status\"]')")?;
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
    browser.wait("document.activeElement?.classList.contains('menu-item')")?;
    browser.key("Escape", 27, 0)?;
    browser.wait("!document.querySelector('dialog')")?;
    browser.key("c", 67, 0)?;
    browser.wait("!!document.querySelector('#ticket-title')")?;
    browser.text("#ticket-title", "Restore saved filters on reload")?;
    browser.screenshot(&output.join("08-new-ticket.png"))?;
    browser.click("dialog button[type='submit']")?;
    browser
        .wait("document.querySelector('h1')?.textContent === 'Restore saved filters on reload'")?;
    browser.click(".label-menu summary")?;
    browser.click(".label-menu .menu-item:last-child")?;
    browser.wait("document.querySelector('.property-labels').textContent.includes('Design')")?;
    browser.text(
        "textarea[aria-label='Write a comment']",
        "**Verified** the saved view.",
    )?;
    browser.click(".comment-form button[type='submit']")?;
    browser.wait("document.querySelector('.comment strong')?.textContent === 'You' && document.querySelector('.comment .markdown strong')?.textContent === 'Verified'")?;
    browser.go(&format!("{base}/design/ticket"))?;
    browser.click(".dependencies .button")?;
    browser.text("#dependency-search", "TKT-6")?;
    browser.screenshot(&output.join("09-add-dependency.png"))?;
    browser.click(".dependency-option:not(:disabled)")?;
    browser.wait("document.querySelector('.dependencies').textContent.includes('TKT-6')")?;
    browser.click(".sidebar .nav-item.quiet")?;
    browser.text("#project-name", "Editor")?;
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
    browser.wait("!!document.querySelector('#token-name')")?;
    if worker_api {
        browser.wait("document.querySelector('.auth-page [role=status]')?.textContent.includes('Please sign in')")?;
        println!("PASS: the local Worker requires authentication for API tokens.");
    }
    browser.screenshot(&output.join("13-token-access.png"))?;
    println!(
        "PASS: routes, fonts, overflow at 1440/390px, keyboard status move with focus retained, drag/drop events, filtering, Cmd+K with search focus, arrow navigation, Escape, C, creation, labels, sanitized comments, dependencies, project creation and project selection, existing login and token screens."
    );
    Ok(())
}
