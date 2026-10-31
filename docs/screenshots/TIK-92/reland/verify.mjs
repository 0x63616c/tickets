const { webkit, chromium } = await import(process.env.PLAYWRIGHT_MODULE || '/tmp/wk2/node_modules/playwright/index.mjs');
import { randomBytes, pbkdf2Sync } from 'node:crypto';
import { mkdirSync, writeFileSync, openSync, readFileSync, readdirSync } from 'node:fs';
import { spawn, execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const root = process.cwd(), dir = `${root}/target/tik92-reland-local-${Date.now()}`, out = `${root}/docs/screenshots/TIK-92/reland`;
mkdirSync(dir, {recursive:true}); mkdirSync(out, {recursive:true});
const password=randomBytes(24).toString('hex'), salt=randomBytes(16), hash=pbkdf2Sync(password,salt,100000,32,'sha256');
writeFileSync(`${dir}/.dev.vars`, `BOOTSTRAP_USERNAME=fixture\nBOOTSTRAP_PASSWORD_HASH="pbkdf2-sha256$100000$${salt.toString('hex')}$${hash.toString('hex')}"\n`, {mode:0o600});
const config=`${dir}/wrangler.toml`, state=`${dir}/state`, base='http://127.0.0.1:8796';
writeFileSync(config,`name='tickets-webkit-local'\nmain='${root}/crates/worker/build/worker/shim.mjs'\ncompatibility_date='2026-10-27'\n[assets]\ndirectory='${root}/crates/web/dist'\nbinding='ASSETS'\nnot_found_handling='none'\nrun_worker_first=true\n[[d1_databases]]\nbinding='DB'\ndatabase_name='webkit-local'\ndatabase_id='00000000-0000-0000-0000-000000000000'\nmigrations_dir='${root}/migrations'\n[[durable_objects.bindings]]\nname='BOARD_ROOMS'\nclass_name='BoardRoom'\n[[migrations]]\ntag='v1-board-room'\nnew_sqlite_classes=['BoardRoom']\n`);
execFileSync('wrangler',['d1','migrations','apply','DB','--local','--config',config,'--persist-to',state],{stdio:'ignore'});
const worker=spawn('wrangler',['dev','--local','--ip','127.0.0.1','--port','8796','--inspector-port','0','--config',config,'--persist-to',state],{stdio:['ignore',openSync(`${dir}/worker.log`,'w'),openSync(`${dir}/worker-errors.log`,'w')],detached:true});
let browser; const lines=[], results=[];
const log=s=>{lines.push(s); console.log(s);};
try {
const oldDom=execFileSync('git',['show','b9db5b0:crates/web/src/components/dom.js'],{encoding:'utf8'});
assert.equal(JSON.parse(execFileSync('git',['show','b9db5b0:crates/web/package-lock.json'],{encoding:'utf8'})).packages['node_modules/ascii-reveal'].integrity, JSON.parse(readFileSync(`${root}/crates/web/package-lock.json`,'utf8')).packages['node_modules/ascii-reveal'].integrity);
assert.equal(readFileSync(`${root}/crates/web/src/components/dom.js`,'utf8'),oldDom);
const oldExports=[...oldDom.matchAll(/export function (\w+)/g)].map(m=>m[1]);
const newDom=readFileSync(`${root}/crates/web/src/components/dom.js`,'utf8');
assert.deepEqual([...newDom.matchAll(/export function (\w+)/g)].map(m=>m[1]),oldExports);
const dragName=readdirSync(`${root}/crates/web/src/components`).find(n=>/^dom_drag_[a-f0-9]{16}\.js$/.test(n));
const { createHash }=await import('node:crypto');
assert.equal(dragName,`dom_drag_${createHash('sha256').update(readFileSync(`${root}/crates/web/src/components/${dragName}`)).digest('hex').slice(0,16)}.js`);
const emittedRoot=`${root}/crates/web/dist/snippets/${readdirSync(`${root}/crates/web/dist/snippets`).find(n=>n.startsWith('tickets-web-'))}/src/components`;
assert.equal(readFileSync(`${emittedRoot}/dom.js`,'utf8'),oldDom);
assert.equal(readFileSync(`${emittedRoot}/${dragName}`,'utf8'),readFileSync(`${root}/crates/web/src/components/${dragName}`,'utf8'));
const glue=readdirSync(`${root}/crates/web/dist`).find(n=>/^tickets-web-.*\.js$/.test(n));
assert(readFileSync(`${root}/crates/web/dist/${glue}`,'utf8').includes(dragName));
log('PASS release artifact identity: emitted legacy/new module bytes and new glue import match this worktree.');

  for(let i=0;i<150;i++){if(await fetch(`${base}/api/health`).then(r=>r.ok,()=>false))break; await new Promise(r=>setTimeout(r,300));}
  for(const [engine,type] of [['webkit',webkit],['chromium',chromium]]) {
    browser=await type.launch({headless:true, ...(engine==='chromium' && process.env.CHROME_BIN ? {executablePath:process.env.CHROME_BIN} : {}), args:engine==='chromium'?['--no-sandbox','--no-proxy-server']:[]});
    log(`${engine}: ${browser.version()}; executable: ${engine==='chromium' && process.env.CHROME_BIN || type.executablePath()}`);
    const context=await browser.newContext({viewport:{width:1440,height:900},colorScheme:'dark'});
    // Emulate a browser that retained EVERY previous locally imported snippet.
    // main had dom.js + ascii-reveal; neither Sortable nor the new digest path existed.
    let staleHits=0;
    await context.route('**/snippets/**',async route=>{
      const path=new URL(route.request().url()).pathname;
      if(path.endsWith('/src/components/dom.js')) {staleHits++; await route.fulfill({status:200,contentType:'text/javascript',body:oldDom});}
      else if(path.endsWith('/node_modules/ascii-reveal/dist/core.js')) {
        staleHits++; await route.fulfill({status:200,contentType:'text/javascript',body:readFileSync(`${root}/crates/web/node_modules/ascii-reveal/dist/core.js`)});
      } else await route.continue();
    });
    const page=await context.newPage(), errors=[]; page.setDefaultTimeout(20000);
    page.on('pageerror',e=>{errors.push(`pageerror: ${e.message}`);log(`${engine} pageerror: ${e.message}`);});
    page.on('console',m=>{if(m.type()==='error'){errors.push(`console: ${m.text()}`);log(`${engine} console: ${m.text()}`);}});
    const check=()=>assert.deepEqual(errors,[]);
    const shot=async name=>{await page.evaluate(()=>document.fonts.ready); await page.screenshot({path:`${out}/${engine}-${name}.png`,fullPage:true});check();};
    await page.goto(`${base}/login`); log(`${engine} login navigation: ${new URL(page.url()).pathname}`); await page.locator('#username').fill('fixture'); await page.locator('#password').fill(password);
    const login=page.waitForResponse(r=>r.url()===base+'/api/v1/login' && r.request().method()==='POST');
    await page.locator('button[type=submit]').click(); const response=await login;
    assert.equal(response.status(),200);
    const cookie=(await response.headerValue('set-cookie')).match(/tickets_session=([^;]+)/)[1];
    await context.addCookies([{name:'tickets_session',value:cookie,url:base,httpOnly:true,secure:false,sameSite:'Lax'}]);
    await page.waitForLoadState('networkidle'); await page.goto(base+'/'); await page.locator('.sidebar').waitFor();
    const fixture=await page.evaluate(async engine=>{
      const call=async(path,body)=>{const r=await fetch('/api/v1/'+path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error(`seed ${r.status}`);return r.json();};
      const p=await call('projects',{name:`${engine} drag fixture`,key_prefix:engine==='webkit'?'WKT':'CHR',color:'#5e6ad2'});
      const ticket=await call('tickets',{project_id:p.id,title:'Verify Safari-safe dragging',status:'backlog'});
      await call('tickets',{project_id:p.id,title:'Destination anchor',status:'todo'});
      return {project:p.id,ticket:ticket.id};
    },engine);
    const board=`${base}/board?project=${fixture.project}`;
    await page.waitForLoadState('networkidle'); await page.goto(board); await page.locator(`#ticket-${fixture.ticket}`).waitFor(); await page.waitForTimeout(800);
    await shot('before'); log(`PASS ${engine}: UI login, authenticated /board boot, no errors`);
    await page.waitForLoadState('networkidle'); await page.reload(); await page.locator(`#ticket-${fixture.ticket}`).waitFor(); await page.waitForTimeout(600);
    assert(staleHits>0); check();
    log(`PASS ${engine}: new wasm booted with previous main dom.js and unchanged ascii-reveal substituted (${staleHits} old module responses)`);
    async function drag(from,to,label) {
      const card=page.locator(`#ticket-${fixture.ticket}`), rect=await card.boundingBox();
      const target=await page.locator(`.column-cards[data-status="${to}"]`).boundingBox();
      const start={x:rect.x+rect.width/2,y:rect.y+rect.height/2};
      await page.mouse.move(start.x,start.y); await page.mouse.down();
      await page.mouse.move(start.x+8,start.y+8); await page.waitForTimeout(80);
      await page.locator('.board-drag-preview').waitFor();
      const sample=async(x,y)=>page.evaluate(({x,y})=>{
        const el=document.querySelector('.board-drag-preview'), origin=document.querySelector('.board-origin-placeholder');
        const r=el.getBoundingClientRect(), o=origin.getBoundingClientRect(), css=getComputedStyle(origin);
        return {x,y,preview:{x:r.x,y:r.y},origin:{x:o.x,y:o.y,width:o.width,height:o.height,opacity:css.opacity,filter:css.filter,inert:origin.inert,ariaHidden:origin.getAttribute('aria-hidden'),pointerEvents:css.pointerEvents},transition:getComputedStyle(el).transitionDuration};
      },{x,y});
      const end={x:target.x+target.width/2,y:Math.max(target.y+60,Math.min(start.y+40,target.y+target.height-20))};
      const samples=[];
      for(let step=1;step<=20;step++) {
        const x=start.x+8+(end.x-start.x-8)*step/20,y=start.y+8+(end.y-start.y-8)*step/20;
        await page.mouse.move(x,y); await page.evaluate(()=>new Promise(requestAnimationFrame));
        const s=await sample(x,y); s.errorX=Math.abs(s.preview.x-rect.x-(x-start.x));s.errorY=Math.abs(s.preview.y-rect.y-(y-start.y));
        samples.push(s); assert(s.errorX<=2 && s.errorY<=2,JSON.stringify(s));
        assert.equal(s.origin.opacity,'0.35');assert.equal(s.origin.filter,'grayscale(1)');assert(s.origin.inert);assert.equal(s.origin.ariaHidden,'true');assert.equal(s.origin.pointerEvents,'none');
        assert(Math.abs(s.origin.x-rect.x)<=2 && Math.abs(s.origin.y-rect.y)<=2 && Math.abs(s.origin.width-rect.width)<=2 && Math.abs(s.origin.height-rect.height)<=2);
        if(step===10)await shot(`${label}-drag`);
        if(step===20)await shot(`${label}-destination-drag`);
      }
      const saved=page.waitForResponse(r=>r.url().includes(`/api/v1/tickets/${fixture.ticket}`) && r.request().method()==='PATCH');
      await page.mouse.up(); assert.equal((await saved).status(),200);
      await page.waitForFunction(({id,to})=>document.querySelector(`#ticket-${id}`)?.closest('.column-cards')?.dataset.status===to,{id:fixture.ticket,to});
      await page.waitForFunction(()=>document.querySelector('.api-board')?.dataset.busy==='false' && document.querySelector('.api-board')?.dataset.moving==='false');
      await page.waitForLoadState('networkidle');
      await page.waitForLoadState('networkidle'); await page.reload(); await page.locator(`.column-cards[data-status="${to}"] #ticket-${fixture.ticket}`).waitFor(); await page.waitForTimeout(400);
      assert.equal(await page.locator('.board-origin-placeholder').count(),0); check(); await shot(`${label}-reloaded`);
      const max=Math.max(...samples.flatMap(s=>[s.errorX,s.errorY]));
      results.push({engine,from,to,maxAxisError:max,samples});log(`PASS ${engine} ${from} -> ${to}: 20 real mouse steps, max axis error ${max}px, grey inert origin, PATCH 200, persisted after reload`);
    }
    await drag('Backlog','Todo','out'); await drag('Todo','Backlog','back');
    await context.close();await browser.close();browser=null;
  }
  log('PASS: both engines, stale-snippet substitution kept active during both persisted round trips; zero pageerrors/console errors.');
} catch(e){log(`FAIL: ${e.stack}`);process.exitCode=1;}
finally{await browser?.close();try{process.kill(-worker.pid,'SIGTERM');}catch{}writeFileSync(`${out}/smoke.log`,lines.join('\n')+'\n');writeFileSync(`${out}/tracking.json`,JSON.stringify(results,null,2)+'\n');}
