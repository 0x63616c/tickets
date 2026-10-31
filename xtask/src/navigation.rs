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
    Ok(())
}
