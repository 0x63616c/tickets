//! Fixture-only, isolated component PNGs. The server binds an OS-assigned loopback port.
use crate::design::Browser;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
const NAMES: &[&str] = &[
    "ticket-type",
    "pr-check-status",
    "mission",
    "pickup-rules",
    "history",
    "board-topbar",
    "board-columns",
    "palette",
    "sidebar",
    "user-menu",
    "project-switcher",
    "select",
    "menu",
    "combobox",
    "modal",
    "loader",
    "skeletons",
    "reveal",
    "icons",
    "busy-button",
    "tabs",
    "tooltip",
];
pub(crate) struct FixtureServer {
    pub url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
impl FixtureServer {
    pub(crate) fn start(assets: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_with_api(assets, std::collections::HashMap::new())
    }
    pub(crate) fn start_with_api(
        assets: &Path,
        fixtures: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let url = format!("http://{}", listener.local_addr()?);
        let assets = assets.canonicalize()?;
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let thread = thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                        let mut request = [0; 8192];
                        let size = stream.read(&mut request).unwrap_or(0);
                        let line = String::from_utf8_lossy(&request[..size]);
                        let request_path = line.split_whitespace().nth(1).unwrap_or("/");
                        let path = line
                            .split_whitespace()
                            .nth(1)
                            .unwrap_or("/")
                            .split('?')
                            .next()
                            .unwrap_or("/");
                        let file = assets.join(path.trim_start_matches('/'));
                        let safe = file
                            .canonicalize()
                            .ok()
                            .filter(|p| p.starts_with(&assets) && p.is_file());
                        let (body, mime) = if let Some(fixture) =
                            fixtures.get(request_path).or_else(|| fixtures.get(path))
                        {
                            (serde_json::to_vec(fixture).unwrap(), "application/json")
                        } else if path.starts_with("/api/v1/projects") {
                            (br##"{"data":[{"id":1,"name":"Tickets","key_prefix":"TKT","color":"#5e6ad2","archived":false,"icon":null,"github_owner":null,"github_repo":null,"github_project_number":null,"github_project_kind":"users","next_ticket_number":1}],"next_cursor":null}"##.to_vec(),"application/json")
                        } else if path == "/api/v1/tokens" {
                            (b"[]".to_vec(), "application/json")
                        } else if path.starts_with("/api/") {
                            (
                                br#"{"authenticated":true,"data":[],"next_cursor":null}"#.to_vec(),
                                "application/json",
                            )
                        } else if let Some(file) = safe {
                            let mime = match file.extension().and_then(|s| s.to_str()).unwrap_or("")
                            {
                                "wasm" => "application/wasm",
                                "js" => "text/javascript",
                                "css" => "text/css",
                                "woff2" => "font/woff2",
                                "png" => "image/png",
                                "svg" => "image/svg+xml",
                                _ => "text/html",
                            };
                            (fs::read(file).unwrap_or_default(), mime)
                        } else {
                            (
                                fs::read(assets.join("index.html")).unwrap_or_default(),
                                "text/html",
                            )
                        };
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(&body);
                    }
                    Err(_) => thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Ok(Self {
            url,
            stop,
            thread: Some(thread),
        })
    }
}
pub fn run(root: &Path, args: &[String]) -> super::Result {
    let name = args.first().map(String::as_str).unwrap_or("all");
    if name != "all" && !NAMES.contains(&name) {
        return Err(format!("unknown component {name}; use {}", NAMES.join(", ")).into());
    }
    let arg = |flag: &str| args.windows(2).find(|p| p[0] == flag).map(|p| p[1].clone());
    let before = arg("--before-assets");
    let variant = arg("--variant").unwrap_or_default();
    let assets = before
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let output = arg("--output")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/component-shots"));
    fs::create_dir_all(&output)?;
    let server = FixtureServer::start(&assets)?;
    let mut b = Browser::start_named(root, "component")?;
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1200,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    for name in if name == "all" {
        NAMES.to_vec()
    } else {
        vec![name]
    } {
        if name == "ticket-type" {
            classification_shots(&mut b, &server.url, &output, before.is_some(), &assets)?;
            continue;
        }
        if name == "project-switcher" {
            project_switcher_shots(root, &assets, &output, before.is_some())?;
            continue;
        }
        if name == "mission" {
            super::mission::shots(root, &assets, &output, before.is_some())?;
            continue;
        }
        if name == "pickup-rules" {
            super::pickup::shots(root, &assets, &output, before.is_some())?;
            continue;
        }
        if name == "board-columns" {
            capture_board_columns(&mut b, &server.url, &output, before.is_some())?;
            continue;
        }
        if name == "board-topbar" {
            capture_board_topbar(&mut b, &server.url, &output, before.is_some())?;
            continue;
        }
        if name == "palette" {
            super::palette::shots(root, &assets, &output, before.is_some())?;
            continue;
        }
        if name == "modal" {
            capture_composer(&mut b, &server.url, &output, before.is_some())?;
            continue;
        }
        if matches!(name, "sidebar" | "user-menu") {
            sidebar_shots(&mut b, &server.url, name, &output, before.is_some())?;
            continue;
        }
        if before.is_some() {
            capture_before(&mut b, &server.url, name, &output)?;
            continue;
        }
        b.go(&format!("{}/design/c/{name}?variant={variant}", server.url))?;
        b.wait("!!document.querySelector('.component-stage')")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        match name {
            "select" => b.click("[role=combobox]")?,
            "modal" => b.click(".component-stage > button")?,
            "menu" | "combobox" => b.click(".menu-trigger")?,
            "user-menu" => b.click(".account-trigger")?,
            "reveal" => {
                b.wait("!!document.querySelector('.reveal-preview h2')")?;
            }
            "tooltip" => {
                b.eval("document.querySelector('.icon-button').focus();true")?;
            }
            _ => {}
        }
        b.eval("new Promise(r=>setTimeout(()=>r(true),300))")?;
        let selector = if name == "modal" {
            "dialog"
        } else {
            ".component-stage"
        };
        let capture = if matches!(name, "sidebar" | "user-menu") {
            visible_crop
        } else {
            crop
        };
        capture(
            &mut b,
            selector,
            &output.join(format!(
                "after-{name}{}.png",
                if variant.is_empty() {
                    String::new()
                } else {
                    format!("-{variant}")
                }
            )),
        )?;
        if matches!(name, "select" | "menu" | "combobox") {
            check_menu_spacing(&mut b)?;
        }
        match name {
            "select" => check_select(&mut b)?,
            "menu" => check_menu(&mut b)?,
            "modal" => check_modal(&mut b)?,
            "combobox" => check_combobox(&mut b)?,
            "tabs" => {
                b.click(".segmented-control button")?;
                b.key("ArrowRight", 39, 0)?;
                b.wait("document.activeElement.textContent==='Preview' && document.activeElement.getAttribute('aria-pressed')==='true'")?;
            }
            "busy-button" => {
                b.wait("[...document.querySelectorAll('.busy-button')].every(b=>b.disabled && b.getAttribute('aria-busy')==='true')")?;
                b.wait("(()=>{const b=document.querySelectorAll('.sample-controls > button');return Math.abs(b[0].getBoundingClientRect().width-b[2].getBoundingClientRect().width)<1})()")?;
            }
            "loader" => {
                b.call(
                    "Emulation.setEmulatedMedia",
                    json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
                )?;
                b.wait("document.querySelector('.ars') && !document.querySelector('.ars canvas')")?;
                b.call(
                    "Emulation.setEmulatedMedia",
                    json!({"features":[{"name":"prefers-reduced-motion","value":"no-preference"}]}),
                )?;
            }
            _ => {}
        }
    }
    println!("PASS component PNGs: fixture data, dark, 2× DPR, tight bounds, loaded fonts");
    Ok(())
}
fn check_menu_spacing(b: &mut Browser) -> super::Result {
    b.wait("(()=>{const p=document.querySelector('.popup:not([hidden])');if(!p)return false;const r=p.getBoundingClientRect();return [...p.querySelectorAll('.menu-item')].every(i=>{const q=i.getBoundingClientRect(),prev=i.previousElementSibling;return q.left-r.left>=4 && r.right-q.right>=4 && getComputedStyle(i).borderRadius==='4px' && (!prev?.matches('.menu-item') || Math.abs(q.top-prev.getBoundingClientRect().bottom-2)<0.5);});})()")?;
    Ok(())
}
fn sidebar_shots(
    b: &mut Browser,
    base: &str,
    name: &str,
    output: &Path,
    before: bool,
) -> super::Result {
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1200,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    let prefix = if before { "before" } else { "after" };
    b.go(&format!("{base}/design/c/{name}"))?;
    b.wait("!!document.querySelector('.sidebar .account-trigger')")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    b.wait("document.querySelector('.sidebar').getBoundingClientRect().width===240 && document.querySelector('.sidebar').getBoundingClientRect().height===620")?;
    if !before {
        b.wait("document.querySelector('.workspace').textContent==='Tickets' && !document.querySelector('.workspace button') && !document.body.textContent.includes('Tickets account') && document.querySelector('.account-name').textContent==='Sample User' && document.querySelector('.account-trigger .avatar').textContent==='S'")?;
    }
    if name == "user-menu" {
        b.click(".account-trigger")?;
        b.wait(
            "document.querySelector('.account-trigger').getAttribute('aria-expanded')==='true'",
        )?;
    }
    visible_crop(b, ".sidebar", &output.join(format!("{prefix}-{name}.png")))?;
    if name == "sidebar" {
        let evidence = output;
        let row = b.eval("(()=>{const r=document.querySelector('.project-nav').getBoundingClientRect();return {x:r.right-12,y:r.top+r.height/2}})()")?;
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":row["x"],"y":row["y"]}),
        )?;
        if before {
            visible_crop(b, ".sidebar", &evidence.join("before-project-menu.png"))?;
            b.go(&format!("{base}/projects"))?;
            b.wait("!!document.querySelector(\"[aria-label='Edit project Tickets']\")")?;
            b.click("[aria-label='Edit project Tickets']")?;
            b.wait("document.activeElement.id==='project-title'")?;
            crop(b, "dialog", &evidence.join("before-project-edit.png"))?;
            b.key("Escape", 27, 0)?;
            b.wait("!document.querySelector('dialog')")?;
        } else {
            b.eval("document.querySelector('.project-more-trigger').focus();true")?;
            b.wait(
                "getComputedStyle(document.querySelector('.project-more-trigger')).opacity==='1'",
            )?;
            b.key("Enter", 13, 0)?;
            b.wait("document.querySelector('.project-more-trigger').getAttribute('aria-expanded')==='true' && document.activeElement.matches('[role=menuitem]')")?;
            check_menu_spacing(b)?;
            crop(b, ".sidebar", &evidence.join("after-project-menu.png"))?;
            b.key("ArrowDown", 40, 0)?;
            b.wait("document.activeElement.textContent.includes('Copy link')")?;
            b.key("Escape", 27, 0)?;
            b.wait("document.activeElement.matches('.project-more-trigger') && document.activeElement.getAttribute('aria-expanded')==='false'")?;
            b.key(" ", 32, 0)?;
            b.wait("document.activeElement.textContent.includes('Edit project')")?;
            b.key("Enter", 13, 0)?;
            b.wait("document.activeElement.id==='project-title'")?;
            crop(b, "dialog", &evidence.join("after-project-edit.png"))?;
            b.key("Escape", 27, 0)?;
            b.wait("!document.querySelector('dialog') && document.activeElement.matches('.project-more-trigger')")?;
        }
        b.go(&format!("{base}/design/c/{name}"))?;
        b.wait("!!document.querySelector('.sidebar')")?;
    }
    if !before && name == "user-menu" {
        check_menu_spacing(b)?;
        b.wait("document.querySelector('.account-dropdown .popup').dataset.side==='top' && getComputedStyle(document.querySelector('.account-dropdown .popup')).transformOrigin.startsWith('0px')")?;
        b.wait("document.activeElement.matches('[role=menuitem]') && document.activeElement.textContent.startsWith('Settings') && document.activeElement.querySelector('kbd')?.textContent===(navigator.platform.includes('Mac')?'⌘7':'Ctrl+7')")?;
        b.key("ArrowDown", 40, 0)?;
        b.wait("document.activeElement.textContent.includes('Keyboard shortcuts')")?;
        b.key(" ", 32, 0)?;
        b.wait("!!document.querySelector('dialog[open]')")?;
        b.key("Escape", 27, 0)?;
        b.wait("!document.querySelector('dialog') && document.activeElement.matches('.account-trigger')")?;
        for (key, code) in [("Enter", 13), (" ", 32), ("ArrowUp", 38), ("ArrowDown", 40)] {
            b.key(key, code, 0)?;
            b.wait("document.querySelector('.account-trigger').getAttribute('aria-expanded')==='true' && document.activeElement.matches('[role=menuitem]')")?;
            b.key("Escape", 27, 0)?;
            b.wait("document.activeElement.matches('.account-trigger') && document.querySelector('.account-trigger').getAttribute('aria-expanded')==='false'")?;
        }
    }
    if !before && name == "user-menu" {
        b.key("Enter", 13, 0)?;
        b.wait("document.activeElement.matches('[role=menuitem]') && document.activeElement.textContent.startsWith('Settings') && document.activeElement.querySelector('kbd')?.textContent===(navigator.platform.includes('Mac')?'⌘7':'Ctrl+7')")?;
        b.key("Enter", 13, 0)?;
        b.wait("location.pathname==='/design/settings/account'")?;
    }
    if name == "sidebar" {
        b.viewport(1200, 900)?;
        b.go(&format!("{base}/design/board"))?;
        b.wait("!!document.querySelector('.sidebar .account-trigger')")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        b.screenshot(&output.join(format!("{prefix}-desktop.png")))?;
        if !before {
            b.click(".project-more-trigger")?;
            b.wait("document.querySelector('.project-more-trigger').getAttribute('aria-expanded')==='true'")?;
            b.screenshot(&output.join("after-project-menu-full.png"))?;
            b.key("Escape", 27, 0)?;
            b.wait("document.querySelector('.project-more-trigger').getAttribute('aria-expanded')==='false'")?;
        }
        b.viewport(390, 844)?;
        b.click("[aria-label='Toggle sidebar']")?;
        b.wait("document.querySelector('[aria-label=\"Toggle sidebar\"]').getAttribute('aria-expanded')==='true'")?;
        b.click(".account-trigger")?;
        b.wait(
            "document.querySelector('.account-trigger').getAttribute('aria-expanded')==='true'",
        )?;
        b.screenshot(&output.join(format!("{prefix}-mobile.png")))?;
        if !before {
            b.wait("document.querySelector('.account-dropdown .popup').dataset.side==='top'")?;
            b.key("Escape", 27, 0)?;
            b.wait("document.activeElement.matches('.account-trigger') && !!document.querySelector('.sidebar-dismiss')")?;
            b.key("Escape", 27, 0)?;
            b.wait("document.activeElement.getAttribute('aria-label')==='Toggle sidebar' && !document.querySelector('.sidebar-dismiss')")?;
            b.viewport(390, 360)?;
            b.click("[aria-label='Toggle sidebar']")?;
            b.wait("(()=>{const n=document.querySelector('.sidebar-nav'),a=document.querySelector('.account-trigger').getBoundingClientRect();return n.scrollHeight>n.clientHeight && a.bottom<=innerHeight && a.top>=44;})()")?;
        }
    }
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1200,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    Ok(())
}
pub(crate) fn crop(b: &mut Browser, selector: &str, path: &Path) -> super::Result {
    b.eval("Promise.all(document.getAnimations().filter(a=>a.effect?.getTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{}))).then(()=>true)")?;
    b.wait("[...document.querySelectorAll('.popup:not([hidden])')].every(p=>getComputedStyle(p).opacity==='1')")?;
    b.eval(
        "new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve(true))))",
    )?;
    let selector = serde_json::to_string(selector)?;
    b.eval("Promise.race([Promise.all(document.getAnimations().filter(a=>a.effect?.getTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{}))),new Promise(r=>setTimeout(r,300))]).then(()=>true)")?;
    let clip=b.eval(&format!("(()=>{{const e=document.querySelector({selector}),r=e.getBoundingClientRect();let left=r.left,top=r.top,right=r.right,bottom=r.bottom;e.querySelectorAll('.popup:not([hidden])').forEach(p=>{{const q=p.getBoundingClientRect();left=Math.min(left,q.left);top=Math.min(top,q.top);right=Math.max(right,q.right);bottom=Math.max(bottom,q.bottom);}});return {{x:Math.max(0,Math.floor(left+window.scrollX)-8),y:Math.max(0,Math.floor(top+window.scrollY)-8),width:Math.ceil(right-left)+16,height:Math.ceil(bottom-top)+16,scale:1}}}})()"))?;
    let result = b.call(
        "Page.captureScreenshot",
        json!({"format":"png","captureBeyondViewport":true,"clip":clip}),
    )?;
    fs::write(
        path,
        STANDARD.decode(result["data"].as_str().ok_or("missing PNG")?)?,
    )?;
    println!("Screenshot: {}", path.display());
    Ok(())
}
/// Capture the visible compositor surface: managed headless Chrome can drop cached
/// text when rasterizing offscreen clips. Page zoom supplies real 2x pixels; canvas
/// only crops the captured bitmap, without resampling or changing the UI.
pub(crate) fn visible_crop(b: &mut Browser, selector: &str, path: &Path) -> super::Result {
    let scale = b.eval("devicePixelRatio")?.as_f64().unwrap_or(1.);
    let viewport = b.eval("({width:innerWidth,height:innerHeight})")?;
    let selector_json = serde_json::to_string(selector)?;
    let original_style=b.eval(&format!("(()=>{{const e=document.querySelector({selector_json}),s=e.style.cssText,r=e.getBoundingClientRect();e.style.width=r.width+'px';e.style.height=r.height+'px';return s}})()"))?;
    let w = viewport["width"].as_f64().unwrap();
    let h = viewport["height"].as_f64().unwrap();
    b.call("Emulation.setDeviceMetricsOverride",json!({"width":(w*scale) as u32,"height":(h*scale) as u32,"deviceScaleFactor":1,"mobile":false}))?;
    b.call(
        "Emulation.setPageScaleFactor",
        json!({"pageScaleFactor":scale}),
    )?;
    b.eval("new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(()=>r(true))))")?;
    let selector = serde_json::to_string(selector)?;
    let rect=b.eval(&format!("(()=>{{const r=document.querySelector({selector}).getBoundingClientRect();return {{x:Math.max(0,r.x-8),y:Math.max(0,r.y-8),width:r.width+16,height:r.height+16}}}})()"))?;
    let png = b.call(
        "Page.captureScreenshot",
        json!({"format":"png","fromSurface":false}),
    )?;
    let data = png["data"].as_str().ok_or("missing PNG")?;
    let cropped=b.eval(&format!("new Promise((resolve,reject)=>{{const i=new Image();i.onload=()=>{{const c=document.createElement('canvas'),r={rect},s={scale};c.width=Math.ceil(r.width*s);c.height=Math.ceil(r.height*s);c.getContext('2d').drawImage(i,Math.floor(r.x*s),Math.floor(r.y*s),c.width,c.height,0,0,c.width,c.height);resolve(c.toDataURL('image/png').split(',')[1]);}};i.onerror=reject;i.src='data:image/png;base64,{data}';}})"))?;
    fs::write(
        path,
        STANDARD.decode(cropped.as_str().ok_or("missing crop")?)?,
    )?;
    b.call("Emulation.setPageScaleFactor", json!({"pageScaleFactor":1}))?;
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":w as u32,"height":h as u32,"deviceScaleFactor":scale,"mobile":false}),
    )?;
    b.eval(&format!(
        "document.querySelector({selector_json}).style.cssText={original_style};true"
    ))?;
    println!("Screenshot: {}", path.display());
    Ok(())
}

