//! TIK-40: fixture screenshots and an isolated authenticated settings journey.
use crate::{
    component::{FixtureServer, visible_crop},
    design::Browser,
};
use std::{fs, path::Path};

pub fn shots(root: &Path) -> super::Result {
    let output = root.join("docs/screenshots/TIK-40");
    fs::create_dir_all(&output)?;
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS");
    let assets = before
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let server = FixtureServer::start(&assets)?;
    let prefix = if before.is_some() { "before" } else { "after" };
    for section in ["account", "api-tokens", "projects", "appearance"] {
        let mut b = Browser::start_named(root, "settings-shots")?;
        b.viewport(1440, 900)?;
        // These sections did not exist before TIK-40: show the actual prior settings entry point.
        let route = if before.is_some() {
            "/settings/tokens".to_string()
        } else {
            format!("/design/settings/{section}")
        };
        b.go(&format!("{}{route}", server.url))?;
        b.wait("!!document.querySelector('.settings-panel, .settings-section, .token-inventory')")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        visible_page(
            &mut b,
            &output.join(format!("{prefix}-settings-{section}.png")),
        )?;
        visible_crop(
            &mut b,
            if before.is_some() {
                ".auth-page"
            } else {
                ".settings-content"
            },
            &output.join(format!("{prefix}-section-{section}.png")),
        )?;
    }
    let mut b = Browser::start_named(root, "settings-shell")?;
    b.viewport(1440, 900)?;
    b.go(&format!("{}/design/board", server.url))?;
    b.wait("!!document.querySelector('.sidebar')")?;
    visible_page(&mut b, &output.join(format!("{prefix}-shell.png")))?;
    drop(b);
    let mut b = Browser::start_named(root, "settings-mobile")?;
    b.viewport(390, 844)?;
    b.go(&format!(
        "{}{}",
        server.url,
        if before.is_some() {
            "/settings/tokens"
        } else {
            "/design/settings/account"
        }
    ))?;
    b.wait("!!document.querySelector('.settings-panel, .settings-section, .token-inventory')")?;
    visible_page(
        &mut b,
        &output.join(format!("{prefix}-settings-mobile.png")),
    )?;
    println!("PASS settings screenshots: fixture-only, dark, desktop and mobile");
    Ok(())
}

