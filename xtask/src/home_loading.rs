//! Cold Home lifecycle recording against an isolated local Worker with real live rooms.
use crate::design::Browser;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;
use std::{
    env, fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

pub fn check(root: &Path, password: &str) -> super::Result {
    let phase = if env::var_os("TICKETS_BEFORE_ASSETS").is_some() {
        "before"
    } else {
        "after"
    };
    let output = root.join("docs/screenshots/TIK-65");
    let frames = root.join(format!("target/TIK-65-{phase}"));
    fs::create_dir_all(&output)?;
    fs::create_dir_all(&frames)?;
    let mut b = Browser::start_named(root, "home-loading")?;
    b.viewport(1280, 800)?;
    b.go("http://127.0.0.1:8795/login")?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("!!document.querySelector('.home-metric strong')")?;
    b.call("Network.setCacheDisabled", json!({"cacheDisabled":true}))?;
    b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":r#"
        window.homeTrace=[]; window.homeRequests=[]; window.homeErrors=[];
        addEventListener('error',e=>window.homeErrors.push(String(e.message)));
        addEventListener('unhandledrejection',e=>window.homeErrors.push(String(e.reason)));
        const nativeFetch=window.fetch;
        window.fetch=async(...args)=>{
            const url=args[0] instanceof Request ? args[0].url : String(args[0]);
            const record={time:performance.now(),url};
            if(url.includes('/api/')) window.homeRequests.push(record);
            const response=await nativeFetch(...args);
            record.completed=performance.now();
            return response;
        };
        let last=''; let skeleton=null; window.homeUnmounts=0; window.homeLifecycle=[];
        new MutationObserver(()=>{
            if(skeleton && !skeleton.isConnected) {
                window.homeUnmounts++;
                window.homeLifecycle.push({event:'loader-unmount',time:performance.now()});
                skeleton=null;
            }
            const current=document.querySelector('.home-loading');
            if(current && current!==skeleton) {
                window.homeLifecycle.push({event:'loader-mount',time:performance.now()});
                skeleton=current;
            }
        }).observe(document,{childList:true,subtree:true});
        function sample(){
            const root=document.querySelector('.home-page');
            const loader=document.querySelector('.home-loading');
            const state=!root?'boot':loader?'loader':root.querySelector('[data-ar-t], .ar-overlay canvas')?'reveal':root.querySelector('.home-metric strong')?'content':'empty';
            if(state!==last){window.homeTrace.push({time:Math.round(performance.now()),state,busy:root?.getAttribute('aria-busy')});last=state;}
            requestAnimationFrame(sample);
        }
        requestAnimationFrame(sample);
    "#}))?;
    b.call("Page.navigate", json!({"url":"http://127.0.0.1:8795/"}))?;
    let start = Instant::now();
    let mut count: usize = 0;
    let mut capture_times = Vec::new();
    let mut captured_images = Vec::new();
    while start.elapsed() < Duration::from_secs(4) {
        let tick = Instant::now();
        let shot = match b.call(
            "Page.captureScreenshot",
            json!({"format":"png","captureBeyondViewport":false}),
        ) {
            Ok(shot) => shot,
            Err(e) if e.to_string().contains("Not attached to an active page") => {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => return Err(e),
        };
        fs::write(
            frames.join(format!("{count:03}.png")),
            STANDARD.decode(shot["data"].as_str().ok_or("PNG missing")?)?,
        )?;
        captured_images.push(shot["data"].as_str().ok_or("PNG missing")?.to_string());
        capture_times.push(start.elapsed().as_millis());
        count += 1;
        thread::sleep(Duration::from_millis(50).saturating_sub(tick.elapsed()));
    }
    if let Err(error) = b.wait("!!document.querySelector('.home-metric strong') && !document.querySelector('.home-page[aria-busy=true]')") {
        eprintln!("Home startup diagnostics: {}", b.eval("({errors:window.homeErrors,trace:window.homeTrace,resources:performance.getEntriesByType('resource').map(r=>({name:r.name,status:r.responseStatus,duration:r.duration}))})")?);
        return Err(error);
    }
    let mut trace=b.eval("({transitions:window.homeTrace,requests:window.homeRequests,lifecycle:window.homeLifecycle,loaderUnmounts:window.homeUnmounts})")?;
    trace["captureTimesMs"] = json!(capture_times);
    let mut phases = Vec::new();
    for state in trace["transitions"]
        .as_array()
        .ok_or("missing transitions")?
    {
        let phase = match state["state"].as_str() {
            Some("loader" | "reveal") => "loading",
            Some("content") => "content",
            _ => continue,
        };
        if phases.last() != Some(&phase) {
            phases.push(phase);
        }
    }
    trace["loadingContentTransitions"] = json!(phases.len().saturating_sub(1));
    fs::write(
        output.join(format!("{phase}-transitions.json")),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    b.screenshot(&output.join(format!("{phase}-home.png")))?;
    // Compose only this recording's frames, using the same browser canvas approach
    // as component crops. This needs no external image tool on CI.
    let sheet = b.eval(&format!(r#"(async()=>{{
        const sources={}, canvas=document.createElement('canvas');
        canvas.width=2048;canvas.height=Math.ceil(sources.length/8)*160;
        const ctx=canvas.getContext('2d');ctx.fillStyle='#000';
        ctx.fillRect(0,0,canvas.width,canvas.height);
        await Promise.all(sources.map((data,i)=>new Promise((resolve,reject)=>{{
            const frame=new Image();
            frame.onload=()=>{{ctx.drawImage(frame,(i%8)*256,Math.floor(i/8)*160,256,160);resolve();}};
            frame.onerror=reject;frame.src='data:image/png;base64,'+data;
        }})));
        return canvas.toDataURL('image/png').split(',')[1];
    }})()"#, serde_json::to_string(&captured_images)?))?;
    fs::write(
        output.join(format!("{phase}-frames.png")),
        STANDARD.decode(sheet.as_str().ok_or("contact sheet missing")?)?,
    )?;
    println!("{phase}: {count} frames; {trace}");
    if phase == "before" {
        return Ok(());
    }
    let states = trace["transitions"].as_array().ok_or("missing trace")?;
    if trace["loaderUnmounts"] != 1
        || trace["loadingContentTransitions"] != 1
        || states.iter().filter(|s| s["state"] == "reveal").count() != 1
    {
        return Err("Home must unmount its loader once and reveal once".into());
    }
    // Browser focus uses the same catch-up path as live room reconnects.
    b.eval("window.homeTrace=[];window.dispatchEvent(new Event('focus'));true")?;
    b.eval("new Promise(r=>setTimeout(()=>r(true),1400))")?;
    b.wait("!document.querySelector('.home-page[aria-busy=true]')")?;
    if b.eval("window.homeTrace.some(s=>s.state==='loader'||s.state==='reveal')")? == true {
        return Err("Home replayed loading/reveal during catch-up".into());
    }
    // Hold later responses long enough to verify content while a request is pending.
    b.eval("window.refreshFetch=window.fetch;window.fetch=async(...args)=>{const response=await window.refreshFetch(...args);await new Promise(r=>setTimeout(r,200));return response;};true")?;
    b.click("[aria-label='Refresh Home']")?;
    b.wait("!!document.querySelector('.home-page[aria-busy=true]') && !!document.querySelector('.home-metric strong')")?;
    b.wait("!document.querySelector('.home-page[aria-busy=true]')")?;
    if b.eval("window.homeTrace.some(s=>s.state==='loader'||s.state==='reveal')")? == true {
        return Err("Home replayed loading/reveal during manual refresh".into());
    }
    b.choose("[aria-label='Home project']", "1")?;
    b.wait("location.search==='?project=1' && !document.querySelector('.home-page[aria-busy=true]') && document.querySelectorAll('.home-waiting tbody tr').length===1")?;
    if b.eval("window.homeTrace.some(s=>s.state==='loader'||s.state==='reveal')")? == true {
        return Err("Home replayed loading/reveal during scope change".into());
    }
    b.viewport(390, 844)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.screenshot(&output.join("after-home-mobile.png"))?;
    println!("PASS Home: one loader unmount/reveal; catch-up and manual refresh retain content");
    Ok(())
}
