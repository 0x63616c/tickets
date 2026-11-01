import { webkit } from '/tmp/wk2/node_modules/playwright/index.mjs';
import { randomBytes, pbkdf2Sync } from 'node:crypto';
import { mkdirSync, writeFileSync, openSync } from 'node:fs';
import { spawn, execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const root = process.cwd(), dir = `${root}/target/tik95-webkit-local-${process.pid}`, out = `${root}/docs/screenshots/TIK-95/webkit`;
mkdirSync(dir, {recursive:true}); mkdirSync(out, {recursive:true});
const password=randomBytes(24).toString('hex'), salt=randomBytes(16), hash=pbkdf2Sync(password,salt,100000,32,'sha256');
writeFileSync(`${dir}/.dev.vars`, `BOOTSTRAP_USERNAME=fixture\nBOOTSTRAP_PASSWORD_HASH="pbkdf2-sha256$100000$${salt.toString('hex')}$${hash.toString('hex')}"\n`, {mode:0o600});
const config=`${dir}/wrangler.toml`, state=`${dir}/state`, base='http://127.0.0.1:18995';
writeFileSync(config,`name='tickets-webkit-local'\nmain='${root}/crates/worker/build/worker/shim.mjs'\ncompatibility_date='2026-10-27'\n[assets]\ndirectory='${root}/crates/web/dist'\nbinding='ASSETS'\nnot_found_handling='none'\nrun_worker_first=true\n[[d1_databases]]\nbinding='DB'\ndatabase_name='webkit-local'\ndatabase_id='00000000-0000-0000-0000-000000000000'\nmigrations_dir='${root}/migrations'\n[[durable_objects.bindings]]\nname='BOARD_ROOMS'\nclass_name='BoardRoom'\n[[migrations]]\ntag='v1-board-room'\nnew_sqlite_classes=['BoardRoom']\n`);
execFileSync('wrangler',['d1','migrations','apply','DB','--local','--config',config,'--persist-to',state],{stdio:'ignore'});
const worker=spawn('wrangler',['dev','--local','--ip','127.0.0.1','--port','18995','--inspector-port','0','--config',config,'--persist-to',state],{stdio:['ignore',openSync(`${dir}/worker.log`,'w'),openSync(`${dir}/worker-errors.log`,'w')],detached:true});
let browser; const lines=[], errors=[];
const log=s=>{lines.push(s); console.log(s);};
try {
  for(let i=0;i<150;i++){if(await fetch(`${base}/api/health`).then(r=>r.ok,()=>false))break; await new Promise(r=>setTimeout(r,300));}
  browser=await webkit.launch({headless:true});
  log(`Playwright WebKit: ${browser.version()}; executable: ${webkit.executablePath()}`);
  log('Release app; isolated local Wrangler/D1; generated fixture credentials; 1440x900 dark viewport.');
  const context=await browser.newContext({viewport:{width:1440,height:900},colorScheme:'dark'});
  await context.addInitScript(()=>Object.defineProperty(navigator,'platform',{get:()=> 'MacIntel'}));
  const page=await context.newPage(); page.setDefaultTimeout(20000); page.setDefaultNavigationTimeout(20000);
  page.on('pageerror',e=>errors.push(`pageerror: ${e.message}`));
  page.on('console',m=>{if(m.type()==='error')errors.push(`console error: ${m.text()}`);});
  async function shot(name){await page.evaluate(()=>document.fonts.ready);await page.screenshot({path:`${out}/${name}.png`,fullPage:true});assert.equal(errors.length,0,errors.join('\n'));}
  await page.goto(`${base}/login`); await page.locator('#username').waitFor(); await shot('login'); log('PASS /login: Wasm mounted login form');
  await page.locator('#username').fill('fixture');await page.locator('#password').fill(password);
  const loginResponse=page.waitForResponse(r=>r.url()===base+'/api/v1/login' && r.request().method()==='POST');
  await page.locator('button[type=submit]').click(); const response=await loginResponse;
  assert.equal(response.status(),200); const cookie=(await response.headerValue('set-cookie')).match(/tickets_session=([^;]+)/)[1];
  await context.addCookies([{name:'tickets_session',value:cookie,url:base,httpOnly:true,secure:false,sameSite:'Lax'}]);
  log('Local HTTP fixture auth: installed returned session cookie with secure:false in this context only; production Secure cookie unchanged.');
  await page.goto(base+'/');await page.locator('.sidebar').waitFor();
  await page.evaluate(async()=>{
    const call=async(path,body)=>{const r=await fetch('/api/v1/'+path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error(`fixture seed ${r.status}`);return r.json();};
    const p=await call('projects',{name:'WebKit fixture',key_prefix:'WKT',color:'#5e6ad2'});
    await call('tickets',{project_id:p.id,title:'Verify Safari navigation and board boot',status:'todo'});
  });

  for (const width of [1440,390]) {
    await page.setViewportSize({width,height:width===1440?900:844});
    for (const [route,name] of [['/','home'],['/analytics','analytics'],['/board','board'],['/list','list'],['/mission','mission']]) {
      let release; const gate=new Promise(resolve=>release=resolve); let held=0; const pending=[];
      const handler=route=>{held++; const work=(async()=>{await gate;await route.continue();})();pending.push(work);return work;};
      await page.route('**/api/v1/**',handler);
      try {
        await page.goto(base+route);
        await page.locator('.ascii-skeleton .ars').first().waitFor();
        await page.evaluate(()=>document.fonts.ready);
        await page.waitForTimeout(1000);
        assert.equal(new URL(page.url()).pathname,route);
        const report=await page.evaluate(()=>{
          const failures=[];
          const hosts=[...document.querySelectorAll('.ascii-skeleton, .ars, .ars-line, .ars-block, .ars-base, .loading-skeleton, .home-loading, .analytics-loading, .loading-indicator')];
          for(const el of hosts) if(getComputedStyle(el).backgroundColor!=='rgba(0, 0, 0, 0)') failures.push(`${el.className}: opaque background`);
          const ctx=document.createElement('canvas').getContext('2d');
          const color=v=>{ctx.fillStyle=v; return ctx.fillStyle;};
          for(const el of document.querySelectorAll('.skeleton-card')) {
            const s=getComputedStyle(el);
            if(color(s.backgroundColor)!==color(s.getPropertyValue('--surface').trim())) failures.push('card surface mismatch');
          }
          const palette=[...document.querySelectorAll('.ars')].map(el=>{
            const s=getComputedStyle(el);return {base:parseFloat(s.getPropertyValue('--ars-base')),wave:parseFloat(s.getPropertyValue('--ars-wave'))};
          });
          for(const p of palette) if(p.base!==.12||p.wave!==.22) failures.push('incorrect glyph opacity');
          return {hosts:hosts.length,palette,failures};
        });
        assert.deepEqual(report.failures,[]); assert.ok(held>0);
        // Compare actual canvas pixels, not DOM text, across animation frames.
        const frame=()=>page.evaluate(()=>[...document.querySelectorAll('.ascii-skeleton canvas')].map(c=>c.toDataURL()));
        const first=await frame(); assert.ok(first.length>0,'missing package sweep canvas');
        await shot(`${name}-${width}-loading-a`);
        await page.waitForTimeout(350);
        const second=await frame();assert.notDeepEqual(second,first,'sweep pixels did not move');
        await shot(`${name}-${width}-loading-b`);
        log(`PASS ${route} ${width}: ${held} held API requests; ${report.hosts} transparent hosts; card surfaces match --surface; base .12 / peak .22; moving canvas sweep; no browser errors`);
      } finally {release(); await Promise.all(pending); await page.unroute('**/api/v1/**',handler);}
      await page.waitForFunction(()=>!document.querySelector('.ascii-skeleton .ars'));
      await page.waitForTimeout(700);
      assert.equal(errors.length,0,errors.join('\n'));
      log(`PASS ${route} ${width}: released real local API responses; loading cleaned up`);
    }
    // Login's pending state is its submit button, not a page skeleton.
    const loginContext=await browser.newContext({viewport:{width,height:width===1440?900:844},colorScheme:'dark'});
    const loginPage=await loginContext.newPage();
    loginPage.on('pageerror',e=>errors.push(`pageerror: ${e.message}`));
    loginPage.on('console',m=>{if(m.type()==='error')errors.push(`console error: ${m.text()}`);});
    await loginPage.goto(base+'/login');await loginPage.locator('#username').waitFor();await loginPage.evaluate(()=>document.fonts.ready);
    await loginPage.screenshot({path:`${out}/login-${width}.png`,fullPage:true});
    let releaseLogin;const gate=new Promise(resolve=>releaseLogin=resolve);
    await loginPage.route('**/api/v1/login',async route=>{await gate;await route.continue();});
    await loginPage.locator('#username').fill('fixture');await loginPage.locator('#password').fill(password);
    await loginPage.locator('button[type=submit]').click();
    await loginPage.waitForTimeout(500);
    assert.equal(await loginPage.locator('button[type=submit]').isDisabled(),true);
    await loginPage.screenshot({path:`${out}/login-${width}-loading.png`,fullPage:true});
    releaseLogin();await loginPage.unrouteAll({behavior:'wait'});await loginPage.waitForTimeout(700);await loginContext.close();
    assert.equal(errors.length,0,errors.join('\n'));log(`PASS /login ${width}: mounted form and held pending submit captured; no browser errors`);
  }
  log('PASS: all six routes at 1440 and 390; zero pageerrors and zero console errors.');
} catch(e){log(`FAIL: ${e.stack}`);for(const error of errors)log(error);process.exitCode=1;}
finally{await browser?.close();try{process.kill(-worker.pid,'SIGTERM');}catch{}writeFileSync(`${out}/smoke.log`,lines.join('\n')+'\n');}