pub fn check(root: &Path) -> super::Result {
    let worker = crate::local_worker::LocalWorker::start(root, Default::default())?;
    let password = worker.password();
    let url = worker.url();
    let mut b = Browser::start_named(root, "settings-check")?;
    b.viewport(1440, 900)?;
    b.go(&format!("{url}/login"))?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/' && !!document.querySelector('.user-menu')")?;
    b.go(&format!("{url}/settings"))?;
    b.wait("location.pathname==='/settings/account' && !document.querySelector('#display-name')?.disabled")?;
    b.text("#display-name", "Sample User")?;
    b.click(".settings-save button")?;
    b.wait("document.querySelector('.settings-save').textContent.includes('Changes saved.')")?;
    b.go(&format!("{url}/settings/account"))?;
    b.wait("document.querySelector('#display-name')?.value==='Sample User'")?;
    b.click(".user-menu .account-trigger")?;
    b.wait("document.querySelector('.user-menu .account-trigger').getAttribute('aria-expanded')==='true' && document.querySelector('.account-trigger').textContent.includes('Sample User')")?;
    b.wait("document.activeElement.matches('.user-menu [role=menuitem]')")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.matches('.user-menu .account-trigger')")?;
    b.click("[aria-controls=sidebar-projects]")?;
    b.wait("document.querySelector('#sidebar-projects').hidden")?;
    b.go(&format!("{url}/settings/account"))?;
    b.wait("document.querySelector('#sidebar-projects')?.hidden===true")?;
    b.click("[aria-controls=sidebar-projects]")?;
    b.go(&format!("{url}/settings/tokens"))?;
    b.wait("location.pathname==='/settings/api-tokens' && !document.querySelector('#token-label')?.disabled")?;
    b.click(".token-create")?;
    b.text("#token-label", "Settings check")?;
    b.eval("window.tokenFetch=window.fetch;window.fetch=(url,options)=>(url.url || String(url)).endsWith('/tokens') && (options?.method || url.method)==='POST' ? Promise.resolve(new Response(JSON.stringify({error:{message:'Fixture create failed'}}),{status:503,headers:{'Content-Type':'application/json'}})) : window.tokenFetch(url,options);true")?;
    b.click(".settings-footer button[type=submit]")?;
    b.wait("document.querySelector('#token-create-error').textContent.includes('Fixture create failed') && document.querySelector('#token-label').value==='Settings check' && !document.querySelector('.secret')")?;
    b.eval("window.fetch=window.tokenFetch;true")?;
    b.click(".settings-footer button[type=submit]")?;
    b.wait(
        "!!document.querySelector('.secret') && document.querySelectorAll('.tokens li').length===1",
    )?;
    // Clipboard rejection must preserve the secret and permit manual acknowledgement.
    b.eval("window.tokenCopy=navigator.clipboard.writeText.bind(navigator.clipboard);navigator.clipboard.writeText=()=>Promise.reject(Error('Fixture clipboard denied'));true")?;
    b.click(".secret button")?;
    b.wait("document.querySelector('#token-copy-status').textContent.includes('copy it manually') && !!document.querySelector('.secret textarea')")?;
    b.eval("navigator.clipboard.writeText=window.tokenCopy;true")?;
    b.key("Escape", 27, 0)?;
    b.wait("!!document.querySelector('.secret') && document.querySelector('.token-acknowledge').disabled")?;
    b.click(".token-saved input")?;
    b.click(".token-acknowledge")?;
    b.wait("!document.querySelector('dialog') && document.activeElement.matches('.token-create')")?;
    b.click(".tokens button")?;
    b.wait(
        "document.querySelector('dialog[data-state=open]')?.textContent.includes('Settings check')",
    )?;
    b.key("Escape", 27, 0)?;
    b.wait(
        "!document.querySelector('dialog') && document.activeElement.matches('.tokens button')",
    )?;
    b.click(".tokens button")?;
    b.eval("window.fetch=(url,options)=>(options?.method || url.method)==='DELETE' ? Promise.resolve(new Response(JSON.stringify({error:{message:'Fixture revoke failed'}}),{status:503,headers:{'Content-Type':'application/json'}})) : window.tokenFetch(url,options);true")?;
    b.click(".modal-footer .button-danger")?;
    b.wait("document.querySelector('.token-revoke').textContent.includes('Fixture revoke failed') && document.querySelectorAll('.tokens li').length===1")?;
    b.eval("window.fetch=window.tokenFetch;true")?;
    b.click(".modal-footer .button-danger")?;
    b.wait("document.querySelector('.token-result').textContent.includes('Revoked Settings check') && !document.querySelector('.tokens li')")?;
    b.click(".token-history input")?;
    b.wait("document.querySelector('.tokens').textContent.includes('Revoked')")?;
    b.go(&format!("{url}/settings/appearance"))?;
    b.wait("!!document.querySelector('[aria-label=Motion]')")?;
    b.choose("[aria-label=Motion]", "reduce")?;
    b.wait("document.documentElement.dataset.motion==='reduce'")?;
    b.go(&format!("{url}/settings/appearance"))?;
    b.wait("document.documentElement.dataset.motion==='reduce' && document.querySelector('[aria-label=Motion]')?.textContent.includes('Reduce motion')")?;
    b.wait("!document.querySelector('[aria-label=Theme]') && getComputedStyle(document.documentElement).colorScheme==='dark'")?;
    // Retired preferences must be removed on startup, whatever their old value.
    for legacy in ["light", "system", "dark", "unknown"] {
        b.eval(&format!(
            "localStorage.setItem('tickets.theme', '{legacy}');true"
        ))?;
        b.go(&format!("{url}/settings/appearance"))?;
        b.wait("!!document.querySelector('[aria-label=Motion]') && localStorage.getItem('tickets.theme')===null && getComputedStyle(document.documentElement).colorScheme==='dark'")?;
    }
    b.go(&format!("{url}/home"))?;
    b.wait("!!document.querySelector('.home-empty')")?;
    b.eval("document.activeElement.blur();true")?;
    let modifier = if b.eval("navigator.platform.includes('Mac')")? == true {
        4
    } else {
        2
    };
    b.key("4", 52, modifier)?;
    b.wait("location.pathname==='/list'")?;
    b.key("3", 51, modifier)?;
    b.wait("location.pathname==='/board'")?;
    b.key("?", 191, 8)?;
    b.wait("!!document.querySelector('dialog[open]') && document.querySelector('dialog').textContent.includes('Keyboard shortcuts')")?;
    b.key("Escape", 27, 0)?;
    b.viewport(390, 844)?;
    b.go(&format!("{url}/settings/account"))?;
    b.wait("!!document.querySelector('#display-name')")?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    b.click("[aria-label='Toggle sidebar']")?;
    b.wait("document.querySelector('.app-shell').classList.contains('sidebar-open')")?;
    b.key("Escape", 27, 0)?;
    b.wait("document.activeElement.getAttribute('aria-label')==='Toggle sidebar'")?;
    super::pickup::check(&mut b)?;
    github_check(root, &mut b, url)?;
    b.sign_out()?;
    b.wait("location.pathname==='/login'")?;
    println!(
        "PASS settings: account persistence, menu focus, collapsed sections, token redirect/lifecycle, appearance persistence, Home and Cmd/Ctrl+number shortcuts, mobile overflow/focus"
    );
    Ok(())
}