fn capture_before(b: &mut Browser, base: &str, name: &str, output: &Path) -> super::Result {
    let route = match name {
        "sidebar" | "user-menu" => "/design/board",
        "menu" | "combobox" => "/design/ticket",
        "icons" => "/design/projects",
        "loader" | "skeletons" => "/board",
        "busy-button" | "modal" | "tabs" => "/",
        _ => "/design",
    };
    if matches!(name, "loader" | "skeletons") {
        b.call(
            "Fetch.enable",
            json!({"patterns":[{"urlPattern":"*/api/v1/projects*","requestStage":"Request"}]}),
        )?;
    }
    b.go(&format!("{base}{route}"))?;
    let selector = match name {
        "sidebar" => {
            b.wait("!!document.querySelector('.sidebar')")?;
            ".sidebar"
        }
        "user-menu" => {
            b.wait("!!document.querySelector('.workspace')")?;
            ".workspace"
        }
        "modal" | "tabs" => {
            b.wait("!document.querySelector('.topbar .button-primary').disabled")?;
            b.click(".topbar .button-primary")?;
            b.wait("!!document.querySelector('dialog')")?;
            if name == "tabs" {
                ".editor-tabs"
            } else {
                "dialog"
            }
        }
        "select" => {
            b.click("select[aria-label='Example status']")?;
            ".form-specimen"
        }
        "menu" | "combobox" => {
            b.click(".label-menu summary")?;
            ".properties"
        }
        "icons" => ".projects-table",
        "busy-button" => {
            b.wait("!document.querySelector('.topbar .button-primary').disabled")?;
            b.click(".topbar .button-primary")?;
            b.text("#board-title", "Review component foundations")?;
            b.call(
                "Fetch.enable",
                json!({"patterns":[{"urlPattern":"*/api/v1/tickets","requestStage":"Request"}]}),
            )?;
            b.click("dialog button[type=submit]")?;
            b.wait("document.querySelector('dialog button[type=submit]').disabled")?;
            ".modal-footer"
        }
        "loader" | "skeletons" => {
            b.wait("!!document.querySelector('.loading-skeleton') || document.querySelector('.board-message')?.textContent.includes('Loading')===true")?;
            ".loading-skeleton, .board-message"
        }
        "tooltip" => ".specimen .icon-button",
        _ => ".specimen-content",
    };
    let capture = if matches!(name, "sidebar" | "user-menu") {
        visible_crop
    } else {
        crop
    };
    capture(b, selector, &output.join(format!("before-{name}.png")))?;
    b.call("Fetch.disable", json!({}))?;
    b.key("Escape", 27, 0)?;
    Ok(())
}
fn check_select(b: &mut Browser) -> super::Result {
    b.key("End", 35, 0)?;
    b.wait("document.activeElement.textContent.trim()==='Canceled'")?;
    b.key("Home", 36, 0)?;
    b.wait("document.activeElement.textContent.trim()==='Backlog'")?;
    b.key("d", 68, 0)?;
    b.wait("document.activeElement.textContent.trim()==='Done'")?;
    b.key("Enter", 13, 0)?;
    b.wait("document.querySelector('[role=combobox]').textContent.trim()==='Done' && document.activeElement.getAttribute('role')==='combobox'")?;
    b.key("ArrowDown", 40, 0)?;
    b.wait("document.querySelector('[role=combobox]').getAttribute('aria-expanded')==='true'")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.getAttribute('role')==='combobox'")?;
    b.key("End", 35, 0)?;
    b.wait("document.activeElement.textContent.trim()==='Canceled'")?;
    b.key("Escape", 27, 0)?;
    b.key("Home", 36, 0)?;
    b.wait("document.activeElement.textContent.trim()==='Backlog'")?;
    b.key("Escape", 27, 0)?;
    b.eval("document.querySelector('.select-root').style.position='fixed';document.querySelector('.select-root').style.bottom='8px';true")?;
    b.click("[role=combobox]")?;
    b.wait("document.querySelector('[role=listbox]').dataset.side==='top'")?;
    b.key("Escape", 27, 0)?;
    Ok(())
}

