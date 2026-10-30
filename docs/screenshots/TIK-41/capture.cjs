const {chromium}=require('/tmp/quantum125-browser/node_modules/playwright-core');
const fs=require('node:fs'),path=require('node:path'),{spawnSync}=require('node:child_process'),assert=require('node:assert/strict');
const ROOT=process.cwd(),OUT='/workspace/tickets-polish/TIK-41',BASE='http://127.0.0.1:8842';
const FAKE='tk_FIXTURE_ONLY_NOT_A_VALID_TOKEN_0000000000000000000000000000000000';
const evidence={fixtureOnly:true,base:BASE,secretSubstitution:FAKE,method:'One fresh Chromium process per capture; local D1 fixtures reset before each state.',screenshots:[],checks:[],errors:[]};
const rows=[
 [1,'Fixture · Retired import','2026-09-04T10:00:00.000Z',null,'2026-10-01T10:00:00.000Z'],
 [2,'Fixture · Local CLI','2026-10-20T10:00:00.000Z',null,null],
 [3,'Fixture · Release automation','2026-10-21T10:00:00.000Z','2026-10-29T10:00:00.000Z',null],
 [4,'Fixture · Build agent','2026-10-28T10:00:00.000Z','2026-10-29T12:00:00.000Z',null]
];
const quote=x=>x===null?'NULL':"'"+String(x).replaceAll("'","''")+"'";
async function capture(state,size,viewport){
 let sql='DELETE FROM api_tokens;\n';
 if(state!=='empty')for(const [id,name,created,last,revoked]of rows){
  const hash=require('node:crypto').createHash('sha256').update('INVALID_FIXTURE_'+id).digest('hex');
  sql+=`INSERT INTO api_tokens(id,user_id,token_hash,name,created_at,last_used_at,revoked_at) VALUES(${id},1,${quote(hash)},${quote(name)},${quote(created)},${quote(last)},${quote(revoked)});\n`;
 }
 fs.writeFileSync('target/tik41-review/seed.sql',sql);
 const seed=spawnSync('wrangler',['d1','execute','DB','--local','--config','target/tik41-review/wrangler.toml','--persist-to','target/tik41-review/state2','--file','target/tik41-review/seed.sql'],{encoding:'utf8'});
 assert.equal(seed.status,0,'Local fixture seed failed');
 const b=await chromium.launch({executablePath:ROOT+'/target/tik41-review/chrome-local',args:['--no-sandbox','--disable-gpu','--no-proxy-server']});
 try{
  const c=await b.newContext({storageState:'target/tik41-review/storage2.json',viewport,deviceScaleFactor:1,colorScheme:'dark',permissions:['clipboard-read','clipboard-write']});
  const p=await c.newPage();p.on('pageerror',e=>evidence.errors.push(e.message));
  await p.goto(BASE+'/settings/tokens');
  if(state==='empty')await p.getByText('No API tokens yet.',{exact:true}).waitFor();
  else await p.waitForFunction(()=>document.querySelectorAll('.tokens li').length===4);
  if(['create','secret','secret-copied'].includes(state))await p.locator('#token-label').fill('Fixture · Documentation example');
  if(['secret','secret-copied'].includes(state)){
   await p.route('**/api/v1/tokens',async r=>{if(r.request().method()!=='POST')return r.continue();const response=await r.fetch();assert.equal(response.status(),201);const json=await response.json();json.token=FAKE;await r.fulfill({response,json});});
   await p.getByRole('button',{name:'Create token',exact:true}).click();
   await p.locator('.secret code').waitFor();
   evidence.checks.push({state,size,focusAfterCreate:await p.evaluate(()=>({tag:document.activeElement.tagName,id:document.activeElement.id})),secretIsFixture:await p.locator('.secret code').textContent()===FAKE});
   if(state==='secret-copied'){await p.getByRole('button',{name:'Copy token',exact:true}).click();await p.getByText('Token copied.',{exact:true}).waitFor();assert.equal(await p.evaluate(()=>navigator.clipboard.readText()),FAKE);}
  }
  if(['revoke','revoked'].includes(state)){
   const row=p.locator('.tokens li').filter({hasText:'Fixture · Build agent'});
   await row.getByRole('button',{name:'Revoke',exact:true}).focus();
   if(state==='revoked'){
    let dialogs=0;p.on('dialog',async d=>{dialogs++;await d.dismiss()});
    await row.getByRole('button',{name:'Revoke',exact:true}).click();await row.getByText('Revoked',{exact:true}).waitFor();
    evidence.checks.push({state,size,noConfirmation:dialogs===0&&await p.locator('dialog[open],[role=alertdialog]').count()===0});
   }
  }
  await p.evaluate(async()=>{await document.fonts.ready;window.scrollTo(0,0);document.querySelector('.app-main')?.scrollTo(0,0);document.querySelector('#main')?.scrollTo(0,0);await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));});
  const geometry=await p.evaluate(()=>({width:innerWidth,height:innerHeight,scrollWidth:document.documentElement.scrollWidth,form:document.querySelector('.settings-panel').getBoundingClientRect().toJSON(),secret:document.querySelector('.secret')?.getBoundingClientRect().toJSON(),rows:[...document.querySelectorAll('.tokens li')].map(e=>({name:e.querySelector('strong').textContent,rect:e.getBoundingClientRect().toJSON()}))}));
  assert.ok(geometry.scrollWidth<=viewport.width);
  const filename=`before-${state}-${size}.png`;
  await p.screenshot({path:path.join(OUT,filename),fullPage:false});fs.copyFileSync(path.join(OUT,filename),path.join(ROOT,'docs/screenshots/TIK-41',filename));
  evidence.screenshots.push({filename,...geometry});
  if(['secret','secret-copied'].includes(state)){await p.getByRole('button',{name:'Dismiss',exact:true}).click();assert.equal(await p.locator('.secret').count(),0);await p.reload();await p.locator('#token-label').waitFor();assert.equal(await p.locator('.secret').count(),0);}
  console.log(filename);
 }finally{await b.close();}
}
(async()=>{for(const state of ['empty','list','create','secret','secret-copied','revoke','revoked'])for(const [size,viewport]of [['desktop',{width:1440,height:900}],['mobile',{width:390,height:844}]])await capture(state,size,viewport);assert.deepEqual(evidence.errors,[]);fs.writeFileSync(OUT+'/capture-evidence.json',JSON.stringify(evidence,null,2));console.log('PASS: 14 fresh-browser fixture captures; viewport overflow, copy, dismissal, reload, revoke checked.');})().catch(e=>{console.error(e);process.exitCode=1});