pub(crate) fn visible_page(b: &mut Browser, path: &Path) -> super::Result {
    use base64::{Engine, engine::general_purpose::STANDARD};
    b.eval("new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(()=>r(true))))")?;
    let png = b.call(
        "Page.captureScreenshot",
        serde_json::json!({"format":"png","fromSurface":false}),
    )?;
    fs::write(
        path,
        STANDARD.decode(png["data"].as_str().ok_or("missing PNG")?)?,
    )?;
    Ok(())
}

fn github_check(root: &Path, b: &mut Browser, url: &str) -> super::Result {
    let output = root.join("docs/screenshots/TIK-70");
    fs::create_dir_all(&output)?;
    // Capture the real previous Settings surface from explicitly supplied base assets.
    if let Some(assets) = std::env::var_os("TICKETS_GITHUB_BEFORE_ASSETS") {
        let server = FixtureServer::start(Path::new(&assets))?;
        let mut before = Browser::start_named(root, "github-before")?;
        before.call(
            "Emulation.setEmulatedMedia",
            serde_json::json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
        )?;
        for (name, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
            before.viewport(width, height)?;
            before.go(&format!("{}/design/settings/account", server.url))?;
            before.wait("!!document.querySelector('.settings-section') && !document.querySelector('#display-name')?.disabled")?;
            before.wait("!document.querySelector('.settings-github')")?;
            before.eval("document.fonts.ready.then(()=>true)")?;
            github_page(
                &mut before,
                &output.join(format!("before-settings-{name}.png")),
            )?;
            github_crop(
                &mut before,
                ".settings-section",
                &output.join(format!("before-section-{name}.png")),
            )?;
        }
    }
    b.viewport(1440, 900)?;
    b.go(&format!("{url}/settings/appearance"))?;
    b.wait("!!document.querySelector('[aria-label=Motion]')")?;
    b.eval("window.githubFetch=window.fetch;window.githubReply={registered:false,app_slug:null,app_name:null,html_url:null,installations:[]};window.fetch=(url,options)=>(url.url || String(url)).endsWith('/github/app') ? new Promise(resolve=>{window.githubResolve=()=>resolve(new Response(JSON.stringify(window.githubReply),{status:window.githubStatus || 200,headers:{'Content-Type':'application/json'}}));if(!window.githubHold)window.githubResolve();}) : window.githubFetch(url,options);window.githubHold=true;true")?;
    b.click(".settings-nav a[href='/settings/account']")?;
    b.wait("document.querySelector('.settings-github')?.textContent.includes('Loading GitHub connection') && !document.querySelector('.settings-github a')")?;
    b.eval("window.githubHold=false;window.githubResolve();true")?;
    b.wait("document.querySelector('.settings-github a.button')?.textContent==='Connect GitHub'")?;
    b.wait("document.querySelector('.settings-github a.button').getAttribute('href')==='/github/app/register' && document.querySelector('.settings-github a.button').rel==='external'")?;
    b.wait("document.querySelector('.settings-github a[href=\"/github/app/register?name=alt\"]')?.rel==='external'")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    github_page(b, &output.join("after-settings-not-registered-desktop.png"))?;
    github_crop(
        b,
        ".settings-github",
        &output.join("after-section-not-registered-desktop.png"),
    )?;
    // Prove the link unloads the SPA and reaches the session-gated Worker page.
    b.eval("window.githubSpaMarker=true;true")?;
    b.click(".settings-github a.button")?;
    b.wait("location.pathname==='/github/app/register' && !window.githubSpaMarker && !!document.querySelector('main')")?;
    b.go(&format!("{url}/settings/appearance"))?;
    b.wait("!!document.querySelector('[aria-label=Motion]')")?;
    b.eval("window.githubFetch=window.fetch;window.githubReply={registered:true,app_slug:'tickets-fixture',app_name:'Tickets Fixture',html_url:'https://github.com/apps/tickets-fixture',installations:[]};window.fetch=(url,options)=>(url.url || String(url)).endsWith('/github/app') ? Promise.resolve(new Response(JSON.stringify(window.githubReply),{status:window.githubStatus || 200,headers:{'Content-Type':'application/json'}})) : window.githubFetch(url,options);true")?;
    b.click(".settings-nav a[href='/settings/account']")?;
    b.wait("document.querySelector('.settings-github a.button')?.textContent.includes('Install on GitHub') && document.querySelector('.settings-github a.button').href==='https://github.com/apps/tickets-fixture/installations/new'")?;
    b.wait("document.querySelector('.settings-github .subtle-link')?.href==='https://github.com/apps/tickets-fixture'")?;
    for selection in ["all", "selected"] {
        b.click(".settings-nav a[href='/settings/appearance']")?;
        b.wait("!!document.querySelector('[aria-label=Motion]')")?;
        b.eval(&format!("window.githubReply.installations=[{{account_login:'fixture-org',repository_selection:'{selection}',repos:[]}}];true"))?;
        b.click(".settings-nav a[href='/settings/account']")?;
        b.wait("document.querySelector('.settings-github')?.textContent.includes('Connected') && document.querySelector('.settings-github').textContent.includes('fixture-org')")?;
        b.wait(&format!(
            "document.querySelector('.settings-github').textContent.includes('{} repositories')",
            if selection == "all" {
                "All"
            } else {
                "Selected"
            }
        ))?;
        b.wait("document.querySelector('.settings-github a.button').textContent.includes('Manage on GitHub') && document.querySelector('.settings-github a.button').href==='https://github.com/apps/tickets-fixture/installations/new'")?;
    }
    for (name, width, height) in [("desktop", 1440, 900), ("mobile", 390, 844)] {
        b.viewport(width, height)?;
        b.wait("document.documentElement.scrollWidth<=innerWidth")?;
        b.eval("document.fonts.ready.then(()=>true)")?;
        github_page(
            b,
            &output.join(format!("after-settings-installed-{name}.png")),
        )?;
        b.eval("document.querySelector('.settings-github').scrollIntoView({block:'center'});true")?;
        github_crop(
            b,
            ".settings-github",
            &output.join(format!("after-section-installed-{name}.png")),
        )?;
    }
    b.click(".settings-nav a[href='/settings/appearance']")?;
    b.wait("!!document.querySelector('[aria-label=Motion]')")?;
    b.eval("window.githubStatus=503;window.githubReply={error:{message:'Fixture GitHub unavailable'}};true")?;
    b.click(".settings-nav a[href='/settings/account']")?;
    b.wait("document.querySelector('.settings-github [role=alert]')?.textContent.includes('Fixture GitHub unavailable') && !document.querySelector('.settings-github a.button')")?;
    b.eval("window.githubStatus=200;window.githubReply={registered:false,app_slug:null,app_name:null,html_url:null,installations:[]};true")?;
    b.click(".settings-github button")?;
    b.wait("document.querySelector('.settings-github a.button')?.textContent==='Connect GitHub'")?;
    b.eval("document.querySelector('.settings-github').scrollIntoView({block:'center'});true")?;
    github_crop(
        b,
        ".settings-github",
        &output.join("after-section-not-registered-mobile.png"),
    )?;
    b.eval("window.scrollTo(0,0);window.fetch=window.githubFetch;true")?;
    github_page(b, &output.join("after-settings-not-registered-mobile.png"))?;
    println!(
        "PASS GitHub settings: loading, errors/retry, full-page manifest navigation, install link, installed all/selected, desktop/mobile screenshots"
    );
    Ok(())
}