fn check_menu(b: &mut Browser) -> super::Result {
    b.key("End", 35, 0)?;
    b.wait("document.activeElement.getAttribute('role')==='menuitemcheckbox'")?;
    b.key(" ", 32, 0)?;
    b.wait("document.querySelector('[role=menuitemcheckbox]').getAttribute('aria-checked')==='false' && document.activeElement.matches('.menu-trigger')")?;
    b.key("Enter", 13, 0)?;
    b.key("Home", 36, 0)?;
    b.wait("document.activeElement.textContent.includes('Edit ticket')")?;
    b.key("c", 67, 0)?;
    b.wait("document.activeElement.textContent.includes('Copy link')")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.matches('.menu-trigger')")?;
    b.click(".menu-trigger")?;
    b.click(".component-stage h1")?;
    b.wait("document.querySelector('.menu-trigger').getAttribute('aria-expanded')==='false'")?;
    Ok(())
}

fn check_modal(b: &mut Browser) -> super::Result {
    let retained = b.eval("document.querySelector('dialog [aria-label=\"Close dialog\"]').click();new Promise(r=>setTimeout(()=>r(document.querySelector('dialog')?.dataset.state==='closed'),40))")?;
    if retained != json!(true) {
        return Err("dialog did not retain its exit".into());
    }
    b.wait(
        "!document.querySelector('dialog') && document.activeElement.textContent==='New ticket'",
    )?;
    b.click(".component-stage > button")?;
    b.wait("!!document.querySelector('dialog')")?;
    b.eval("document.querySelector('dialog [aria-label=\"Close dialog\"]').click();new Promise(r=>setTimeout(()=>{document.querySelector('.component-stage > button').click();r(true)},40))")?;
    b.eval("new Promise(r=>setTimeout(()=>r(true),220))")?;
    b.wait("document.querySelector('dialog')?.dataset.state==='open'")?;
    b.key("Escape", 27, 0)?;
    b.wait(
        "!document.querySelector('dialog') && document.activeElement.textContent==='New ticket'",
    )?;
    Ok(())
}
fn check_combobox(b: &mut Browser) -> super::Result {
    b.wait("document.activeElement.id==='label-filter'")?;
    b.key("ArrowDown", 40, 0)?;
    b.wait("document.activeElement.getAttribute('role')==='option'")?;
    b.key("ArrowUp", 38, 0)?;
    b.wait("document.activeElement.id==='label-filter'")?;
    b.text("#label-filter", "bug")?;
    b.wait("document.querySelectorAll('[role=option]').length===1")?;
    b.key("ArrowDown", 40, 0)?;
    b.key("Enter", 13, 0)?;
    b.wait("document.querySelector('[role=option]').getAttribute('aria-selected')==='true'")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.classList.contains('menu-trigger')")?;
    Ok(())
}

