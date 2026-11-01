//! Focused #54 evidence on isolated local D1, including composited WCAG ratios.
use crate::design::Browser;
use serde_json::{Value, json};
use std::{env, fs, path::Path};

// Paint backgrounds from the canvas outward, then composite text before applying
// WCAG's sRGB transfer function. The primary button deliberately has opacity 1.
const MEASURE: &str = r#"(()=>{
 const rgba=s=>{const c=(s.match(/[\d.]+/g)||[]).map(Number);return s.startsWith('color(srgb')?c.map((v,i)=>i<3?v*255:v):c};
 const over=(f,b)=>f.slice(0,3).map((v,i)=>v*(f[3]??1)+b[i]*(1-(f[3]??1)));
 const bg=e=>e?over(rgba(getComputedStyle(e).backgroundColor),bg(e.parentElement)):[0,0,0];
 const lum=c=>c.map(v=>{v/=255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4}).reduce((a,v,i)=>a+v*[.2126,.7152,.0722][i],0);
 // Project initials have their own foreground; the parent tint colors decorative SVGs.
 return [...document.querySelectorAll('.button-primary,kbd,.label-chip span,.agent-badge,.avatar,.project-mark > span,.comment-count,.resolved,.blocked-indicator')].filter(e=>e.getClientRects().length).map(e=>{
 for(let p=e;p;p=p.parentElement)if(Number(getComputedStyle(p).opacity)!==1)throw Error('Unexpected opacity in contrast measurement');
 const c=getComputedStyle(e),background=bg(e),foreground=over(rgba(c.color),background),a=lum(foreground),b=lum(background);
 return {selector:e.className||e.tagName,text:e.textContent.trim(),foreground,background,ratio:(Math.max(a,b)+.05)/(Math.min(a,b)+.05),size:c.fontSize,opacity:c.opacity};
 });
})()"#;

fn headings(b: &mut Browser) -> super::Result {
    b.wait("!!document.querySelector('h1')")?;
    if b.eval(r#"(()=>{let previous=0;return [...document.querySelectorAll('h1,h2,h3,h4,h5,h6')].every(e=>{const n=Number(e.tagName[1]),ok=n<=previous+1;previous=n;return ok})})()"#)? != true {
        return Err("Heading levels skip on the current route".into());
    }
    Ok(())
}

fn capture(
    b: &mut Browser,
    output: &Path,
    phase: &str,
    route: &str,
    rows: &mut Vec<Value>,
) -> super::Result {
    for (size, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
        b.viewport(width, height)?;
        b.call(
            "Input.dispatchMouseEvent",
            json!({"type":"mouseMoved","x":0,"y":0}),
        )?;
        b.eval("new Promise(r=>setTimeout(r,180))")?;
        b.screenshot(&output.join(format!("{route}-{phase}-{size}-rest.png")))?;
        if phase == "after" {
            b.wait("document.documentElement.scrollWidth<=innerWidth")?;
            headings(b)?;
            let pairs = b.eval(MEASURE)?;
            for pair in pairs.as_array().ok_or("missing contrast pairs")? {
                if pair["ratio"].as_f64().ok_or("missing ratio")? < 4.5 {
                    return Err(format!("{route}: contrast failure {pair}").into());
                }
                if pair["size"]
                    .as_str()
                    .unwrap_or("0px")
                    .trim_end_matches("px")
                    .parse::<f64>()?
                    < 11.0
                {
                    return Err(format!("{route}: undersized badge {pair}").into());
                }
            }
            rows.push(json!({"route":route,"viewport":size,"state":"rest","pairs":pairs}));
        }
        if size == "desktop" {
            let point=b.eval("(()=>{const r=document.querySelector('.button-primary').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()")?;
            b.call(
                "Input.dispatchMouseEvent",
                json!({"type":"mouseMoved","x":point["x"],"y":point["y"]}),
            )?;
            b.eval("new Promise(r=>setTimeout(r,180))")?;
            b.screenshot(&output.join(format!("{route}-{phase}-{size}-hover.png")))?;
            if phase == "after" {
                for state in ["hover", "active", "focus", "disabled"] {
                    match state {
                        "active" => {
                            b.call("Input.dispatchMouseEvent",json!({"type":"mousePressed","x":point["x"],"y":point["y"],"button":"left","clickCount":1}))?;
                        }
                        "focus" => {
                            // Release away from the button so no action fires.
                            b.call(
                                "Input.dispatchMouseEvent",
                                json!({"type":"mouseMoved","x":0,"y":0}),
                            )?;
                            b.call("Input.dispatchMouseEvent",json!({"type":"mouseReleased","x":0,"y":0,"button":"left","clickCount":1}))?;
                            b.key("Tab", 9, 0)?;
                            b.eval("document.querySelector('.button-primary').focus();true")?;
                            b.wait("document.querySelector('.button-primary').matches(':focus-visible')")?;
                        }
                        "disabled" => {
                            b.eval("document.querySelector('.button-primary').disabled=true;true")?;
                        }
                        _ => {}
                    }
                    b.eval("new Promise(r=>setTimeout(r,180))")?;
                    let pairs = b.eval(MEASURE)?;
                    for pair in pairs.as_array().ok_or("missing pairs")? {
                        if pair["ratio"].as_f64().ok_or("missing ratio")? < 4.5 {
                            return Err(format!("{route} {state}: {pair}").into());
                        }
                    }
                    rows.push(json!({"route":route,"viewport":size,"state":state,"pairs":pairs}));
                }
                b.eval("document.querySelector('.button-primary').disabled=false;document.activeElement.blur();true")?;
            }
        }
    }
    if route == "login" {
        b.eval("document.querySelector('#username').focus();true")?;
    }
    b.viewport(1440, 900)?;
    Ok(())
}