fn github_page(b: &mut Browser, path: &Path) -> super::Result {
    let size = b.eval(
        "({width:innerWidth,height:innerHeight,full:document.documentElement.scrollHeight})",
    )?;
    b.viewport(
        size["width"].as_u64().unwrap() as u32,
        size["full"].as_u64().unwrap() as u32,
    )?;
    b.eval("window.scrollTo(0,0);true")?;
    visible_page(b, path)?;
    b.viewport(
        size["width"].as_u64().unwrap() as u32,
        size["height"].as_u64().unwrap() as u32,
    )?;
    Ok(())
}

fn github_crop(b: &mut Browser, selector: &str, path: &Path) -> super::Result {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let selector = serde_json::to_string(selector)?;
    b.eval(&format!(
        "document.querySelector({selector}).scrollIntoView({{block:'center'}});true"
    ))?;
    b.eval("document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(()=>r(true)))))")?;
    let clip = b.eval(&format!("(()=>{{const r=document.querySelector({selector}).getBoundingClientRect();return {{x:Math.max(0,r.x+scrollX-8),y:Math.max(0,r.y+scrollY-8),width:r.width+16,height:r.height+16,scale:2}}}})()"))?;
    let png = b.call("Page.captureScreenshot", serde_json::json!({"format":"png","fromSurface":true,"captureBeyondViewport":true,"clip":clip}))?;
    fs::write(
        path,
        STANDARD.decode(png["data"].as_str().ok_or("missing GitHub crop")?)?,
    )?;
    println!("Screenshot: {}", path.display());
    Ok(())
}