/// Same real composer route, fixture-only HTTP responses, and viewport for each baseline.
fn capture_composer(b: &mut Browser, base: &str, output: &Path, before: bool) -> super::Result {
    let phase = if before { "before" } else { "after" };
    b.go(&format!("{base}{}", if before { "/" } else { "/board" }))?;
    let trigger = if before {
        ".topbar .button-primary"
    } else {
        "[aria-label='Add ticket to Todo']"
    };
    b.wait(&format!(
        "document.querySelector({})?.disabled===false",
        serde_json::to_string(trigger)?
    ))?;
    b.click(trigger)?;
    b.wait("document.querySelector('dialog')?.dataset.state==='open'")?;
    b.eval("document.fonts.ready.then(()=>new Promise(r=>setTimeout(()=>r(true),300)))")?;
    crop(b, "dialog", &output.join(format!("{phase}-modal.png")))?;
    crop(
        b,
        ".segmented-control",
        &output.join(format!("{phase}-segmented-write.png")),
    )?;
    let height = b.eval("document.querySelector('dialog').getBoundingClientRect().height")?;
    b.click(".segmented-control button:last-child")?;
    crop(
        b,
        ".segmented-control",
        &output.join(format!("{phase}-segmented-preview.png")),
    )?;
    if !before {
        if b.eval("document.querySelector('dialog').getBoundingClientRect().height")? != height {
            return Err("composer preview changed modal height".into());
        }
        b.wait("document.querySelector('.preview-placeholder')?.textContent==='Nothing to preview yet.'")?;
    }
    crop(b, "dialog", &output.join(format!("{phase}-preview.png")))?;
    b.click(".segmented-control button:first-of-type")?;
    if !before {
        b.click("[aria-label='New ticket status']")?;
        b.wait("document.querySelector('[aria-label=\"New ticket status\"]').getAttribute('aria-expanded')==='true' && document.activeElement.getAttribute('role')==='option'")?;
        b.wait("document.querySelectorAll('[role=listbox][aria-label=\"New ticket status\"] svg[class*=status-]').length===6")?;
        crop(b, "dialog", &output.join("after-status-open.png"))?;
        b.key("End", 35, 0)?;
        b.wait("document.activeElement.textContent.trim()==='Canceled'")?;
        b.key("Enter", 13, 0)?;
        b.wait("document.activeElement.getAttribute('aria-label')==='New ticket status' && document.activeElement.textContent.trim()==='Canceled'")?;
        crate::design::select_value(b, "New ticket status", "Todo")?;
        // A keyboard toggle preserves the draft and the editor's geometry.
        b.text("#composer-body", "**Ready** for review.")?;
        b.key("P", 80, 12)?;
        b.wait("!document.querySelector('.markdown-preview').hidden && document.querySelector('.markdown-preview strong')?.textContent==='Ready'")?;
        b.key("P", 80, 12)?;
        b.wait("!document.querySelector('#composer-body').hidden && document.querySelector('#composer-body').value==='**Ready** for review.'")?;
        b.key("Escape", 27, 0)?;
        b.wait("document.activeElement.textContent==='Keep editing'")?;
        b.key("Enter", 13, 0)?;
        b.wait("document.activeElement.id==='composer-body' && document.activeElement.value==='**Ready** for review.'")?;
        b.key("Escape", 27, 0)?;
        b.wait("document.activeElement.textContent==='Keep editing'")?;
        b.key("Escape", 27, 0)?;
        b.wait("document.activeElement.id==='composer-body' && document.activeElement.value==='**Ready** for review.'")?;
        b.text("#composer-body", "")?;
        b.wait("(()=>{const e=document.querySelector('.composer-editor-content'),s=getComputedStyle(e),c=document.createElement('canvas').getContext('2d');const lum=color=>{c.fillStyle=color;c.fillRect(0,0,1,1);const v=[...c.getImageData(0,0,1,1).data].slice(0,3).map(n=>n/255).map(n=>n<=.04045?n/12.92:((n+.055)/1.055)**2.4);return v[0]*.2126+v[1]*.7152+v[2]*.0722};return (lum(s.borderTopColor)+.05)/(lum(s.backgroundColor)+.05)>=3})()")?;
        b.call(
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
        )?;
        b.wait("getComputedStyle(document.querySelector('.segment-indicator')).transitionDuration==='0s'")?;
        b.call(
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":"prefers-reduced-motion","value":"no-preference"}]}),
        )?;
    } else {
        // Baseline status is static; capture its actual composition honestly.
        crop(b, "dialog", &output.join("before-status-static.png"))?;
    }
    b.eval("document.querySelector('#board-title').focus();true")?;
    b.viewport(1440, 900)?;
    b.screenshot(&output.join(format!("{phase}-full-page.png")))?;
    b.viewport(390, 844)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join(format!("{phase}-mobile.png")))?;
    b.viewport(1200, 900)?;
    b.key("Escape", 27, 0)?;
    b.wait("!document.querySelector('dialog')")?;
    if !before {
        b.go(&format!("{base}/design/c/modal"))?;
        b.wait("!!document.querySelector('.component-stage > button')")?;
        b.click(".component-stage > button")?;
        b.wait("document.querySelector('dialog')?.dataset.state==='open'")?;
        check_modal(b)?;
    }
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1200,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    Ok(())
}

