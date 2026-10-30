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
    "palette",
    "sidebar",
    "user-menu",
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
        if name == "palette" {
            super::palette::shots(root, &assets, &output, before.is_some())?;
            continue;
        }
        if name == "modal" {
            capture_composer(&mut b, &server.url, &output, before.is_some())?;
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
pub(crate) fn crop(b: &mut Browser, selector: &str, path: &Path) -> super::Result {
    b.eval("Promise.all(document.getAnimations().filter(a=>a.effect?.getTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{}))).then(()=>true)")?;
    b.wait("[...document.querySelectorAll('.popup:not([hidden])')].every(p=>getComputedStyle(p).opacity==='1')")?;
    b.eval(
        "new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve(true))))",
    )?;
    let selector = serde_json::to_string(selector)?;
    b.eval("Promise.race([Promise.all(document.getAnimations().filter(a=>a.effect?.getTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{}))),new Promise(r=>setTimeout(r,300))]).then(()=>true)")?;
    let clip=b.eval(&format!("(()=>{{const e=document.querySelector({selector}),r=e.getBoundingClientRect();let left=r.left,top=r.top,right=r.right,bottom=r.bottom;e.querySelectorAll('.popup:not([hidden])').forEach(p=>{{const q=p.getBoundingClientRect();left=Math.min(left,q.left);top=Math.min(top,q.top);right=Math.max(right,q.right);bottom=Math.max(bottom,q.bottom);}});return {{x:Math.max(0,Math.floor(left)-8),y:Math.max(0,Math.floor(top)-8),width:Math.ceil(right-left)+16,height:Math.ceil(bottom-top)+16,scale:1}}}})()"))?;
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
    b.wait("!!document.querySelector('.topbar .button-primary') && !document.querySelector('.topbar .button-primary').disabled")?;
    b.click(".topbar .button-primary")?;
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