/// Actual browser processing plus authenticated Worker/D1 binary roundtrips.
pub fn avatar_check(root: &Path) -> super::Result {
    let worker = crate::local_worker::LocalWorker::start(root, Default::default())?;
    let password = worker.password();
    let url = worker.url();
    let output = std::env::var_os("TICKETS_AVATAR_SHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target/avatar-shots"));
    fs::create_dir_all(&output)?;
    let mut b = Browser::start_named(root, "avatar-check")?;
    b.viewport(1440, 900)?;
    b.go(&format!("{url}/login"))?;
    b.text("#username", "fixture")?;
    b.text("#password", password)?;
    b.click("button[type=submit]")?;
    b.wait("location.pathname==='/' && !!document.querySelector('.user-menu')")?;
    b.go(&format!("{url}/settings/account"))?;
    b.wait("!!document.querySelector('#profile-photo-file') && !document.querySelector('#profile-photo-file').disabled")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    visible_page(&mut b, &output.join("account-initials.png"))?;
    b.eval(r#"(async()=>{
      const check=(condition,message)=>{if(!condition)throw Error(message)};
      for(const [path,method] of [['/api/v1/me/avatar','PUT'],['/api/v1/me/avatar','POST'],['/api/v1/me/avatar','DELETE'],['/api/v1/users/1/avatar','GET']]) {
        const r=await fetch(path,{method,credentials:'omit',body:method==='PUT'||method==='POST'?'invalid':undefined});
        check(r.status===401,'authentication required '+method);
      }
      const token=await (await fetch('/api/v1/tokens',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name:'Avatar test'})})).json();
      let r=await fetch('/api/v1/me/avatar',{method:'PUT',headers:{Authorization:'Bearer '+token.token,'Content-Type':'image/png'},body:'invalid'});
      check(r.status===403,'session-only upload');
      r=await fetch('/api/v1/me/avatar',{method:'PUT',headers:{'Content-Type':'image/svg+xml'},body:'<svg/>'});check(r.status===415,'bad type');
      r=await fetch('/api/v1/me/avatar',{method:'PUT',headers:{'Content-Type':'image/png'},body:new Uint8Array(524289)});check(r.status===413,'oversize');
      r=await fetch('/api/v1/me/avatar',{method:'PUT',headers:{'Content-Type':'image/png'},body:new Uint8Array([255,216,255])});check(r.status===415,'magic mismatch');
      await fetch('/api/v1/tokens/'+token.metadata.id,{method:'DELETE'});
      const c=document.createElement('canvas');c.width=640;c.height=400;
      const x=c.getContext('2d');x.fillStyle='#535fc4';x.fillRect(0,0,640,400);
      x.fillStyle='#f7f8f8';x.font='180px sans-serif';x.textAlign='center';x.fillText('S',320,265);
      window.avatarOriginal=await new Promise(resolve=>c.toBlob(resolve,'image/png'));
      window.choosePhoto=(file)=>{const dt=new DataTransfer();dt.items.add(file);const input=document.querySelector('#profile-photo-file');input.files=dt.files;input.dispatchEvent(new Event('change',{bubbles:true}));};
      window.choosePhoto(new File(['bad'],'bad.svg',{type:'image/svg+xml'}));
      return true;
    })()"#)?;
    b.wait("document.querySelector('#photo-error').textContent.includes('JPEG, PNG or WebP')")?;
    b.eval("window.choosePhoto(new File([new Uint8Array(5242881)],'large.png',{type:'image/png'}));true")?;
    b.wait("document.querySelector('#photo-error').textContent.includes('5 MB')")?;
    b.eval("window.choosePhoto(new File([window.avatarOriginal],'sample.png',{type:'image/png'}));true")?;
    b.wait("document.querySelector('.profile-photo [role=status]').textContent==='Photo saved.' && document.querySelector('.profile-photo img')?.naturalWidth===256 && document.querySelector('.account-trigger img')?.naturalWidth===256")?;
    visible_page(&mut b, &output.join("account-uploaded.png"))?;
    visible_crop(&mut b, ".user-menu", &output.join("sidebar-avatar.png"))?;
    b.eval(r#"(async()=>{
      const assert=(c,m)=>{if(!c)throw Error(m)};
      const me=await (await fetch('/api/v1/me')).json();window.firstAvatar=me.avatar_url;
      let r=await fetch(me.avatar_url);assert(r.ok,'read photo');
      assert(['image/webp','image/jpeg'].includes(r.headers.get('Content-Type')),'processed type');
      assert(r.headers.get('Cache-Control')==='private, no-cache','private cache');
      const etag=r.headers.get('ETag'),blob=await r.blob();assert(blob.size<102400,'small processed image');
      const bitmap=await createImageBitmap(blob);assert(bitmap.width===256&&bitmap.height===256,'square resized');bitmap.close();
      r=await fetch(me.avatar_url,{cache:'no-store',headers:{'If-None-Match':etag}});assert(r.status===304,'ETag revalidation');
      window.encodedPhoto=blob;
      const project=await (await fetch('/api/v1/projects',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name:'Avatar check',key_prefix:'AV',color:'#535fc4'})})).json();
      const ticket=await (await fetch('/api/v1/tickets',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({project_id:project.id,title:'Profile photo across the workspace',assignee:me.username})})).json();
      await fetch('/api/v1/tickets/'+ticket.id+'/comments',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({body:'Profile photo saved and visible on comments.'})});
      return true;
    })()"#)?;
    b.go(&format!("{url}/t/AV-1"))?;
    b.wait("document.body.innerText.includes('Profile photo saved and visible on comments.') && document.querySelector('.comment .avatar img')?.naturalWidth===256 && document.querySelector('.assignee-editor .avatar img')?.naturalWidth===256")?;
    // Let the ascii-reveal loading animation settle before capturing.
    b.eval("new Promise(r=>setTimeout(()=>r(true),2500))")?;
    visible_page(&mut b, &output.join("comment-assignee-avatar.png"))?;
    b.go(&format!("{url}/settings/account"))?;
    b.wait("document.querySelector('.profile-photo img')?.naturalWidth===256")?;
    // Simulate a browser without a WebP encoder and exercise the JPEG fallback.
    b.eval(r#"(async()=>{
      const c=document.createElement('canvas');c.width=400;c.height=300;
      const x=c.getContext('2d');x.fillStyle='#535fc4';x.fillRect(0,0,400,300);
      const original=await new Promise(resolve=>c.toBlob(resolve,'image/png'));
      window.nativeToBlob=HTMLCanvasElement.prototype.toBlob;
      HTMLCanvasElement.prototype.toBlob=function(callback,type,quality){return window.nativeToBlob.call(this,callback,type==='image/webp'?'image/png':type,quality)};
      const dt=new DataTransfer();dt.items.add(new File([original],'fallback.png',{type:'image/png'}));
      const input=document.querySelector('#profile-photo-file');input.files=dt.files;input.dispatchEvent(new Event('change',{bubbles:true}));
      return true;
    })()"#)?;
    b.wait("document.querySelector('.profile-photo [role=status]').textContent==='Photo saved.'")?;
    b.eval(r#"(async()=>{
      HTMLCanvasElement.prototype.toBlob=window.nativeToBlob;
      const me=await (await fetch('/api/v1/me')).json();
      const r=await fetch(me.avatar_url);if(r.headers.get('Content-Type')!=='image/jpeg')throw Error('JPEG encoder fallback');
      return true;
    })()"#)?;
    // Another valid image exercises replacement via the POST alias, then removal.
    b.eval(r#"(async()=>{
      const before=await (await fetch('/api/v1/me')).json();
      const c=document.createElement('canvas');c.width=256;c.height=256;c.getContext('2d').fillRect(0,0,256,256);
      const blob=await new Promise(resolve=>c.toBlob(resolve,'image/jpeg',.8));
      const r=await fetch('/api/v1/me/avatar',{method:'POST',headers:{'Content-Type':blob.type},body:blob});
      const after=await r.json();if(!r.ok||after.avatar_url===before.avatar_url)throw Error('replacement version');
      const del=await fetch('/api/v1/me/avatar',{method:'DELETE'});if((await del.json()).avatar_url!==null)throw Error('remove');
      if((await fetch(before.avatar_url)).status!==404)throw Error('removed bytes still served');
      return true;
    })()"#)?;
    b.go(&format!("{url}/settings/account"))?;
    b.wait("document.querySelector('.profile-photo-actions button')?.textContent==='Upload photo' && !document.querySelector('.profile-photo img')")?;
    b.viewport(390, 844)?;
    b.wait("document.documentElement.scrollWidth<=innerWidth")?;
    visible_page(&mut b, &output.join("account-mobile.png"))?;
    println!(
        "PASS avatars: browser validation/canvas upload, auth, type/size limits, binary/cache roundtrip, replacement, removal, sidebar/comment/assignee; PNGs in {}",
        output.display()
    );
    Ok(())
}