/// Board chrome evidence uses only the in-memory design fixtures.
fn capture_board_topbar(b: &mut Browser, base: &str, output: &Path, before: bool) -> super::Result {
    let phase = if before { "before" } else { "after" };
    b.go(&format!("{base}/design/board"))?;
    b.wait("document.querySelectorAll('.board-column').length>=4")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    b.wait("!document.querySelector('[data-ar-t]')")?;
    for width in [1440, 390] {
        b.call(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":900,"deviceScaleFactor":2,"mobile":false}),
        )?;
        b.wait("document.documentElement.scrollWidth<=innerWidth && document.querySelector('.topbar').getBoundingClientRect().height===44")?;
        b.wait(if before { "!!document.querySelector('.topbar .button-primary')" } else { "!document.querySelector('.topbar button') && !document.querySelector('.topbar .toolbar-divider')" })?;
        crop(
            b,
            ".topbar",
            &output.join(format!("{phase}-board-topbar-{width}.png")),
        )?;
        b.viewport(width, 900)?;
        b.screenshot(&output.join(format!("{phase}-board-{width}.png")))?;
    }
    if !before {
        for route in ["list", "ticket", "home"] {
            b.go(&format!("{base}/design/{route}"))?;
            b.wait("document.querySelector('.topbar .button-primary')?.textContent.includes('New ticket')")?;
        }
    }
    b.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":1200,"height":900,"deviceScaleFactor":2,"mobile":false}),
    )?;
    Ok(())
}

