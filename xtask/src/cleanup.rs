//! TIK-45 acceptance against disposable fixture data, never the live service.
use crate::design::Browser;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;
use std::{env, fs, path::Path};

pub fn check(b: &mut Browser, root: &Path) -> super::Result {
    let before = env::var_os("TICKETS_BEFORE_ASSETS").is_some();
    let phase = if before { "before" } else { "after" };
    let output = root.join("docs/screenshots/TIK-45");
    fs::create_dir_all(&output)?;
    // Capture a stable supported motion preference at every viewport; resizing
    // must not snapshot an in-flight FLIP transform from the previous size.
    b.call("Emulation.setEmulatedMedia", json!({"features":[{"name":"prefers-color-scheme","value":"dark"},{"name":"prefers-reduced-motion","value":"reduce"}]}))?;
    b.eval(r#"(async()=>{
      const api=async(path,body,method='POST')=>{const r=await fetch('/api/v1/'+path,{method,headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error(path);return r.json()};
      await api('tickets/1',{title:'Remember filters when switching views'},'PATCH');
      const imported=await api('tickets/by-external?id=github:example/fixture:42',{project_id:1,title:'Review imported ticket metadata and exceptionally long text in the compact list',body:'Model: sol\n\nFollow up on #1 and **#2**. Keep `#3` as code.',status:'todo',priority:2,external_url:'https://github.com/example/fixture/issues/42'},'PUT');
      for(const name of ['priority:low','status:blocked','model:sol','Feature with a long label']){const label=await api('labels',{name,color:'#8b82d9'});await api('tickets/'+imported.id+'/labels/'+label.id,{})}
      const later=await api('labels',{name:'later',color:'#8b82d9'});
      const backlog=await api('tickets',{project_id:1,title:'Deferred fixture ticket',status:'todo'});
      await api('tickets/'+backlog.id+'/labels/'+later.id,{});
      const blocker=await api('tickets',{project_id:1,title:'Unresolved fixture blocker',status:'todo'});
      await api('tickets/'+blocker.id+'/dependencies/'+imported.id,{});
      await api('tickets',{project_id:1,title:'Canceled fixture ticket',status:'canceled'});
      window.cleanupTicket=imported;
      window.cleanupBlocker=blocker;
      window.cleanupLater=backlog.id;
      return true;
    })()"#)?;
    let key = b
        .eval("window.cleanupTicket.key")?
        .as_str()
        .ok_or("fixture key")?
        .to_owned();
    let id = b
        .eval("window.cleanupTicket.id")?
        .as_i64()
        .ok_or("fixture id")?;
    let blocker = b
        .eval("window.cleanupBlocker.id")?
        .as_i64()
        .ok_or("blocker id")?;
    let later = b.eval("window.cleanupLater")?.as_i64().ok_or("later id")?;
    b.go("http://127.0.0.1:8795/list?project=1")?;
    b.wait(&format!("!!document.querySelector('#list-ticket-{id}')"))?;
    if !before {
        b.wait("![...document.querySelectorAll('.label-chip')].some(e=>/priority:|status:blocked|^later$/.test(e.textContent)) && !document.body.innerText.includes('Canceled fixture ticket')")?;
        b.wait("[...document.querySelectorAll('[aria-label=Backlog] .row-title')].some(e=>e.textContent==='Deferred fixture ticket')")?;
        b.wait(&format!("document.querySelector('#list-ticket-{id} .row-blocked')?.textContent.includes('Blocked')"))?;
        b.eval(r#"(()=>{const rows=[...document.querySelectorAll('.api-list-row')];const x=rows[0].querySelector('.row-title').getBoundingClientRect().x;if(!rows.every(r=>r.querySelector('.row-title').getBoundingClientRect().x===x))throw Error('Misaligned title columns');return true})()"#)?;
    }
    for (size, width) in [("desktop", 1440), ("mobile", 390)] {
        b.viewport(width, 900)?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        full_page(b, &output.join(format!("{phase}-list-{size}.png")))?;
    }
    if !before {
        for width in [770, 900, 1200] {
            b.viewport(width, 900)?;
            b.wait("document.documentElement.scrollWidth<=innerWidth && [...document.querySelectorAll('.api-list-row')].every(row=>row.scrollWidth<=row.clientWidth)")?;
        }
        b.go("http://127.0.0.1:8795/list?project=1&canceled=true")?;
        b.wait("document.body.innerText.includes('Canceled fixture ticket')")?;
    }
    b.viewport(1440, 900)?;
    b.go(&format!("http://127.0.0.1:8795/t/{key}"))?;
    b.wait("!!document.querySelector('.property-dates')")?;
    if !before {
        b.wait("document.querySelector('.detail-description a[href=\"/t/TKT-1\"]') && !document.querySelector('.detail-description').textContent.includes('Model: sol') && document.querySelector('.activity').textContent.includes('Imported from GitHub') && document.querySelector('.property-dates').textContent.includes('UTC') && !!document.querySelector('.properties .row-blocked')")?;
    }
    b.eval("document.fonts.ready.then(()=>true)")?;
    full_page(b, &output.join(format!("{phase}-detail-desktop.png")))?;
    b.viewport(390, 900)?;
    if !before {
        b.wait("document.querySelector('.properties-toggle')?.getAttribute('aria-expanded')==='false' && document.querySelector('.properties .property').getClientRects().length===0")?;
    }
    full_page(b, &output.join(format!("{phase}-detail-mobile.png")))?;
    if !before {
        b.click(".properties-toggle")?;
        b.wait("document.querySelector('.properties-toggle').getAttribute('aria-expanded')==='true' && document.querySelector('.properties .property').getClientRects().length>0")?;
        b.eval("document.querySelector('.properties').scrollIntoView();true")?;
        b.screenshot(&output.join("after-properties-mobile.png"))?;
        // Resolve the blocker; the badge must disappear without removing metadata.
        b.eval(&format!(r#"fetch('/api/v1/tickets/{blocker}',{{method:'PATCH',headers:{{'Content-Type':'application/json'}},body:'{{"status":"done"}}'}}).then(r=>r.ok)"#))?;
        b.go(&format!("http://127.0.0.1:8795/t/{key}"))?;
        b.wait("document.querySelector('.property-dates') && !document.querySelector('.properties .row-blocked')")?;
        b.eval(&format!(r#"fetch('/api/v1/tickets/{id}').then(r=>r.json()).then(t=>{{if(t.labels.length!==4 || !t.body.startsWith('Model: sol'))throw Error('Stored metadata changed');return true}})"#))?;
        // Reordering another Backlog ticket must not rewrite legacy Todo data.
        b.viewport(1440, 900)?;
        b.go("http://127.0.0.1:8795/board")?;
        b.wait(&format!("!!document.querySelector('#ticket-{later}')"))?;
        b.eval("document.querySelector('#ticket-1').focus();true")?;
        b.key(" ", 32, 0)?;
        b.key("ArrowDown", 40, 0)?;
        b.key(" ", 32, 0)?;
        b.wait("document.querySelector('.board').getAttribute('aria-busy')==='false'")?;
        b.eval(&format!("fetch('/api/v1/tickets/{later}').then(r=>r.json()).then(t=>{{if(t.status!=='todo'||!t.labels.some(l=>l.name==='later'))throw Error('Legacy backlog data changed');return true}})"))?;
        // Brief reconnects should remain quiet; sustained failures must surface.
        let script = b.call("Page.addScriptToEvaluateOnNewDocument", json!({"source":r#"
            const NativeSocket=window.WebSocket,start=Date.now();
            window.WebSocket=class extends NativeSocket{constructor(...args){if(Date.now()-start<300)throw Error('Fixture socket interruption');super(...args)}};
            window.reconnectFlashed=false;
            new MutationObserver(()=>{if(document.querySelector('.connection.reconnecting'))window.reconnectFlashed=true}).observe(document,{subtree:true,childList:true});
        "#}))?;
        b.viewport(1440, 900)?;
        b.go("http://127.0.0.1:8795/list?project=1")?;
        b.wait("!!document.querySelector('.connection.connected')")?;
        b.eval("if(window.reconnectFlashed)throw Error('Reconnect indicator flickered');true")?;
        b.call(
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({"identifier":script["identifier"]}),
        )?;
        let script=b.call("Page.addScriptToEvaluateOnNewDocument",json!({"source":"window.WebSocket=class{constructor(){throw Error('Fixture sustained interruption')}}"}))?;
        b.go("http://127.0.0.1:8795/list?project=1")?;
        b.wait("!!document.querySelector('.connection.reconnecting')")?;
        b.call(
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({"identifier":script["identifier"]}),
        )?;
    }
    println!(
        "PASS TIK-45: {phase} list/detail desktop/mobile evidence; legacy metadata, backlog, canceled toggle, derived blockers, references, mobile properties and stored-data preservation"
    );
    Ok(())
}

fn full_page(b: &mut Browser, path: &Path) -> super::Result {
    b.eval("document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(()=>r(true)))))")?;
    b.eval("Promise.race([Promise.all(document.getAnimations().filter(a=>a.effect?.getTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{}))),new Promise(r=>setTimeout(r,400))]).then(()=>true)")?;
    let clip=b.eval("({x:0,y:0,width:innerWidth,height:Math.max(innerHeight,document.documentElement.scrollHeight),scale:1})")?;
    let result = b.call(
        "Page.captureScreenshot",
        json!({"format":"png","captureBeyondViewport":true,"clip":clip}),
    )?;
    fs::write(
        path,
        STANDARD.decode(result["data"].as_str().ok_or("missing PNG")?)?,
    )?;
    Ok(())
}