// Keep the same unresolved blocker, routes and content for both asset builds.
fn mobile_layout(b: &mut Browser, root: &Path, phase: &str) -> super::Result {
    let output = root.join("docs/screenshots/55");
    fs::create_dir_all(&output)?;
    for (route, path, ready, marker) in [
        (
            "list",
            "/list?project=1",
            "document.querySelectorAll('.api-list-row').length===4",
            "#list-ticket-1 .row-blocked",
        ),
        (
            "ticket-detail",
            "/ticket?id=1",
            "!!document.querySelector('.comment')",
            ".dependency-kind",
        ),
    ] {
        b.go(&format!("http://127.0.0.1:8795{path}"))?;
        b.wait(ready)?;
        for (size, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
            b.viewport(width, height)?;
            b.call(
                "Input.dispatchMouseEvent",
                json!({"type":"mouseMoved","x":0,"y":0}),
            )?;
            b.eval("new Promise(r=>setTimeout(r,180))")?;
            b.eval("document.fonts.ready.then(()=>{window.scrollTo(0,0);return true})")?;
            if phase == "after" {
                b.wait(&format!(r#"(()=>{{const e=document.querySelector('{marker}');if(!e)return false;const r=e.getBoundingClientRect(),s=getComputedStyle(e);return r.width>0 && r.height>0 && s.visibility==='visible' && s.display!=='none'}})()"#))?;
                b.wait("document.documentElement.scrollWidth<=innerWidth")?;
                if size == "mobile" && route == "ticket-detail" {
                    b.wait("document.querySelector('.properties').getBoundingClientRect().bottom<=document.querySelector('.detail-description').getBoundingClientRect().top && document.querySelector('.properties').getBoundingClientRect().top<document.querySelector('.activity').getBoundingClientRect().top")?;
                    b.wait("!!(document.querySelector('.properties').compareDocumentPosition(document.querySelector('.activity')) & Node.DOCUMENT_POSITION_FOLLOWING)")?;
                }
                if route == "list" {
                    b.wait("(()=>{const row=document.querySelector('#list-ticket-1').getBoundingClientRect(),marker=document.querySelector('#list-ticket-1 .row-blocked').getBoundingClientRect();return marker.left>=row.left && marker.right<=row.right && marker.top>=row.top && marker.bottom<=row.bottom})()")?;
                }
            }
            b.screenshot(&output.join(format!("{route}-{phase}-{size}.png")))?;
        }
    }
    b.viewport(1440, 900)?;
    Ok(())
}

pub fn check(root: &Path, password: &str) -> super::Result {
    let phase = if env::var_os("TICKETS_BEFORE_ASSETS").is_some() {
        "before"
    } else {
        "after"
    };
    let output = root.join("docs/screenshots/54");
    fs::create_dir_all(&output)?;
    let mut rows = Vec::new();
    let mut b = Browser::start_named(root, "contrast-browser")?;
    b.call("Network.clearBrowserCookies", json!({}))?;
    b.go("http://127.0.0.1:8795/login")?;
    if env::var_os("TICKETS_MOBILE_LAYOUT_ONLY").is_none() {
        capture(&mut b, &output, phase, "login", &mut rows)?;
    }
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/'")?;
    b.eval(r#"(async()=>{
      const api=async(p,body,method='POST')=>{const r=await fetch('/api/v1/'+p,{method,headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error('Seed failed');return r.json()};
      await api('projects',{name:'Tickets',key_prefix:'TKT',color:'#5e6ad2'});
      const labels=[];
      for(const [name,color] of [['Feature','#8b82d9'],['Bug','#eb5757'],['Infrastructure','#4ea7fc'],['Design','#c6a1b9']])labels.push(await api('labels',{name,color}));
      for(const [i,title] of ['Keep keyboard focus after moving tickets','Group the ticket list by status','Link projects to GitHub','Preserve compact metadata'].entries()){
        const t=await api('tickets',{project_id:1,title,status:['in_progress','todo','backlog','done'][i],priority:2,body:'### Acceptance criteria\n- [x] Show status, labels and priority\n- [ ] Keep keyboard focus visible\n\nUse the status menu or drag a card to move it.'});
        await api('tickets/'+t.id+'/labels/'+labels[i].id,{});
      }
      await api('tickets/2/dependencies/1',{});
      await api('tickets/1/comments',{body:'Verify the shortcut hints and labels in each view.'});
      return true;
    })()"#)?;
    mobile_layout(&mut b, root, phase)?;
    if env::var_os("TICKETS_MOBILE_LAYOUT_ONLY").is_some() {
        println!("PASS: #55 {phase} mobile layout evidence and rendered regression assertions");
        return Ok(());
    }
    b.go("http://127.0.0.1:8795/list?project=1")?;
    b.wait("document.querySelectorAll('.api-list-row').length===4")?;
    capture(&mut b, &output, phase, "list", &mut rows)?;
    b.go("http://127.0.0.1:8795/ticket?id=1")?;
    b.wait("!!document.querySelector('.comment')")?;
    capture(&mut b, &output, phase, "ticket-detail", &mut rows)?;
    b.viewport(390, 844)?;
    b.eval("document.querySelector('.properties').scrollIntoView();true")?;
    b.screenshot(&output.join(format!("ticket-properties-{phase}-mobile.png")))?;
    b.viewport(1440, 900)?;
    b.go("http://127.0.0.1:8795/board")?;
    b.wait("document.querySelectorAll('.api-ticket').length===4")?;
    capture(&mut b, &output, phase, "board", &mut rows)?;
    if phase == "after" {
        for route in [
            "/",
            "/board",
            "/projects",
            "/settings/tokens",
            "/design",
            "/design/board",
            "/design/list",
            "/design/ticket",
            "/design/empty",
            "/design/projects",
        ] {
            b.go(&format!("http://127.0.0.1:8795{route}"))?;
            headings(&mut b)?;
            let pairs = b.eval(MEASURE)?;
            for pair in pairs.as_array().ok_or("missing pairs")? {
                if pair["ratio"].as_f64().ok_or("missing ratio")? < 4.5 {
                    return Err(format!("{route}: {pair}").into());
                }
                if pair["size"]
                    .as_str()
                    .ok_or("missing size")?
                    .trim_end_matches("px")
                    .parse::<f64>()?
                    < 11.0
                {
                    return Err(format!("{route}: undersized badge {pair}").into());
                }
            }
            rows.push(json!({"route":route,"viewport":"desktop","state":"rest","pairs":pairs}));
        }
        fs::write(
            output.join("contrast-pairs.json"),
            format!(
                "[\n{}\n]\n",
                rows.iter()
                    .map(serde_json::to_string)
                    .collect::<Result<Vec<_>, _>>()?
                    .join(",\n")
            ),
        )?;
    }
    println!(
        "PASS: #54 {phase} captures; after checks cover all route headings, badge sizes and WCAG text states"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    fn token(name: &str) -> [f64; 3] {
        let css = include_str!("../../crates/web/style/tokens.css");
        let value = css
            .split(&format!("--{name}: "))
            .nth(1)
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .trim();
        if let Some(alias) = value
            .strip_prefix("var(--")
            .and_then(|s| s.strip_suffix(')'))
        {
            return token(alias);
        }
        let hex = value.strip_prefix('#').expect("hex color or palette alias");
        std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap() as f64)
    }
    fn luminance(c: [f64; 3]) -> f64 {
        c.into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(v, weight)| {
                let s = v / 255.0;
                weight
                    * if s <= 0.04045 {
                        s / 12.92
                    } else {
                        ((s + 0.055) / 1.055).powf(2.4)
                    }
            })
            .sum()
    }
    fn ratio(a: [f64; 3], b: [f64; 3]) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    #[test]
    fn action_text_and_shortcuts_meet_aa_in_every_state() {
        for fill in [
            "action-fill",
            "action-hover",
            "action-active",
            "action-disabled",
        ] {
            let contrast = ratio(token("action-text"), token(fill));
            assert!(contrast >= 4.5, "{fill}: {contrast}");
        }
        // Focus is offset from the fill, over the surrounding dark surfaces.
        for surface in ["bg", "sidebar", "surface", "surface-raised"] {
            assert!(ratio(token("focus"), token(surface)) >= 3.0, "{surface}");
        }
    }
}