/// Matching, fixture-only TIK-50 evidence, including the former toolbar.
fn capture_board_columns(
    b: &mut Browser,
    base: &str,
    output: &Path,
    before: bool,
) -> super::Result {
    let phase = if before { "before" } else { "after" };
    b.go(&format!("{base}/design/board"))?;
    b.wait("document.querySelectorAll('.board-column').length>=4")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    b.wait("!document.querySelector('[data-ar-t]')")?;
    for width in [1440, 390] {
        b.call(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":900,"deviceScaleFactor":2,"mobile":false}),
        )?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        crop(
            b,
            ".view-toolbar",
            &output.join(format!("{phase}-toolbar-{width}.png")),
        )?;
        b.viewport(width, 900)?;
        b.screenshot(&output.join(format!("{phase}-board-{width}.png")))?;
        b.call(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":900,"deviceScaleFactor":2,"mobile":false}),
        )?;
        if !before {
            b.click("button[aria-label=Columns]")?;
            b.wait("document.querySelector('button[aria-label=Columns]').getAttribute('aria-expanded')==='true' && document.querySelector('.columns-menu .popup').dataset.state==='open'")?;
            crop(
                b,
                ".columns-menu .popup",
                &output.join(format!("{phase}-popover-{width}.png")),
            )?;
            b.viewport(width, 900)?;
            b.screenshot(&output.join(format!("{phase}-board-popover-{width}.png")))?;
            b.key("Escape", 27, 0)?;
            b.wait("document.activeElement.getAttribute('aria-label')==='Columns'")?;
        } else {
            // The old toolbar had no Columns popover; record that absence.
            crop(
                b,
                ".view-toolbar",
                &output.join(format!("{phase}-popover-{width}.png")),
            )?;
        }
    }
    Ok(())
}

