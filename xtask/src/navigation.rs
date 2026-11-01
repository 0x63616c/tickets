//! Fixture-only navigation shortcut regression and paired dark evidence.
use crate::{component::FixtureServer, design::Browser, settings::visible_page};
use std::{fs, path::Path};

pub fn check(root: &Path) -> super::Result {
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS");
    let assets = before
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let server = FixtureServer::start(&assets)?;
    let output = root.join("docs/screenshots/TIK-94");
    fs::create_dir_all(&output)?;
    let prefix = if before.is_some() { "before" } else { "after" };
    for (platform, modifier, key_label) in [("MacIntel", 4, "⌘"), ("Linux x86_64", 2, "Ctrl+")] {
        let mut b = Browser::start_named(root, "navigation-check")?;
        b.call(
            "Emulation.setDeviceMetricsOverride",
            serde_json::json!({"width":1440,"height":900,"deviceScaleFactor":2,"mobile":false}),
        )?;
        b.call("Page.addScriptToEvaluateOnNewDocument", serde_json::json!({"source":format!("Object.defineProperty(navigator,'platform',{{get:()=>'{platform}'}})")}))?;
        b.go(&format!("{}/design/board", server.url))?;
        b.wait("!!document.querySelector('.sidebar')")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        if before.is_none() {
            // Real CDP key dispatch verifies both platform modifiers and every destination.
            for (key, route) in [
                ("1", "home"),
                ("2", "analytics"),
                ("3", "board"),
                ("4", "list"),
                ("5", "mission"),
                ("6", "projects"),
                ("7", "settings/account"),
            ] {
                b.eval("document.activeElement.blur();true")?;
                b.key(key, key.as_bytes()[0] as u32, modifier)?;
                b.wait(&format!("location.pathname==='/design/{route}'"))?;
            }
            b.eval("document.activeElement.blur();true")?;
            // Observe cancellation directly: the browser owns some number combinations.
            b.eval(&format!(r#"(()=>{{
              const send=(key,extra={{}},target=window)=>{{const e=new KeyboardEvent('keydown',{{key,bubbles:true,cancelable:true,{modifier_key}:true,...extra}});target.dispatchEvent(e);return e.defaultPrevented;}};
              if(!send('1'))throw Error('handled navigation must preventDefault');
              for(const [key,extra] of [['9',{{}}],['0',{{}}],['2',{{altKey:true}}],['2',{{shiftKey:true}}],['2',{{repeat:true}}],['2',{{isComposing:true}}],['2',{{ctrlKey:true,metaKey:true}}]]){{if(send(key,extra))throw Error('unhandled shortcut canceled');}}
              return true;
            }})()"#, modifier_key=if modifier==4 {"metaKey"} else {"ctrlKey"}))?;
            b.wait("location.pathname==='/design/home'")?;
            // Native editable controls and inherited/plaintext contenteditable all retain focus.
            for markup in [
                "<input>",
                "<textarea></textarea>",
                "<div contenteditable><span tabindex='0'>draft</span></div>",
                "<div contenteditable='plaintext-only'>draft</div>",
            ] {
                b.eval(&format!(r#"(()=>{{const host=document.createElement('div');host.id='shortcut-edit-test';host.innerHTML={};document.body.append(host);(host.querySelector('span')||host.firstElementChild).focus();return true;}})()"#, serde_json::to_string(markup)?))?;
                b.key("4", 52, modifier)?;
                b.wait("location.pathname==='/design/home' && document.activeElement.closest('#shortcut-edit-test')!==null")?;
                b.eval("document.querySelector('#shortcut-edit-test').remove();true")?;
            }
            // Legacy sequences no longer navigate.
            b.key("g", 71, 0)?;
            b.key("l", 76, 0)?;
            b.wait("location.pathname==='/design/home'")?;
        }
        b.go(&format!("{}/design/board", server.url))?;
        b.wait("!!document.querySelector('.sidebar')")?;
        b.key("k", 75, modifier)?;
        b.wait("!!document.querySelector('.command-search input')")?;
        if before.is_none() {
            b.key("4", 52, modifier)?;
            b.wait("location.pathname==='/design/board' && document.activeElement.matches('.command-search input')")?;
            for (label, number) in [
                ("Go to home", 1),
                ("Go to analytics", 2),
                ("Go to board", 3),
                ("Go to list", 4),
                ("Go to mission control", 5),
                ("Go to projects", 6),
                ("Settings", 7),
            ] {
                b.wait(&format!("Array.from(document.querySelectorAll('.command-option')).some(r=>r.textContent.includes('{label}') && r.querySelector('kbd')?.textContent==='{key_label}{number}')"))?;
            }
        }
        b.text(".command-search input", "go to")?;
        b.wait("Array.from(document.querySelectorAll('.command-option')).some(r=>r.textContent.includes('Go to analytics'))")?;
        b.eval("(()=>{const row=Array.from(document.querySelectorAll('.command-option')).find(r=>r.textContent.includes('Go to analytics'));row.scrollIntoView({block:'start'});row.dispatchEvent(new PointerEvent('pointermove',{bubbles:true,pointerType:'mouse'}));return true;})()")?;
        b.wait("Array.from(document.querySelectorAll('.command-option')).some(r=>r.textContent.includes('Go to analytics') && r.getAttribute('aria-selected')==='true' && getComputedStyle(r.querySelector('.command-hint')).opacity==='1')")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        let os = if modifier == 4 { "mac" } else { "linux" };
        super::component::crop(
            &mut b,
            ".command-palette",
            &output.join(format!("{prefix}-palette-{os}.png")),
        )?;
        b.key("Escape", 27, 0)?;
        b.wait("!document.querySelector('dialog')")?;
        b.eval("document.activeElement.blur();true")?;
        b.key("?", 191, 8)?;
        b.wait("!!document.querySelector('.shortcut-list')")?;
        if before.is_none() {
            for number in 1..=7 {
                b.wait(&format!("Array.from(document.querySelectorAll('.shortcut-list kbd')).some(k=>k.textContent==='{key_label}{number}')"))?;
            }
        }
        super::component::crop(
            &mut b,
            "dialog",
            &output.join(format!("{prefix}-shortcuts-{os}.png")),
        )?;
        b.viewport(390, 844)?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        visible_page(
            &mut b,
            &output.join(format!("{prefix}-shortcuts-mobile-{os}.png")),
        )?;
        b.key("Escape", 27, 0)?;
        b.wait("!document.querySelector('dialog')")?;
        if before.is_none() {
            b.viewport(1440, 900)?;
            b.eval("document.querySelector('.sidebar a').focus();true")?;
            b.wait(&format!("Array.from(document.querySelectorAll('[role=tooltip]')).some(t=>t.dataset.state==='open' && t.textContent.includes('Home') && t.querySelector('kbd')?.textContent==='{key_label}1')"))?;
            visible_page(&mut b, &output.join(format!("{prefix}-tooltip-{os}.png")))?;
        }
    }
    if before.is_some() {
        println!("PASS navigation baseline: paired Mac/Linux desktop/mobile captures");
    } else {
        println!(
            "PASS navigation: Mac/Linux modifiers, seven destinations, preventDefault, editable fields, palette focus, removed chords, matching kbd hints and desktop/mobile evidence"
        );
    }
    sidebar_tooltips(root)
}

/// Real pointer/keyboard regression, also run by navigation-check in CI.
pub fn sidebar_tooltips(root: &Path) -> super::Result {
    use serde_json::json;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS");
    let assets = before
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let server = FixtureServer::start(&assets)?;
    let output = root.join("docs/screenshots/TIK-146");
    fs::create_dir_all(&output)?;
    let prefix = if before.is_some() { "before" } else { "after" };
    let mut b = Browser::start_named(root, "sidebar-tooltip-check")?;
    b.call(
        "Page.addScriptToEvaluateOnNewDocument",
        json!({"source":"Object.defineProperty(navigator,'platform',{get:()=>'MacIntel'})"}),
    )?;
    b.go(&format!("{}/design/board", server.url))?;
    b.wait("!!document.querySelector('.sidebar .tooltip-trigger')")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    let closed = "![...document.querySelectorAll('.sidebar [role=tooltip]')].some(t=>t.dataset.state==='open')";
    let open = "document.querySelector('.sidebar [role=tooltip][data-state=open]')";
    let geometry = r#"(()=>{
      const p=document.querySelector('.sidebar [role=tooltip][data-state=open]');
      if(!p)return false;
      const r=p.getBoundingClientRect(), t=p.parentElement.getBoundingClientRect();
      const rows=[...document.querySelectorAll('.sidebar .nav-item')].filter(n=>n.getClientRects().length);
      const overlaps=rows.some(n=>{const a=n.getBoundingClientRect();return r.left<a.right && r.right>a.left && r.top<a.bottom && r.bottom>a.top;});
      if(overlaps)throw Error('Tooltip overlaps a sidebar nav row (including the next item)');
      if(r.left<0 || r.right>innerWidth || r.top<0 || r.bottom>innerHeight)throw Error('Tooltip outside viewport');
      if(Math.abs((r.top+r.bottom-t.top-t.bottom)/2)>2)throw Error('Tooltip not vertically centered');
      return true;
    })()"#;
    // Expanded, section-collapsed, and the mobile drawer use the same nav component.
    for mode in ["expanded", "collapsed", "mobile"] {
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":1000,"y":800}),
        )?;
        b.eval("document.activeElement.blur();new Promise(r=>setTimeout(r,800))")?;
        if mode == "collapsed" {
            b.click(".nav-section-toggle")?;
            b.wait("document.querySelector('.nav-section-toggle').getAttribute('aria-expanded')==='false'")?;
        }
        if mode == "mobile" {
            b.viewport(390, 844)?;
            b.click("[aria-label='Toggle sidebar']")?;
            b.wait("!!document.querySelector('.sidebar-open')")?;
            b.wait("document.activeElement.matches('.sidebar a')")?;
            b.eval("document.activeElement.blur();new Promise(r=>setTimeout(r,800))")?;
        }
        let point = b.eval("(()=>{const r=document.querySelector('.sidebar .tooltip-trigger').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()")?;
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":point["x"],"y":point["y"]}),
        )?;
        if before.is_none() {
            b.eval("new Promise(r=>setTimeout(r,200))")?;
            b.wait(closed)?;
        }
        b.wait(&format!("!!({open})"))?;
        b.eval("new Promise(r=>setTimeout(r,160))")?;
        if before.is_none() {
            b.wait(geometry)?;
        }
        visible_page(&mut b, &output.join(format!("{prefix}-{mode}.png")))?;
        if before.is_some() {
            continue;
        }
        // Leave a tooltip open longer than the skip-delay window, then move to its neighbor.
        b.eval("new Promise(r=>setTimeout(r,800))")?;
        let point = b.eval("(()=>{const r=document.querySelectorAll('.sidebar .tooltip-trigger')[1].getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()")?;
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":point["x"],"y":point["y"]}),
        )?;
        b.eval("new Promise(r=>setTimeout(r,80))")?;
        b.wait_for("!!document.querySelectorAll('.sidebar .tooltip-trigger')[1].querySelector('[role=tooltip][data-state=open]')", std::time::Duration::from_millis(150))?;
        b.wait(geometry)?;
        if mode == "mobile" {
            let long = b.eval("(()=>{const r=document.querySelectorAll('.sidebar .tooltip-trigger')[2].getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()")?;
            b.call(
                "Input.dispatchMouseEvent",
                json!({"type":"mouseMoved","x":long["x"],"y":long["y"]}),
            )?;
            b.wait(&format!("!!({open})"))?;
            b.wait(geometry)?;
            visible_page(&mut b, &output.join("after-mobile-long.png"))?;
        }
        println!("sidebar tooltips: {mode} pointer dismissal");
        // Leaving alone also dismisses, without waiting for an exit animation.
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":1000,"y":800}),
        )?;
        b.wait(closed)?;
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":point["x"],"y":point["y"]}),
        )?;
        b.wait(&format!("!!({open})"))?;
        println!("sidebar tooltips: {mode} Escape dismissal");
        // Escape must dismiss pointer-open tooltips even without trigger focus.
        if mode == "mobile" {
            // Earlier blur leaves focus on body, outside AppShell's key handler.
            // Keep keyboard input inside the drawer without focusing a tooltip trigger.
            b.eval("document.querySelector('.sidebar .nav-section-toggle').focus();true")?;
            b.wait(&format!("!!({open})"))?;
        }
        b.key("Escape", 27, 0)?;
        b.wait(closed)?;
        if mode == "mobile" {
            // Escape also closes the mobile drawer; reopen it for the keyboard path.
            b.wait("!document.querySelector('.sidebar-open')")?;
            b.click("[aria-label='Toggle sidebar']")?;
            b.wait("!!document.querySelector('.sidebar-open')")?;
            // Opening schedules focus on the first link in the next animation frame.
            // Wait for that focus before clearing it for the explicit Tab sequence.
            b.wait("document.activeElement.matches('.sidebar a')")?;
            b.eval("document.activeElement.blur();true")?;
        }
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":1000,"y":800}),
        )?;
        b.wait(closed)?;
        println!("sidebar tooltips: {mode} keyboard focus");
        // Tab, rather than programmatic mouse focus, exercises focus-visible.
        b.eval("document.activeElement.blur();document.querySelector('.sidebar .tooltip-trigger a').focus();true")?;
        b.key("Tab", 9, 0)?;
        b.wait(&format!(
            "!!({open}) && document.activeElement.matches(':focus-visible')"
        ))?;
        b.wait(geometry)?;
        b.eval("document.querySelector('.sidebar-nav').dispatchEvent(new Event('scroll'));true")?;
        b.wait(closed)?;
        b.key("Tab", 9, 8)?;
        b.wait(&format!("!!({open})"))?;
        println!("sidebar tooltips: {mode} click dismissal");
        // A click on the current route must dismiss without depending on unmounting.
        b.click(".sidebar .tooltip-trigger a")?;
        b.wait(closed)?;
        b.eval("document.activeElement.blur();true")?;
        if mode == "mobile" {
            b.wait("!document.querySelector('.sidebar-open')")?;
            b.click("[aria-label='Toggle sidebar']")?;
            b.wait("!!document.querySelector('.sidebar-open')")?;
            b.call(
                "Emulation.setTouchEmulationEnabled",
                json!({"enabled":true}),
            )?;
            let point = b.eval("(()=>{const r=document.querySelector('.sidebar .tooltip-trigger a').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()")?;
            b.call(
                "Input.dispatchTouchEvent",
                json!({"type":"touchStart","touchPoints":[{"x":point["x"],"y":point["y"]}]}),
            )?;
            b.call(
                "Input.dispatchTouchEvent",
                json!({"type":"touchEnd","touchPoints":[]}),
            )?;
            b.eval("new Promise(r=>setTimeout(r,600))")?;
            b.wait(closed)?;
            b.wait("!document.querySelector('.sidebar-open')")?;
            b.call(
                "Emulation.setTouchEmulationEnabled",
                json!({"enabled":false}),
            )?;
        }
    }
    if before.is_none() {
        b.viewport(1440, 900)?;
        // Exercise the flip path using the real sidebar near the right viewport edge.
        b.eval("document.querySelector('.sidebar').style.cssText='position:fixed;left:auto;right:0;top:0;height:100vh';true")?;
        b.key("Tab", 9, 0)?;
        b.eval("document.querySelector('.sidebar .tooltip-trigger a').focus();true")?;
        b.wait(&format!("!!({open})"))?;
        b.wait("document.querySelector('.sidebar [role=tooltip][data-state=open]').dataset.side==='left'")?;
        b.wait(geometry)?;
        visible_page(&mut b, &output.join("after-flipped.png"))?;
        b.key("Escape", 27, 0)?;
        b.eval("document.activeElement.blur();document.querySelector('.sidebar').style.cssText='';true")?;
        // Dismissal while still waiting must cancel the timer, too.
        for dismiss in [
            "document.querySelector('.sidebar-nav').dispatchEvent(new Event('scroll'))",
            "window.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape'}))",
            "window.dispatchEvent(new MouseEvent('click'))",
        ] {
            b.call(
                "Input.dispatchMouseEvent",
                json!({"type":"mouseMoved","x":1000,"y":800}),
            )?;
            b.eval("new Promise(r=>setTimeout(r,800))")?;
            let point = b.eval("(()=>{const r=document.querySelector('.sidebar .tooltip-trigger').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()")?;
            b.call(
                "Input.dispatchMouseEvent",
                json!({"type":"mouseMoved","x":point["x"],"y":point["y"]}),
            )?;
            b.eval(&format!("{dismiss};new Promise(r=>setTimeout(r,650))"))?;
            b.wait(closed)?;
        }
        println!(
            "PASS sidebar tooltips: no nav overlap, centered right/left placement, delay, warm handoff, keyboard focus, Escape/click/scroll dismissal, collapsed sections and mobile drawer"
        );
    }
    Ok(())
}