/// The real project editor with fixture data; no account or API credentials.
pub fn project_colors_shots(root: &Path) -> super::Result {
    let before = std::env::var_os("TICKETS_BEFORE_ASSETS");
    let assets = before
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("crates/web/dist"));
    let phase = if before.is_some() { "before" } else { "after" };
    let output = root.join("docs/screenshots/TIK-135");
    fs::create_dir_all(&output)?;
    let server = FixtureServer::start(&assets)?;
    let mut b = Browser::start_named(root, "project-colors")?;
    b.viewport(1440, 1000)?;
    b.go(&format!("{}/design/board", server.url))?;
    b.wait("!!document.querySelector('.project-more-trigger')")?;
    b.click(".project-more-trigger")?;
    b.wait("document.activeElement.textContent.includes('Edit project')")?;
    b.key("Enter", 13, 0)?;
    b.wait("!!document.querySelector('.project-colors')")?;
    b.eval("document.fonts.ready.then(()=>true)")?;
    if before.is_none() {
        b.wait("document.querySelectorAll('.project-colors input[type=radio]').length===9")?;
        b.eval("document.querySelector('.project-colors input').focus();true")?;
        b.key("ArrowDown", 40, 0)?;
        b.wait("document.activeElement.value==='Purple' && document.activeElement.checked")?;
        b.key("ArrowUp", 38, 0)?;
        b.wait("document.activeElement.value==='Indigo' && document.activeElement.checked")?;
        b.click(".project-colors input[value=Graphite]")?;
        b.key("ArrowUp", 38, 0)?;
        b.key("ArrowDown", 40, 0)?;
        b.wait("document.querySelector('.project-colors input[value=Graphite]').checked")?;
        b.wait("getComputedStyle(document.querySelector('.project-color-row:has(input:focus-visible)')).outlineStyle==='solid'")?;
        b.eval(r#"(()=>{
            const icon=document.querySelector('.project-icon-picker .project-mark');
            const style=getComputedStyle(icon);
            const channels=s=>s.match(/[\d.]+/g).slice(-3).map(v=>Number(v)*(s.startsWith('color(')?255:1));
            const luminance=rgb=>rgb.map(v=>{v/=255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4}).reduce((v,c,i)=>v+c*[.2126,.7152,.0722][i],0);
            const fg=luminance(channels(style.color)),bg=luminance(channels(style.backgroundColor));
            const ratio=(Math.max(fg,bg)+.05)/(Math.min(fg,bg)+.05);
            if(ratio<4.5)throw Error('Graphite icon contrast '+ratio);
            return {foreground:style.color,background:style.backgroundColor,ratio};
        })()"#).map(|pairs|println!("Graphite contrast: {pairs}"))?;
    }
    for (size, width, height) in [("desktop", 1440, 1000), ("mobile", 390, 844)] {
        b.viewport(width, height)?;
        b.eval("document.querySelector('.project-colors').scrollIntoView({block:'center'});true")?;
        b.screenshot(&output.join(format!("{phase}-{size}.png")))?;
        visible_crop(
            &mut b,
            ".project-colors",
            &output.join(format!("{phase}-picker-{size}.png")),
        )?;
    }
    if before.is_none() {
        b.click(".project-colors input[value=Custom]")?;
        b.text("#project-color", "#aabbcc")?;
        b.screenshot(&output.join("after-custom.png"))?;
        b.viewport(1440, 1000)?;
        b.click("dialog button[type=submit]")?;
        b.wait("!document.querySelector('dialog')")?;
        b.click(".project-more-trigger")?;
        b.wait("document.activeElement.textContent.includes('Edit project')")?;
        b.key("Enter", 13, 0)?;
        b.wait("document.querySelector('.project-colors input[value=Custom]')?.checked && document.querySelector('#project-color')?.value==='#aabbcc'")?;
        b.text("#project-color", "#4EA7FC")?;
        b.click("dialog button[type=submit]")?;
        b.wait("!document.querySelector('dialog')")?;
        b.click(".project-more-trigger")?;
        b.wait("document.activeElement.textContent.includes('Edit project')")?;
        b.key("Enter", 13, 0)?;
        b.wait("document.querySelector('.project-colors input[value=Blue]')?.checked && !document.querySelector('#project-color')")?;
    }
    println!("PASS project color picker {phase}: desktop/mobile screenshots");
    if before.is_none() {
        println!(
            "PASS named radios, arrow keys, visible focus, Graphite contrast, custom round trip and case-insensitive palette matching"
        );
    }
    Ok(())
}