/// Exercise the real shared Home/Analytics switcher with immutable HTTP fixtures.
fn project_switcher_shots(
    root: &Path,
    assets: &Path,
    output: &Path,
    before: bool,
) -> super::Result {
    let seed: serde_json::Value =
        serde_json::from_str(include_str!("../../docs/fixtures/home-api.json"))?;
    let home: serde_json::Value =
        serde_json::from_str(include_str!("../../docs/fixtures/home.json"))?;
    let mut projects = seed["projects"].clone();
    // Cover the sidebar's three supported identity forms: glyph, emoji and initial.
    projects[2]["icon"] = json!("🚀");
    projects[4]["icon"] = serde_json::Value::Null;
    let server = FixtureServer::start_with_api(
        assets,
        std::collections::HashMap::from([
            (
                "/api/v1/me".into(),
                json!({"authenticated":true,"username":"sample-user","display_name":"Sample User"}),
            ),
            (
                "/api/v1/projects".into(),
                json!({"data":projects,"next_cursor":null}),
            ),
            ("/api/v1/analytics/home".into(), home),
        ]),
    )?;
    let mut b = Browser::start_named(root, "project-switcher")?;
    let phase = if before { "before" } else { "after" };
    for page in ["home", "analytics"] {
        for (size, width) in [("desktop", 1440), ("mobile", 390)] {
            b.viewport(width, 900)?;
            b.go(&format!("{}/{page}", server.url))?;
            b.wait("!!document.querySelector('.home-page[aria-busy=false]') && !document.querySelector('#boot-splash,[data-ar-t]') && !document.querySelector('.home-toolbar .select-trigger:disabled')")?;
            b.click(".home-toolbar .select-trigger")?;
            b.wait("!!document.querySelector('.home-toolbar .select-trigger[aria-expanded=true]') && document.querySelectorAll('.select-option').length===6")?;
            check_menu_spacing(&mut b)?;
            b.wait("document.documentElement.scrollWidth<=innerWidth")?;
            if !before {
                b.wait("(()=>{const labels=[...document.querySelectorAll('.select-option > span:not(.select-glyph):not(.project-mark):not(.option-check)')];return labels.every(e=>Math.abs(e.getBoundingClientRect().left-labels[0].getBoundingClientRect().left)<1)})()")?;
                b.wait("!!document.querySelector('.select-value .select-glyph > .icon') && document.querySelectorAll('.select-option .project-mark').length===5 && !!document.querySelector('.select-option[aria-selected=true] .option-check')")?;
                b.eval("(()=>{for(const o of document.querySelectorAll('.select-option[data-value]:not([data-value=\"\"])')){const side=document.querySelector('.sidebar a[href=\"/board?project='+o.dataset.value+'\"] .project-mark');if(!side || side.outerHTML!==o.querySelector('.project-mark').outerHTML)throw Error('Sidebar icon mismatch');}return true;})()")?;
            }
            b.eval("document.fonts.ready.then(()=>true)")?;
            b.screenshot(&output.join(format!("{phase}-{page}-{size}.png")))?;
            if size == "desktop" {
                b.call(
                    "Emulation.setDeviceMetricsOverride",
                    json!({"width":1440,"height":900,"deviceScaleFactor":2,"mobile":false}),
                )?;
                crop(
                    &mut b,
                    ".home-toolbar .select-root",
                    &output.join(format!("{phase}-{page}-switcher.png")),
                )?;
                b.viewport(width, 900)?;
            }
            if !before {
                b.key("a", 65, 0)?;
                b.key("p", 80, 0)?;
                b.wait("document.activeElement.dataset.value==='3'")?;
                b.key("End", 35, 0)?;
                b.wait("document.activeElement.dataset.value==='5'")?;
                b.key("Home", 36, 0)?;
                b.key("ArrowDown", 40, 0)?;
                b.key("Enter", 13, 0)?;
                b.wait("location.search==='?project=1' && !!document.querySelector('.select-value .project-mark') && document.activeElement.matches('.select-trigger')")?;
                b.click(".home-toolbar .select-trigger")?;
                b.wait("!!document.querySelector('.select-trigger[aria-expanded=true]') && !!document.querySelector('.select-option[aria-selected=true][data-value=\"1\"]')")?;
                b.screenshot(&output.join(format!("{phase}-{page}-{size}-selected.png")))?;
                b.key("Escape", 27, 0)?;
                b.wait("document.activeElement.matches('.select-trigger') && !!document.querySelector('.select-trigger[aria-expanded=false]')")?;
            }
        }
    }
    println!(
        "PASS project switcher: fixture icons, shared spacing, selection, keyboard and responsive layout"
    );
    Ok(())
}

/// TIK-109: the same fixture tickets before/after, plus the real shared composer.
fn classification_shots(
    b: &mut Browser,
    base: &str,
    output: &Path,
    before: bool,
    assets: &Path,
) -> super::Result {
    let phase = if before { "before" } else { "after" };
    let ticket = json!({"id":9,"project_id":1,"key":"TKT-9","title":"Create and edit tickets","type":"feat","scope":"ui","status":"todo","priority":2,"assignee":null,"sort_order":0,"labels":[],"blocked_by":[],"blocks":[],"comment_count":0,"created_at":"2026-10-31T12:00:00.000Z","updated_at":"2026-10-31T12:00:00.000Z","body":"Keep ticket titles plain."});
    let fixtures = std::collections::HashMap::from([
        (
            "/api/v1/tickets".into(),
            json!({"data":[ticket.clone()],"next_cursor":null}),
        ),
        ("/api/v1/tickets/TKT-9".into(), ticket.clone()),
        ("/api/v1/tickets/9".into(), ticket),
    ]);
    let api_server = FixtureServer::start_with_api(assets, fixtures)?;
    for (width, height, size) in [(1440, 900, "desktop"), (390, 844, "mobile")] {
        b.call(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":height,"deviceScaleFactor":2,"mobile":false}),
        )?;
        for (route, selector, name) in [
            ("/design/board", ".ticket-card", "card"),
            ("/design/list", ".ticket-row", "row"),
            ("/design/ticket?id=9", ".ticket-detail", "detail"),
        ] {
            b.go(&format!("{base}{route}"))?;
            b.wait(&format!("!!document.querySelector('{}')", selector))?;
            b.eval("document.fonts.ready.then(()=>new Promise(r=>setTimeout(()=>r(true),400)))")?;
            crop(
                b,
                selector,
                &output.join(format!("{phase}-{name}-{size}.png")),
            )?;
            crop(
                b,
                "body",
                &output.join(format!("{phase}-{name}-{size}-page.png")),
            )?;
            if !before {
                b.wait("!!document.querySelector('.ticket-type')")?;
            }
        }
        b.go(&format!("{}/t/TKT-9", api_server.url))?;
        b.wait("!!document.querySelector('.ticket-detail h1')")?;
        b.click(".topbar .button-secondary")?;
        b.wait("document.querySelector('dialog')?.dataset.state==='open'")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        crop(
            b,
            "dialog",
            &output.join(format!("{phase}-edit-{size}.png")),
        )?;
        b.go(&format!("{base}/design/c/modal"))?;
        b.wait("!!document.querySelector('.component-stage > button')")?;
        b.click(".component-stage > button")?;
        b.wait("document.querySelector('dialog')?.dataset.state==='open'")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        crop(
            b,
            "dialog",
            &output.join(format!("{phase}-create-{size}.png")),
        )?;
        if !before {
            b.click("[aria-label='Ticket type']")?;
            b.wait("!!document.querySelector('[role=option]')")?;
            b.eval("[...document.querySelectorAll('[role=option]')].find(e=>e.textContent.trim()==='feat').click();true")?;
            b.wait("document.querySelector('[aria-label=\"Ticket type\"]').textContent.includes('feat')")?;
            b.eval("const s=document.querySelector('[aria-label=\"Ticket scope\"]');s.value=' UI ';s.dispatchEvent(new Event('input',{bubbles:true}));true")?;
            crop(
                b,
                "dialog",
                &output.join(format!("{phase}-create-classified-{size}.png")),
            )?;
            // A valid maximum-length scope with surrounding whitespace must reach
            // API normalization intact, including when pasted through the browser.
            b.eval("document.querySelector('[aria-label=\"Ticket scope\"]').focus();document.querySelector('[aria-label=\"Ticket scope\"]').select();true")?;
            b.call(
                "Input.insertText",
                json!({"text":format!(" {} ","A".repeat(32))}),
            )?;
            b.wait("document.querySelector('[aria-label=\"Ticket scope\"]').value===' '+ 'A'.repeat(32)+' '")?;
            b.click("[aria-label='Ticket type']")?;
            b.wait("document.querySelector('[aria-label=\"Ticket type\"]').getAttribute('aria-expanded')==='true'")?;
            b.eval("document.getElementById(document.querySelector('[aria-label=\"Ticket type\"]').getAttribute('aria-controls')).querySelector('[data-value=\"\"]').click();true")?;
            b.wait("document.querySelector('[aria-label=\"Ticket type\"]').textContent.includes('No type')")?;
        }
    }
    Ok(())
}
